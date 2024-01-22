use const_format::concatcp;

const PLUG_ENDPOINT: &str = "http://192.168.178.86/rpc/"; // RPC endpoint of my Shelly Plus Plug S managing power to my heater.
const STATUS_ENDPOINT: &str = concatcp!(PLUG_ENDPOINT, "Shelly.GetStatus"); // Status endpoint of the plug.
const SWITCH_SET_ENDPOINT: &str = concatcp!(PLUG_ENDPOINT, "Switch.Set"); // Set endpoint of the plug. Used to turn the heater on and off.

/// Returns whether the heater is currently on.
pub async fn is_on() -> Result<bool, reqwest::Error> {
    let resp = reqwest::get(STATUS_ENDPOINT)
        .await?
        .json::<serde_json::Map<String, serde_json::Value>>()
        .await?;
    
    let is_heating = resp.get("switch:0").unwrap()
        .as_object().unwrap()
        .get("output").unwrap()
        .as_bool().unwrap();
    Ok(is_heating)
}

/// Turns the heater on or off.
pub async fn switch(on: bool) -> Result<(), reqwest::Error> {
    reqwest::get(format!("{}?id=0&on={}", SWITCH_SET_ENDPOINT, on)).await?;
    Ok(())
}
