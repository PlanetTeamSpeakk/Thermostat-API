use std::{path::Path, fs};
use actix_web::{Responder, get, HttpServer, App, web, patch, middleware::Logger};
use log::info;
use tokio::sync::RwLock;

pub mod heater;
pub mod pc;
pub mod metrics;
pub mod heatman;

const CONFIG_PATH: &str = "heater_config.json";

#[cfg(debug_assertions)]
const PORT: u16 = 5568;
#[cfg(not(debug_assertions))]
const PORT: u16 = 5567;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    #[cfg(debug_assertions)]
    std::env::set_var("RUST_LOG", "actix_web=debug,actix_server=debug");
    #[cfg(not(debug_assertions))]
    std::env::set_var("RUST_LOG", "actix_web=info,actix_server=debug");
    env_logger::init();

    // Read config
    let config = 
        if Path::new(CONFIG_PATH).exists() { Some(fs::read_to_string(CONFIG_PATH)?) }
        else { None };
    let config = config.map_or(Config::default(), |s| serde_json::from_str(&s).unwrap());
    let config_data = web::Data::new(RwLock::new(config));

    // Start heater manager.
    heatman::start(config_data.clone());

    // Create and start server.
    HttpServer::new(move || {
        App::new()
            .app_data(config_data.clone())
            .wrap(Logger::default())
            .service(get_config_and_state)
            .service(patch_config)
    })
    .bind(("0.0.0.0", PORT))?
    .run()
    .await
}

#[get("/")]
async fn get_config_and_state(config: web::Data<RwLock<Config>>, query: web::Query<GetConfigAndStateQuery>) -> Result<impl Responder, Box<dyn std::error::Error>> {
    let config = config.read().await;
    send_config_and_state(if query.include_config.unwrap_or(false) { Some(&*config) } else { None }).await
}

#[patch("/")]
async fn patch_config(config: web::Data<RwLock<Config>>, new_config: web::Json<Config>) -> Result<impl Responder, Box<dyn std::error::Error>> {
    let new_config = new_config.into_inner();

    fs::write(CONFIG_PATH, serde_json::to_string(&new_config)?)?; // Write config

    // Update config
    let mut config = config.write().await;
    *config = new_config;
    info!("Updated config to {:?}", &new_config);

    heatman::check_heater(&new_config).await;

    send_config_and_state(Some(&new_config)).await
}

async fn send_config_and_state(config: Option<&Config>) -> Result<impl Responder, Box<dyn std::error::Error>> {
    // Get metrics
    let (temperature, co2) = metrics::get_temp_and_co2().await?;

    // Check if heater is on
    let is_heating = heater::is_on().await?;

    // Formulate response
    let resp = GetConfigAndStateResp {
        config: config.cloned(),
        temperature,
        co2,
        is_heating,
    };
    Ok(web::Json(resp))
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Clone, Copy, Debug)]
pub struct Config {
    master_switch: bool,
    force: bool,
    target_temp: f32,
    co2_target: Option<i32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            master_switch: true,
            force: false,
            target_temp: 18.0,
            co2_target: Some(500),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct GetConfigAndStateQuery {
    include_config: Option<bool>
}

#[derive(serde::Deserialize, serde::Serialize)]
struct GetConfigAndStateResp {
    config: Option<Config>,
    temperature: f32,
    co2: i32,
    is_heating: bool,
}
