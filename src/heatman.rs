use std::time::Duration;
use tokio::{sync::RwLock, time::interval};
use crate::{heater, metrics, pc, Config};

/// Starts the heater manager loop.
/// Spawns a new tokio task that periodically checks the heater status and turns it on or off.
pub fn start(config: actix_web::web::Data<RwLock<Config>>) {
    tokio::spawn(async move {
        println!("HeatMan started.");

        let mut interval = interval(Duration::from_secs(15));
        loop {
            interval.tick().await;

            let config = config.read().await;
            check_heater(&*config).await;
        }
    });
}

/// Compares the current heater status to the desired status and switches the heater if necessary.
pub async fn check_heater(config: &Config) {
    if !config.master_switch {
        return;
    }

    // Check if heater should be on.
    let should_be_on = should_be_on(config).await;
    if let Err(e) = should_be_on {
        println!("Error while checking if heater should be on: {}", e);
        return;
    }
    let should_be_on = should_be_on.unwrap();

    // Check if heater is on.
    let is_on = heater::is_on().await;
    if let Err(e) = is_on {
        println!("Error while checking if heater is on: {}", e);
        return;
    }
    let is_on = is_on.unwrap();

    // Switch heater if necessary.
    if should_be_on != is_on {
        let switch_result = heater::switch(should_be_on).await;
        if let Err(e) = switch_result {
            println!("Error while switching heater: {}", e);
            return;
        }

        println!("Switched heater {}", if should_be_on { "on" } else { "off" });
    }
}

/// Determines whether the heater should be on.
async fn should_be_on(config: &Config) -> Result<bool, Box<dyn std::error::Error>> {
    if config.force {
        return Ok(true);
    }

    // Acquire data that will determine whether the heater should be on.
    let (temperature, co2) = metrics::get_temp_and_co2().await?;
    let pc_on = pc::is_on().await?;
    let pc_locked = pc::is_locked().await;

    // If the temperature is below the target temperature, the CO2 level is at least the minimum,
    // and the PC is on and not locked, the heater should be on.
    let should_be_on = temperature < config.target_temp && 
        (config.co2_target.is_none() || co2 >= config.co2_target.unwrap()) &&
        pc_on &&
        !pc_locked.to_bool().unwrap_or(false); // Assume it's not locked if request failed.

    Ok(should_be_on)
}
