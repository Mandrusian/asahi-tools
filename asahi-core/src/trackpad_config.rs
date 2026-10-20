use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct TrackpadConfig {
    pub enabled: bool,
    pub hard_press: i32,
    pub release: i32,
    pub observed_light_max: Option<i32>,
    pub observed_hard_max: Option<i32>,
    pub calibrated_at: Option<String>,
}

impl Default for TrackpadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hard_press: 650,
            release: 400,
            observed_light_max: None,
            observed_hard_max: None,
            calibrated_at: None,
        }
    }
}

/// Resolve the real user's home directory.
/// When running under `sudo`, `$HOME` is `/root` — but we want the
/// invoking user's home so configs are shared between the privileged
/// daemon and the unprivileged TUI/GUI.
pub fn user_home() -> PathBuf {
    if let Ok(sudo_user) = std::env::var("SUDO_USER") {
        if !sudo_user.is_empty() && sudo_user != "root" {
            if let Ok(passwd) = std::fs::read_to_string("/etc/passwd") {
                for line in passwd.lines() {
                    let fields: Vec<&str> = line.split(':').collect();
                    if fields.len() >= 6 && fields[0] == sudo_user {
                        if !fields[5].is_empty() {
                            return PathBuf::from(fields[5]);
                        }
                    }
                }
            }
        }
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_path() -> PathBuf {
    user_home().join(".config/asahi-tools/trackpad.json")
}

impl TrackpadConfig {
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
        std::fs::write(&p, serde_json::to_string_pretty(self)?)?;

        // If we're root via sudo, restore ownership to the invoking user
        // so the TUI (which runs as them) can read the file without sudo.
        if let Ok(sudo_uid) = std::env::var("SUDO_UID") {
            if let Ok(uid) = sudo_uid.parse::<u32>() {
                let gid = std::env::var("SUDO_GID")
                    .ok()
                    .and_then(|g| g.parse::<u32>().ok())
                    .unwrap_or(uid);
                let _ = std::process::Command::new("chown")
                    .arg(format!("{uid}:{gid}"))
                    .arg(&p)
                    .status();
                if let Some(parent) = p.parent() {
                    let _ = std::process::Command::new("chown")
                        .arg(format!("{uid}:{gid}"))
                        .arg(parent)
                        .status();
                }
            }
        }
        Ok(())
    }

    pub fn is_calibrated(&self) -> bool {
        self.calibrated_at.is_some()
            && self.observed_light_max.is_some()
            && self.observed_hard_max.is_some()
    }
}
