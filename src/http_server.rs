use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::error::Error;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::io;
use std::io::{BufReader, Read, Write};
use std::sync::{Arc, RwLock, RwLockWriteGuard};
use std::time::{Duration, Instant};
use crate::error::{DEFAULT_HANDLER, DefaultError, ErrorAction, ServerError};
use crate::message::{Request, Response};
use crate::method::HttpMethod;
use crate::route::{NOT_FOUND_ACTION, RouteAction, Router};
use super::route::RoutingTreeNode;

pub const BUFFER_SIZE: usize = 2048;
const EDIT_AFTER_INIT_MESSAGE: &str = "Error: Attempt to edit server configuration after initialization. All configuration must be done before calling HttpServer::listen()";

pub struct HttpServer<E: ServerError, R: RouteAction<E>, F: ErrorAction<E>> {
    router: Arc<RwLock<Router<E, R>>>,
    error_handler: Arc<RwLock<F>>,
    active: bool
}

impl HttpServer<DefaultError, fn(&Request) -> Result<Response, DefaultError>, fn(&Request, DefaultError) -> Response> {
    pub fn default() -> Self {
        Self::new(NOT_FOUND_ACTION, DEFAULT_HANDLER)
    }
}

impl <E: ServerError + 'static, R: RouteAction<E>, F: ErrorAction<E>> HttpServer<E, R, F> {
    pub fn new(not_found_action: R, error_handler: F) -> Self {
        Self {
            active: false,
            error_handler: Arc::new(RwLock::new(error_handler)),
            router: Arc::new(RwLock::new(Router::new(not_found_action)))
        }
    }

    pub fn listen(mut self, port: u16) -> io::Result<()> {
        let address = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = TcpListener::bind(address)?;
        self.active = true;
        println!("Server active");

        for client in listener.incoming() {
            self.handle_client(client?)?
        }

        Ok(())
    }

    fn handle_client(&self, mut client: TcpStream) -> io::Result<()> {
        let router = self.router.clone();
        let error_handler = self.error_handler.clone();

        thread::spawn(move || {
            if let Ok(addr) = client.peer_addr() {
                println!("Accepted client: {}:{}", addr.ip(), addr.port());
                let mut buffer = [0_u8; BUFFER_SIZE];
                let mut last_request = Instant::now();
                let router_lock = router.read().unwrap();
                let err_hand_lock = error_handler.read().unwrap();

                loop {
                    let mut data = Vec::new();

                    while let Ok(size) = client.read(&mut buffer) {
                        data.extend_from_slice(&buffer[..size]);

                        if size < BUFFER_SIZE {
                            break;
                        }
                    }

                    if data.len() > 0 {
                        let request = Request::from_bytes(addr, &data).unwrap();

                        println!("Request:\n{:?}", String::from_utf8_lossy(&data));

                        let mut action = router_lock.get(request.method(), request.route());

                        let mut response = match action(&request) {
                            Ok(res) => res,
                            Err(err) => err_hand_lock(&request, err)
                        };

                        response.fill_from(&request);
                        let bytes = response.to_bytes();
                        println!("\nConnection HEADER: {:?}", request.header("Connection"));
                        println!("Response:\n{:?}", String::from_utf8_lossy(&bytes));
                        client.write(&bytes).unwrap();

                        if request.version() == 1.0 || Some("close") == request.header("Connection") {
                            break;
                        }

                        last_request = Instant::now();
                    } else if last_request.elapsed().as_secs() > 4 {
                        break;
                    }

                    thread::sleep(Duration::from_millis(50))
                }

                println!("Closing connection with: {}:{}", addr.ip(), addr.port());
            }
        });

        Ok(())
    }

    pub fn route(&mut self, method: HttpMethod, route: &str, action: R) {
        let mut router = self.edit_router();
        router.add(method, route, action);
    }

    pub fn get(&mut self, route: &str, action: R) {
        self.route(HttpMethod::Get, route, action);
    }

    pub fn post(&mut self, route: &str, action: R) {
        self.route(HttpMethod::Post, route, action);
    }

    pub fn put(&mut self, route: &str, action: R) {
        self.route(HttpMethod::Put, route, action);
    }

    pub fn patch(&mut self, route: &str, action: R) {
        self.route(HttpMethod::Patch, route, action);
    }

    pub fn delete(&mut self, route: &str, action: R) {
        self.route(HttpMethod::Delete, route, action);
    }

    pub fn panic_if_active(&self) {
        if self.active {
            panic!("{}", EDIT_AFTER_INIT_MESSAGE);
        }
    }

    pub fn edit_router(&mut self) -> RwLockWriteGuard<Router<E, R>> {
        self.panic_if_active();
        self.router.write().expect(EDIT_AFTER_INIT_MESSAGE)
    }
}