use actix_web::{web, App, HttpResponse, HttpServer, put, options, get, HttpRequest, middleware};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use queues::*;

struct AppState {
    port_numbers: Mutex<Queue<String>>,
}

#[derive(Serialize)]
struct Response {
    message: String,
    current_numbers: Vec<String>,
}


#[get("/")]
async fn get_serve(
    state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    log::info!("Processing request");
    let mut queue = state.port_numbers.lock().map_err(|e| {
        log::error!("Failed to acquire lock: {}", e);
        actix_web::error::ErrorInternalServerError("Lock acquisition failed")
    })?;

    if queue.size() == 0 {
        return Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "No available origins"
        })));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    while queue.size() > 0 {
        let origin = queue.remove().map_err(|e| {
            log::error!("Failed to get next origin: {}", e);
            actix_web::error::ErrorInternalServerError("Queue operation failed")
        })?;

        match client.get(&format!("{}/healthCheck", origin)).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    if let Err(e) = queue.add(origin.clone()) {
                        log::error!("Failed to re-add successful origin to queue: {}", e);
                    }

                    log::info!("Redirecting to healthy origin: {}", origin);

                    return Ok(HttpResponse::TemporaryRedirect()
                        .insert_header(("Location", origin))
                        .finish());
                } else {
                    log::warn!("Health check returned non-200 status for {}: {}", origin, response.status());
                }
            },
            Err(e) => {
                log::warn!("Health check failed for {}: {}", origin, e);
            }
        }
    }

    Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
        "error": "No healthy origins available"
    })))
}

#[put("/port")]
async fn add_number(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> HttpResponse {
    let origin = match req.headers().get("origin") {
        Some(o) => match o.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid origin header",
                "status": "error"
            }))
        },
        None => return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Missing origin header",
            "status": "error"
        }))
    };

    if !origin.starts_with("http://") && !origin.starts_with("https://") {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Origin must start with http:// or https://",
            "status": "error"
        }));
    }

    let mut queue = match state.port_numbers.lock() {
        Ok(queue) => queue,
        Err(_) => {
            log::error!("Failed to acquire lock on queue");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal server error",
                "status": "error"
            }));
        }
    };

    let mut temp_queue = Queue::new();
    let mut exists = false;
    let mut current_origins = Vec::new();

    while let Ok(existing_origin) = queue.remove() {
        if existing_origin == origin {
            exists = true;
        }
        current_origins.push(existing_origin.clone());
        if let Err(_) = temp_queue.add(existing_origin) {
            log::error!("Failed to add to temporary queue");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal server error",
                "status": "error"
            }));
        }
    }

    while let Ok(origin) = temp_queue.remove() {
        if let Err(_) = queue.add(origin) {
            log::error!("Failed to restore queue");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal server error",
                "status": "error"
            }));
        }
    }

    if exists {
        log::error!("Origin {} already exists", origin);
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Origin already exists",
            "origin": origin,
            "status": "error"
        }));
    }

    if let Err(_) = queue.add(origin.clone()) {
        log::error!("Failed to add new origin to queue");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Failed to add origin to queue",
            "status": "error"
        }));
    }

    current_origins.push(origin.clone());

    let response = Response {
        message: format!("Successfully added origin: {}", origin),
        current_numbers: current_origins,
    };

    println!("Successfully added origin. Current origins: {:?}", response.current_numbers);

    HttpResponse::Ok().json(serde_json::json!({
        "message": response.message,
        "current_numbers": response.current_numbers,
        "status": "success"
    }))
}

#[options("/port")]
async fn options_handler() -> HttpResponse {
    HttpResponse::Ok()
        .append_header(("Access-Control-Allow-Methods", "PUT, OPTIONS"))
        .append_header(("Access-Control-Allow-Headers", "origin"))
        .finish()
}

#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "{} [{}] - {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();

    let app_state = web::Data::new(AppState {
        port_numbers: Mutex::new(Queue::new()),
    });
    log::info!("Server starting at http://127.0.0.1:8080");

    HttpServer::new(move || {
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::default())  // Add logger middleware
            .app_data(app_state.clone())
            .service(add_number)
            .service(get_serve)
    })
        .bind("127.0.0.1:8080")?
        .run()
        .await
}