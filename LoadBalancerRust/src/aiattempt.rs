use std::net::{TcpListener, TcpStream, SocketAddr};
use std::sync::{Arc, Mutex};
use std::thread;
use std::io::{Read, Write};

// Represents the load balancer
struct LoadBalancer {
    servers: Arc<Mutex<Vec<SocketAddr>>>,
    current_server: Arc<Mutex<usize>>, // Index for round-robin selection
}

impl LoadBalancer {
    // Initialize a new load balancer
    fn new() -> Self {
        LoadBalancer {
            servers: Arc::new(Mutex::new(vec![])),
            current_server: Arc::new(Mutex::new(0)),
        }
    }

    // Add a server to the list
    fn add_server(&self, server_addr: SocketAddr) {
        let mut servers = self.servers.lock().unwrap();
        servers.push(server_addr);
    }

    // Get the next server in a round-robin fashion
    fn get_next_server(&self) -> Option<SocketAddr> {
        let mut servers = self.servers.lock().unwrap();
        if servers.is_empty() {
            return None;
        }
        let mut current = self.current_server.lock().unwrap();
        let server = servers[*current];
        *current = (*current + 1) % servers.len();
        Some(server)
    }

    // Handle client requests and forward them to the backend servers
    fn handle_client(&self, mut client: TcpStream) {
        if let Some(server_addr) = self.get_next_server() {
            if let Ok(mut server) = TcpStream::connect(server_addr) {
                let _ = client.write_all(b"Connected to server via load balancer");

                // Forward data between client and server in separate threads
                let client_to_server = client.try_clone().unwrap();
                let server_to_client = server.try_clone().unwrap();

                thread::spawn(move || {
                    let mut buffer = [0; 1024];
                    while let Ok(n) = client_to_server.read(&mut buffer) {
                        if n == 0 {
                            break;
                        }
                        let _ = server.write_all(&buffer[0..n]);
                    }
                });

                thread::spawn(move || {
                    let mut buffer = [0; 1024];
                    while let Ok(n) = server_to_client.read(&mut buffer) {
                        if n == 0 {
                            break;
                        }
                        let _ = client.write_all(&buffer[0..n]);
                    }
                });
            } else {
                eprintln!("Could not connect to server {:?}", server_addr);
            }
        } else {
            let _ = client.write_all(b"No available servers");
        }
    }

    // Listen for new client connections and handle them
    fn listen_for_clients(&self, bind_addr: &str) {
        let listener = TcpListener::bind(bind_addr).expect("Could not bind to address");

        println!("Load balancer listening on {}", bind_addr);
        for client in listener.incoming() {
            match client {
                Ok(client) => {
                    let balancer = self.clone();
                    thread::spawn(move || {
                        balancer.handle_client(client);
                    });
                }
                Err(e) => eprintln!("Failed to accept client: {}", e),
            }
        }
    }

    // Listen for backend servers to register themselves with the load balancer
    fn listen_for_servers(&self, bind_addr: &str) {
        let listener = TcpListener::bind(bind_addr).expect("Could not bind to address");

        println!("Load balancer listening for servers on {}", bind_addr);
        for server in listener.incoming() {
            match server {
                Ok(server) => {
                    let server_addr = server.peer_addr().expect("Could not get server address");
                    self.add_server(server_addr);
                    println!("Added server: {}", server_addr);
                }
                Err(e) => eprintln!("Failed to add server: {}", e),
            }
        }
    }
}

// Clone implementation for LoadBalancer to use in threads
impl Clone for LoadBalancer {
    fn clone(&self) -> Self {
        LoadBalancer {
            servers: Arc::clone(&self.servers),
            current_server: Arc::clone(&self.current_server),
        }
    }
}

pub fn main() {
    let load_balancer = LoadBalancer::new();
    let balancer_client = load_balancer.clone();
    let balancer_server = load_balancer.clone();

    // Start listening for client connections on port 8080
    thread::spawn(move || {
        balancer_client.listen_for_clients("127.0.0.1:8080");
    });

    // Start listening for backend servers to register on port 9090
    thread::spawn(move || {
        balancer_server.listen_for_servers("127.0.0.1:9090");
    });

    // Keep the main thread alive
    loop {
        thread::park();
    }
}