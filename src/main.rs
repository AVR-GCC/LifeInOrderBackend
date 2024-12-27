mod config;
use eyre::WrapErr;
use crate::config::Config;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::Serialize;

#[derive(Serialize)]
struct Message {
    content: String,
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello  3  world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    let response = Message { content: String::from("Hey there biitccchh!") };
    HttpResponse::Ok().json(response)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let c = Config::from_env().unwrap_or_else(|e| panic!("{:?}", e));
    HttpServer::new(|| {
        App::new()
            .service(hello)
            .service(echo)
            .route("/hey", web::get().to(manual_hello))
    })
    .bind((c.host, c.port))?
    .run()
    .await
}
