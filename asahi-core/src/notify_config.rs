use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Toggle {
    pub enabled: bool,
}

impl Toggle {
    pub fn on() -> Self { Self { enabled: true } }
    pub fn off() -> Self { Self { enabled: false } }
    pub fn flip(&mut self) { self.enabled = !self.enabled; }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Threshold {
    pub enabled: bool,
    pub threshold: f64,
}

impl Threshold {
    pub fn new(t: f64) -> Self { Self { enabled: true, threshold: t } }
    pub fn bump(&mut self, delta: f64, min: f64, max: f64) {
        self.threshold = (self.threshold + delta).clamp(min, max);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct NotifyConfig {
    pub enabled: bool,
    pub battery_target_reached: Toggle,
    pub battery_low: Threshold,
    pub thermal_high: Threshold,
    pub fan_ramped_max: Toggle,
    pub charge_override_expired: Toggle,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            battery_target_reached: Toggle::on(),
            battery_low: Threshold::new(20.0),
            thermal_high: Threshold::new(80.0),
            fan_ramped_max: Toggle::on(),
            charge_override_expired: Toggle::off(),
        }
    }
}

pub fn config_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config/asahi-tools/notifications.json")
}

impl NotifyConfig {
    pub fn load() -> Self {
        match std::fs::read_to_string(config_path()) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }
    pub fn save(&self) -> std::io::Result<()> {
        let p = config_path();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(p, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
