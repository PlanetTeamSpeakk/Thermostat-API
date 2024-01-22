use std::{path::Path, fs, collections::HashMap, sync::Mutex};
use actix_web::{Responder, get, HttpServer, App, web, patch, middleware::Logger};
use const_format::concatcp;

const CONFIG_PATH: &str = "heater_config.json";
const PLUG_ENDPOINT: &str = "http://192.168.178.86/rpc/";
const STATUS_ENDPOINT: &str = concatcp!(PLUG_ENDPOINT, "Shelly.GetStatus");

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    #[cfg(debug_assertions)]
    std::env::set_var("RUST_LOG", "actix_web=debug,actix_server=debug");
    env_logger::init();

    // Read config
    let config = 
        if Path::new(CONFIG_PATH).exists() { Some(fs::read_to_string(CONFIG_PATH)?) }
        else { None };
    let config = config.map_or(Config::default(), |s| serde_json::from_str(&s).unwrap());
    let config_data = web::Data::new(Mutex::new(config));

    HttpServer::new(move || {
        App::new()
            .app_data(config_data.clone())
            .wrap(Logger::default())
            .service(get_config_and_state)
            .service(patch_config)
    })
    .bind(("0.0.0.0", 5567))?
    .run()
    .await
}

#[get("/")]
async fn get_config_and_state(config: web::Data<Mutex<Config>>, query: web::Query<GetConfigAndStateQuery>) -> Result<impl Responder, Box<dyn std::error::Error>> {
    let config = config.lock().unwrap();
    println!("Sending config: {:?}", *config);
    send_config_and_state(if query.include_config.unwrap_or(false) { Some(&*config) } else { None }).await
}

#[patch("/")]
async fn patch_config(config: web::Data<Mutex<Config>>, new_config: web::Json<Config>) -> Result<impl Responder, Box<dyn std::error::Error>> {
    let new_config = new_config.into_inner();

    fs::write(CONFIG_PATH, serde_json::to_string(&new_config)?)?;
    let mut config = config.lock().unwrap();
    *config = new_config;
    println!("Updated config to {:?}", *config);
    send_config_and_state(Some(&*config)).await
}

async fn send_config_and_state(config: Option<&Config>) -> Result<impl Responder, Box<dyn std::error::Error>> {
    // Get metrics
    let metrics = get_metrics().await?;
    let temperature = metrics.get("temperature").unwrap().parse().unwrap();
    let co2 = metrics.get("co2").unwrap().parse::<f32>().unwrap() as i32;

    // Check if heater is on
    let resp = reqwest::get(STATUS_ENDPOINT).await?.json::<serde_json::Map<String, serde_json::Value>>().await?;
    let is_heating = resp.get("switch:0").unwrap()
        .as_object().unwrap()
        .get("output").unwrap()
        .as_bool().unwrap();

    // Formulate response
    let resp = GetConfigAndStateResp {
        config: config.cloned(),
        temperature,
        co2,
        is_heating,
    };
    Ok(web::Json(resp))
}

async fn get_metrics() -> Result<HashMap<String, String>, reqwest::Error> {
    let resp = reqwest::get("http://localhost:8000").await?;
    let body = resp.text().await?;

    let metrics = body
        .lines()
        .filter(|line| !line.starts_with("#"))
        .map(|line| line.split(" ").collect::<Vec<_>>())
        .filter(|pair| pair.len() == 2)
        .map(|pair| (pair[0].to_string(), pair[1].to_string()))
        .collect();
    
    Ok(metrics)
}

#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Clone, Copy, Debug)]
struct Config {
    master_switch: bool,
    force: bool,
    target_temp: f32,
    co2_target: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            master_switch: true,
            force: false,
            target_temp: 28.0,
            co2_target: 500,
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
