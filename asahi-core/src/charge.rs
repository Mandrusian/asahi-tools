use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub const SOCKET: &str = "/run/asahi-charge.sock";

#[derive(Serialize, Debug)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Override {
        percent: u32,
        duration_secs: u64,
    },
    CancelOverride,
    SetTarget {
        target: u32,
        hysteresis: Option<u32>,
    },
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Status {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub capacity: Option<u32>,
    #[serde(default)]
    pub temp_c: Option<f64>,
    #[serde(default)]
    pub charging_enabled: bool,
    #[serde(default)]
    pub override_active: bool,
    #[serde(default)]
    pub override_remaining_secs: Option<u64>,
    #[serde(default)]
    pub effective_target: u32,
    #[serde(default)]
    pub config_target: u32,
    #[serde(default)]
    pub hysteresis: u32,
    #[serde(default)]
    pub max_temp_c: u32,
    #[serde(default)]
    pub last_action: String,
}

pub fn query(req: &Request) -> std::io::Result<Status> {
    let mut stream = UnixStream::connect(SOCKET)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    writeln!(stream, "{}", serde_json::to_string(req).unwrap())?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    serde_json::from_str(&line).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

pub fn status() -> Option<Status> {
    query(&Request::Status).ok().filter(|s| s.ok)
}

pub fn override_for(percent: u32, duration_secs: u64) -> std::io::Result<()> {
    query(&Request::Override {
        percent,
        duration_secs,
    })?;
    Ok(())
}

pub fn cancel_override() -> std::io::Result<()> {
    query(&Request::CancelOverride)?;
    Ok(())
}

pub fn set_target(target: u32, hysteresis: Option<u32>) -> std::io::Result<()> {
    query(&Request::SetTarget { target, hysteresis })?;
    Ok(())
}

pub fn fmt_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}
