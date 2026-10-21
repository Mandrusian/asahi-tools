use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub const SOCKET: &str = "/run/asahi-priv.sock";

#[derive(Serialize, Debug)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    GetKbd,
    SetKbd { value: u32 },
    GetDisplay,
    SetDisplay { value: u32 },
    GetProfile,
    SetProfile { profile: String },
    RestartModule { module: String },
    GetFnMode,
    SetFnMode { value: u32 },
    ServiceAction { name: String, action: String },
    ServiceStatus { name: String },
    AlsStatus,
    AlsSetEnabled { enabled: bool },
    AlsManualOverride,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct Response {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub current: Option<u32>,
    #[serde(default)]
    pub max: Option<u32>,
    #[serde(default)]
    pub value: Option<u32>,
    #[serde(default)]
    pub kbd: Option<u32>,
    #[serde(default)]
    pub kbd_max: Option<u32>,
    #[serde(default)]
    pub display: Option<u32>,
    #[serde(default)]
    pub display_max: Option<u32>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub fnmode: Option<u32>,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub lux: Option<f64>,
    #[serde(default)]
    pub override_active: Option<bool>,
    #[serde(default)]
    pub override_remaining_secs: Option<i64>,
    #[serde(default)]
    pub sensor_path: Option<String>,
    #[serde(default)]
    pub sensor_available: Option<bool>,
}

pub fn query(req: &Request) -> Option<Response> {
    let mut stream = UnixStream::connect(SOCKET).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    writeln!(stream, "{}", serde_json::to_string(req).ok()?).ok()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}

pub fn is_running() -> bool {
    UnixStream::connect(SOCKET).is_ok()
}

pub fn set_kbd(value: u32) -> Result<u32, String> {
    let r = query(&Request::SetKbd { value }).ok_or("daemon not running")?;
    if r.ok {
        Ok(r.value.unwrap_or(value))
    } else {
        Err(r.error.unwrap_or_default())
    }
}

pub fn get_kbd() -> Option<(u32, u32)> {
    let r = query(&Request::GetKbd)?;
    if r.ok {
        Some((r.current?, r.max?))
    } else {
        None
    }
}

pub fn set_display(value: u32) -> Result<u32, String> {
    let r = query(&Request::SetDisplay { value }).ok_or("daemon not running")?;
    if r.ok {
        Ok(r.value.unwrap_or(value))
    } else {
        Err(r.error.unwrap_or_default())
    }
}

pub fn get_display() -> Option<(u32, u32)> {
    let r = query(&Request::GetDisplay)?;
    if r.ok {
        Some((r.current?, r.max?))
    } else {
        None
    }
}

pub fn set_profile(profile: &str) -> Result<(), String> {
    let r = query(&Request::SetProfile {
        profile: profile.to_string(),
    })
    .ok_or("daemon not running")?;
    if r.ok {
        Ok(())
    } else {
        Err(r.error.unwrap_or_default())
    }
}

pub fn get_profile() -> Option<String> {
    query(&Request::GetProfile)?.profile
}

pub fn restart_module(module: &str) -> Result<(), String> {
    let r = query(&Request::RestartModule {
        module: module.to_string(),
    })
    .ok_or("daemon not running")?;
    if r.ok {
        Ok(())
    } else {
        Err(r.error.unwrap_or_default())
    }
}

pub fn set_fnmode(value: u32) -> Result<(), String> {
    let r = query(&Request::SetFnMode { value }).ok_or("daemon not running")?;
    if r.ok {
        Ok(())
    } else {
        Err(r.error.unwrap_or_default())
    }
}

pub fn get_fnmode() -> Option<u32> {
    query(&Request::GetFnMode)?.fnmode
}

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub active: bool,
    pub enabled: bool,
    pub detail: String,
}

pub fn service_status(name: &str) -> Option<ServiceInfo> {
    let r = query(&Request::ServiceStatus {
        name: name.to_string(),
    })?;
    if !r.ok {
        return None;
    }
    Some(ServiceInfo {
        active: r.active.unwrap_or(false),
        enabled: r.enabled.unwrap_or(false),
        detail: r.detail.unwrap_or_default(),
    })
}

pub fn service_action(name: &str, action: &str) -> Result<(), String> {
    let r = query(&Request::ServiceAction {
        name: name.to_string(),
        action: action.to_string(),
    })
    .ok_or("daemon not running")?;
    if r.ok {
        Ok(())
    } else {
        Err(r.error.unwrap_or_default())
    }
}

#[derive(Debug, Clone)]
pub struct AlsStatus {
    pub enabled: bool,
    pub lux: Option<f64>,
    pub override_active: bool,
    pub override_remaining_secs: i64,
    pub sensor_available: bool,
}

pub fn als_status() -> Option<AlsStatus> {
    let r = query(&Request::AlsStatus)?;
    if !r.ok {
        return None;
    }
    Some(AlsStatus {
        enabled: r.enabled.unwrap_or(false),
        lux: r.lux,
        override_active: r.override_active.unwrap_or(false),
        override_remaining_secs: r.override_remaining_secs.unwrap_or(0),
        sensor_available: r.sensor_available.unwrap_or(false),
    })
}

pub fn als_set_enabled(enabled: bool) -> Result<(), String> {
    let r = query(&Request::AlsSetEnabled { enabled }).ok_or("daemon not running")?;
    if r.ok {
        Ok(())
    } else {
        Err(r.error.unwrap_or_default())
    }
}

pub fn als_manual_override() -> Result<(), String> {
    let r = query(&Request::AlsManualOverride).ok_or("daemon not running")?;
    if r.ok {
        Ok(())
    } else {
        Err(r.error.unwrap_or_default())
    }
}
