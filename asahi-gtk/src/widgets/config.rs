use super::{default_layout, WidgetInstance, WidgetSize};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct DashboardConfig {
    pub version: u32,
    pub widgets: Vec<WidgetInstance>,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            version: 1,
            widgets: default_layout(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".config/asahi-tools")
}

pub fn config_path() -> PathBuf {
    config_dir().join("dashboard.json")
}

pub fn load() -> DashboardConfig {
    let path = config_path();
    if !path.exists() {
        return DashboardConfig::default();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return DashboardConfig::default(),
    };

    match serde_json::from_str::<DashboardConfig>(&content) {
        Ok(cfg) if cfg.version == 1 => {
            let filtered: Vec<WidgetInstance> = cfg
                .widgets
                .into_iter()
                .filter(|w| super::find_def(&w.widget_type).is_some())
                .collect();
            DashboardConfig {
                version: 1,
                widgets: filtered,
            }
        }
        _ => {
            let backup = config_dir().join("dashboard.json.bak");
            let _ = std::fs::copy(&path, &backup);
            DashboardConfig::default()
        }
    }
}

pub fn save(cfg: &DashboardConfig) -> std::io::Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = config_path();
    let tmp = config_dir().join("dashboard.json.tmp");
    let json = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn reset() -> DashboardConfig {
    let cfg = DashboardConfig::default();
    let _ = save(&cfg);
    cfg
}

#[allow(dead_code)]
pub fn normalize_sizes(cfg: &mut DashboardConfig) {
    for w in &mut cfg.widgets {
        if !matches!(
            w.size,
            WidgetSize::Small | WidgetSize::Medium | WidgetSize::Large
        ) {
            w.size = WidgetSize::Medium;
        }
    }
}
