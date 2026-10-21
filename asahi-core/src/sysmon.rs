use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub const SOCKET: &str = "/run/asahi-sysmon.sock";

#[derive(Serialize, Debug)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    SetFan { fan: u32, target: u32 },
    Max { fan: u32 },
    Auto { fan: u32 },
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct FanView {
    pub index: u32,
    pub rpm: u32,
    pub min: Option<u32>,
    pub max: Option<u32>,
    pub target: Option<u32>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Status {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default)]
    pub max_temp_c: f64,
    #[serde(default)]
    pub temps: Vec<(String, f64)>,
    #[serde(default)]
    pub fans: Vec<FanView>,
    #[serde(default)]
    pub message: Option<String>,
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

pub fn set_fan(fan: u32, target: u32) -> std::io::Result<()> {
    query(&Request::SetFan { fan, target })?;
    Ok(())
}

pub fn max_fan(fan: u32) -> std::io::Result<()> {
    query(&Request::Max { fan })?;
    Ok(())
}

pub fn auto_fan(fan: u32) -> std::io::Result<()> {
    query(&Request::Auto { fan })?;
    Ok(())
}
