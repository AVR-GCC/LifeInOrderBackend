use color_eyre::Result;
use eyre::WrapErr;
//use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;
use dotenv::dotenv;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: i32
}
//Result<Config>
impl Config {
    pub fn from_env() -> Result<Config> {
        dotenv().ok();

        let env = config::Config::builder()
            .add_source(config::Environment::with_prefix("APP"))
            .build()
            .context("Error: Failed to read env")?;
        let host = env.get("host").context("Error: Host not found")?;
        let port = env.get::<String>("port")
            .context("Error: Port not found")?
            .parse()
            .context("Error: Port not valid")?;
        Ok(Config { host: host, port: port })
    }
}
