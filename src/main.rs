use std::{path::Path, fs};
use actix_web::{Responder, get, HttpServer, App, web, patch, middleware::Logger};
use log::{error, info};
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
async fn main() -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    std::env::set_var("RUST_LOG", "debug");
    #[cfg(not(debug_assertions))]
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    // Read config
    let config = 
        if Path::new(CONFIG_PATH).exists() { Some(fs::read_to_string(CONFIG_PATH)?) }
        else { None };
    let config = config.map_or(Config::default(), |s| serde_json::from_str(&s).unwrap());
    let state = State { config, available: true };
    let state_data = web::Data::new(RwLock::new(state));

    // Start heater manager.
    heatman::start(state_data.clone());

    // Create and start server.
    HttpServer::new(move || {
        App::new()
            .app_data(state_data.clone())
            .wrap(Logger::default())
            .service(get_config_and_state)
            .service(patch_config)
    })
    .bind(("0.0.0.0", PORT))?
    .run()
    .await?;
    
    Ok(())
}

#[get("/")]
async fn get_config_and_state(state: web::Data<RwLock<State>>, query: web::Query<Query>) -> Result<impl Responder> {
    let state = state.read().await;
    send_config_and_state(&state, query.include_config.unwrap_or(false)).await
}

#[patch("/")]
async fn patch_config(state: web::Data<RwLock<State>>, new_config: web::Json<Config>) -> Result<impl Responder> {
    let new_config = new_config.into_inner();

    fs::write(CONFIG_PATH, serde_json::to_string(&new_config)?)?; // Write config

    // Update config
    state.write().await.config = new_config;
    info!("Updated config to {:?}", &new_config);

    heatman::check_heater(&new_config).await?;

    send_config_and_state(&*state.read().await, true).await
}

async fn send_config_and_state(state: &State, include_config: bool) -> Result<impl Responder> {
    // Get metrics
    let (temperature, co2) = metrics::get_temp_and_co2().await?;

    // Check if heater is on
    let is_heating = if state.available { heater::is_on().await? } else { false };

    // Formulate response
    let resp = Response {
        success: true,
        data: Some(ResponseData {
            config: if include_config { Some(state.config) } else { None },
            available: state.available,
            state: ResponseStateData {
                temperature,
                co2,
                is_heating,
            }
        }),
        error: None,
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
struct Query {
    include_config: Option<bool>
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Response {
    success: bool,
    data: Option<ResponseData>,
    error: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ResponseData {
    config: Option<Config>,
    available: bool,
    state: ResponseStateData,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct ResponseStateData {
    temperature: f32,
    co2: i32,
    is_heating: bool,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct State {
    config: Config,
    available: bool,
}

pub type Result<T> = std::result::Result<T, AHError>;

#[derive(thiserror::Error, Debug)]
pub enum AHError {
    #[error("an unknown error occurred: {0}")]
    AnyError(#[from] anyhow::Error),
    #[error("an unknown error occurred during (de)serialization: {0}")]
    SerdeError(#[from] serde_json::Error),
    #[error("an unknown IO error occurred: {0}")]
    IOError(#[from] std::io::Error),
}

impl actix_web::ResponseError for AHError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
    }
    
    fn error_response(&self) -> actix_web::HttpResponse {
        error!("Error on request: {:?}", self);
        actix_web::HttpResponse::InternalServerError()
            .json(Response {
                success: false,
                data: None,
                error: Some(self.to_string()),
            })
    }
}
