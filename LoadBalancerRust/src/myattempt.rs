use actix_web::{web, App, HttpResponse, HttpServer, put, options, get, HttpRequest};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use queues::*;

struct AppState {
    port_numbers: Mutex<Queue<String>>,
    current_index: Mutex<usize>,
}

#[derive(Deserialize)]
struct NumberPayload {
    origin: String,
}

#[derive(Serialize)]
struct Response {
    message: String,
    current_numbers: Vec<String>,
}

#[get("/")]
async fn get_serve(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> HttpResponse {
    let mut queue = state.port_numbers.lock().unwrap();

    if queue.size() == 0 {
        return HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "error": "No available origins"
        }));
    }

    let target_origin = match queue.remove() {
        Ok(origin) => {
            if let Err(e) = queue.add(origin.clone()) {
                eprintln!("Failed to re-add origin to queue: {}", e);
            }
            origin
        },
        Err(_) => return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Failed to get next origin"
        }))
    };

    let redirect_url = format!("{}{}",
                               target_origin,
                               req.uri().path_and_query()
                                   .map(|x| x.as_str())
                                   .unwrap_or("")
    );

    println!("Redirecting to origin: {}", target_origin);

    HttpResponse::PermanentRedirect()
        .append_header(("Location", redirect_url))
        .finish()
}

#[put("/port")]
async fn add_number(
    data: web::Json<NumberPayload>,
    state: web::Data<AppState>,
) -> HttpResponse {
    println!("Port endpoint accessed - Attempting to add: {}", data.origin);

    // Validate input
    if data.origin.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Origin cannot be empty",
            "status": "error"
        }));
    }

    let mut queue = match state.port_numbers.lock() {
        Ok(queue) => queue,
        Err(_) => {
            println!("Failed to acquire lock on queue");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal server error",
                "status": "error"
            }));
        }
    };

    let mut temp_queue = Queue::new();
    let mut exists = false;
    let mut current_origins = Vec::new();

    while let Ok(origin) = queue.remove() {
        if origin == data.origin {
            exists = true;
        }
        current_origins.push(origin.clone());
        if let Err(_) = temp_queue.add(origin) {
            println!("Failed to add to temporary queue");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal server error",
                "status": "error"
            }));
        }
    }

    while let Ok(origin) = temp_queue.remove() {
        if let Err(_) = queue.add(origin) {
            println!("Failed to restore queue");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal server error",
                "status": "error"
            }));
        }
    }

    if exists {
        println!("Origin {} already exists", data.origin);
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Origin already exists",
            "origin": data.origin,
            "status": "error"
        }));
    }

    if let Err(_) = queue.add(data.origin.clone()) {
        println!("Failed to add new origin to queue");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Failed to add origin to queue",
            "status": "error"
        }));
    }

    current_origins.push(data.origin.clone());

    let response = Response {
        message: format!("Successfully added origin: {}", data.origin),
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
    HttpResponse::Ok().finish()
}

#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
    let app_state = web::Data::new(AppState {
        port_numbers: Mutex::new(Queue::new()),
        current_index: Mutex::new(0),
    });

    println!("Server starting at http://127.0.0.1:8080");

    HttpServer::new(move || {
        let cors = Cors::permissive()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(app_state.clone())
            .service(add_number)
            .service(options_handler)
            .service(get_serve)
    })
        .bind("127.0.0.1:8080")?
        .run()
        .await
}