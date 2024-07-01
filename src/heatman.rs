use std::time::Duration;
use log::{error, info};
use tokio::{sync::RwLock, time::interval};
use crate::{heater, metrics, pc, Config};

/// Starts the heater manager loop.
/// Spawns a new tokio task that periodically checks the heater status and turns it on or off.
pub fn start(config: actix_web::web::Data<RwLock<Config>>) {
    tokio::spawn(async move {
        info!("HeatMan started.");

        let mut interval = interval(Duration::from_secs(15));
        loop {
            interval.tick().await;

            let config = config.read().await;
            let res = check_heater(&config).await;
            if let Err(err) = res {
                error!("Error in heater manager: {}", err);
                error!("Backtrace:\n{}", err.backtrace());
            }
        }
    });
}

/// Compares the current heater status to the desired status and switches the heater if necessary.
pub async fn check_heater(config: &Config) -> anyhow::Result<()> {
    if !config.master_switch {
        return Ok(()); // Master switch is off, do nothing.
    }

    // Check if heater should be on.
    let should_be_on = should_be_on(config).await?;

    // Check if heater is on.
    let is_on = heater::is_on().await?;

    // Switch heater if necessary.
    if should_be_on != is_on {
        heater::switch(should_be_on).await?;
        info!("Switched heater {}", if should_be_on { "on" } else { "off" });
    }
    
    Ok(())
}

/// Determines whether the heater should be on.
async fn should_be_on(config: &Config) -> anyhow::Result<bool> {
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
