use std::{net::{IpAddr, Ipv4Addr}, time::Duration, str::FromStr};
use const_format::concatcp;
use ping::dgramsock::ping; // TODO: we should use dgramsock::ping here as rawsock requires sudo, but dgramsock seems to be broken. 
use tristate::TriState;

const PC_IP_STR: &str = "192.168.178.89"; // Local static IP of my PC.
const WINLOCK_ENDPOINT: &str = concatcp!("http://", PC_IP_STR, ":26969/"); // WinLock server on my PC (https://github.com/PlanetTeamSpeakk/WinLockServer)

/// Returns whether the PC is currently on.
pub async fn is_on() -> Result<bool, ping::Error> {
    let ip = IpAddr::V4(Ipv4Addr::from_str(PC_IP_STR).unwrap());
    let res = ping(ip, Some(Duration::from_secs(1)), None, None, None, None);

    if let Err(err) = res {
        if let ping::Error::IoError { error: _ } = err {
            Ok(false) // PC is off (timeout).
        } else {
            Err(err) // Some unexpected error occured.
        }
    } else {
        Ok(true) // PC is on.
    }
}

/// Returns whether the PC is currently locked.
/// Returns `TriState::Unknown` if the request failed.
pub async fn is_locked() -> TriState {
    let resp = reqwest::get(WINLOCK_ENDPOINT).await;
    if resp.is_err() {
        return TriState::Unknown;
    }

    let body = resp.unwrap().text().await;
    if body.is_err() {
        return TriState::Unknown;
    }

    // 1 = locked, 0 = unlocked
    let is_locked = body.unwrap() == "1";
    TriState::from(is_locked)
}
