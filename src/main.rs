mod http_server;
mod route;
mod message;
mod method;
mod error;

use std::fmt::{Display, Formatter};
use serde::{Deserialize, Serialize};
use http_server::HttpServer;
use crate::message::{Request, Response};
use crate::route::NOT_FOUND_ACTION;


#[derive(Serialize, Deserialize)]
struct Something {
    pub name: String,
    pub time: String,
}

fn main() {
    let mut server = HttpServer::default();

    server.post("/", |req: &Request| {
        let json: Something = req.json()?;
        Ok(Response::text(format!("yuor jason is {}", serde_json::to_string(&json).unwrap()), 200))
    });

    server.get("/", |req: &Request| {
        Ok(Response::text("Welcome to index", 200))
    });

    server.get("/hello", |req: &Request| {
        Ok(Response::file("hellsdfo.html", 200)?)
    });

    server.get("/hel", |req: &Request| {
        Ok(Response::file("helfdflo.html", 200)?)
    });

    server.listen(3000).unwrap();
}
