use std::collections::HashMap;

/// Returns the current temperature and CO2 level from the metrics server.
pub async fn get_temp_and_co2() -> Result<(f32, i32), Box<dyn std::error::Error>> {
    let metrics = get_metrics().await?;
    let temperature = metrics.get("temperature").unwrap().parse::<f32>()?;
    let co2 = metrics.get("co2").unwrap().parse::<f32>()? as i32;
    Ok((temperature, co2))
}

/// Returns a map of all metrics from the Prometheus exporter running on the local machine.
/// Metrics are provided by https://github.com/PlanetTeamSpeakk/Metrics
pub async fn get_metrics() -> Result<HashMap<String, String>, reqwest::Error> {
    let resp = reqwest::get("http://localhost:8000").await?;
    let body = resp.text().await?;

    let metrics = body
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split(' ').collect::<Vec<_>>())
        .filter(|pair| pair.len() == 2)
        .map(|pair| (pair[0].to_string(), pair[1].to_string()))
        .collect();
    
    Ok(metrics)
}
