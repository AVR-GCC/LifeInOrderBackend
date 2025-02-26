use color_eyre::Result;
use eyre::WrapErr;
//use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;
use dotenv::dotenv;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: i32
}
//Result<Config>
impl Config {
    pub fn from_env() -> Result<Config> {
        dotenv().ok();

        let env = config::Config::builder()
            .add_source(config::Environment::default())
            .build()
            .context("Error: Failed to read env")?;
        let host = env.get("host").context("Error: Host not found")?;
        let port = env.get::<String>("port")
            .context("Error: Port not found")?
            .parse()
            .context("Error: Port not valid")?;
        let database_url = env.get("database_url").context("Error: Database url not found")?;
        Ok(Config { host, port, database_url })
    }
}
