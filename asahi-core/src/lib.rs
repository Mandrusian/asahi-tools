use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AsahiError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sysfs path not found: {0}")]
    PathNotFound(String),
    #[error("invalid value: {0}")]
    InvalidValue(String),
}

// ---------- keyboard backlight ----------
pub const KBD_BASE: &str = "/sys/class/leds/kbd_backlight";

pub fn get_kbd_brightness() -> Result<u32, AsahiError> {
    read_u32(&format!("{KBD_BASE}/brightness"))
}

pub fn get_kbd_max_brightness() -> Result<u32, AsahiError> {
    read_u32(&format!("{KBD_BASE}/max_brightness"))
}

pub fn set_kbd_brightness(value: u32) -> Result<(), AsahiError> {
    let max = get_kbd_max_brightness()?;
    if value > max {
        return Err(AsahiError::InvalidValue(format!("{value} > max {max}")));
    }
    fs::write(format!("{KBD_BASE}/brightness"), value.to_string())?;
    Ok(())
}

// ---------- battery ----------
pub const BATTERY_BASE: &str = "/sys/class/power_supply/macsmc-battery";

pub fn get_charge_thresholds() -> Result<(u32, u32), AsahiError> {
    let start = read_u32(&format!("{BATTERY_BASE}/charge_control_start_threshold"))?;
    let end = read_u32(&format!("{BATTERY_BASE}/charge_control_end_threshold"))?;
    Ok((start, end))
}

pub fn set_charge_thresholds(start: u32, end: u32) -> Result<(), AsahiError> {
    if start >= end {
        return Err(AsahiError::InvalidValue("start must be < end".into()));
    }
    if end > 100 {
        return Err(AsahiError::InvalidValue("end must be <= 100".into()));
    }
    fs::write(
        format!("{BATTERY_BASE}/charge_control_start_threshold"),
        start.to_string(),
    )?;
    fs::write(
        format!("{BATTERY_BASE}/charge_control_end_threshold"),
        end.to_string(),
    )?;
    Ok(())
}

pub fn get_battery_capacity() -> Result<u32, AsahiError> {
    read_u32(&format!("{BATTERY_BASE}/capacity"))
}

pub fn get_battery_status() -> Result<String, AsahiError> {
    let s = fs::read_to_string(format!("{BATTERY_BASE}/status"))?;
    Ok(s.trim().to_string())
}

// ---------- hwmon / fans ----------
pub const HWMON_BASE: &str = "/sys/class/hwmon";

pub fn find_fan_hwmon() -> Option<String> {
    for entry in fs::read_dir(HWMON_BASE).ok()? {
        let path = entry.ok()?.path();
        let name = fs::read_to_string(path.join("name")).ok()?;
        if name.trim() == "macsmc_hwmon" {
            return Some(path.display().to_string());
        }
    }
    None
}

pub fn read_hwmon_raw(dir: &str, attr: &str) -> Result<String, AsahiError> {
    let p = Path::new(dir).join(attr);
    fs::read_to_string(&p)
        .map(|s| s.trim().to_string())
        .map_err(|_| AsahiError::PathNotFound(p.display().to_string()))
}

pub fn read_hwmon_i64(dir: &str, attr: &str) -> Result<i64, AsahiError> {
    let s = read_hwmon_raw(dir, attr)?;
    s.parse()
        .map_err(|e| AsahiError::InvalidValue(format!("{attr}: {e}")))
}

pub fn read_hwmon_f64(dir: &str, attr: &str) -> Result<f64, AsahiError> {
    let s = read_hwmon_raw(dir, attr)?;
    s.parse()
        .map_err(|e| AsahiError::InvalidValue(format!("{attr}: {e}")))
}

pub fn write_hwmon_raw(dir: &str, attr: &str, value: &str) -> Result<(), AsahiError> {
    fs::write(Path::new(dir).join(attr), value)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct TempSensor {
    pub name: String,
    pub celsius: f64,
}

#[derive(Debug, Clone)]
pub struct Fan {
    pub index: u32,
    pub rpm: u32,
    pub min: Option<u32>,
    pub max: Option<u32>,
    pub target: Option<u32>,
}

pub fn scan_temps(hwmon: &str) -> Vec<TempSensor> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(hwmon) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("temp") && name.ends_with("_input") {
                if let Ok(milli) = read_hwmon_i64(hwmon, &name) {
                    let short = name.trim_end_matches("_input").to_string();
                    out.push(TempSensor {
                        name: short,
                        celsius: milli as f64 / 1000.0,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn scan_fans(hwmon: &str) -> Vec<Fan> {
    let mut out = Vec::new();
    for idx in 1..=8 {
        let input = format!("fan{idx}_input");
        let rpm = match read_hwmon_i64(hwmon, &input) {
            Ok(v) => v as u32,
            Err(_) => continue,
        };
        let min = read_hwmon_i64(hwmon, &format!("fan{idx}_min"))
            .ok()
            .map(|v| v as u32);
        let max = read_hwmon_i64(hwmon, &format!("fan{idx}_max"))
            .ok()
            .map(|v| v as u32);
        let target = read_hwmon_i64(hwmon, &format!("fan{idx}_target"))
            .ok()
            .map(|v| v as u32);
        out.push(Fan {
            index: idx,
            rpm,
            min,
            max,
            target,
        });
    }
    out
}

// ---------- internal helpers ----------
fn read_u32(path: &str) -> Result<u32, AsahiError> {
    let s = fs::read_to_string(path).map_err(|_| AsahiError::PathNotFound(path.to_string()))?;
    s.trim()
        .parse()
        .map_err(|e| AsahiError::InvalidValue(format!("{path}: {e}")))
}
