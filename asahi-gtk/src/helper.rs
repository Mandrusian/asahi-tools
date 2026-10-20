use std::path::PathBuf;
use std::process::Command;

pub fn find_binary(name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let here = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    for c in [
        here.join(name),
        home.join(".local/share/asahi-tools/bin").join(name),
        home.join(".local/bin").join(name),
        home.join("asahi-tools/target/release").join(name),
        home.join("asahi-tools/target/debug").join(name),
        home.join("code/asahi-tools/target/release").join(name),
        home.join("Projects/asahi-tools/target/release").join(name),
        PathBuf::from("target/release").join(name),
        PathBuf::from("target/debug").join(name),
    ] {
        if c.exists() { return Some(c); }
    }
    if let Ok(o) = Command::new("which").arg(name).output() {
        if o.status.success() {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !s.is_empty() { return Some(PathBuf::from(s)); }
        }
    }
    None
}

/// Launch a tool in a terminal (kgx/konsole/xterm fallback chain).
pub fn spawn_in_terminal(name: &str) -> bool {
    let bin = match find_binary(name) {
        Some(b) => b,
        None => return false,
    };
    for term in ["kgx", "konsole", "alacritty", "kitty", "foot", "xterm"] {
        if Command::new(which(term).as_deref().unwrap_or(term))
            .args(if term == "kgx" { vec!["-e".to_string(), bin.display().to_string()] }
                  else if term == "konsole" { vec!["-e".to_string(), bin.display().to_string()] }
                  else { vec!["-e".to_string(), bin.display().to_string()] })
            .spawn()
            .is_ok()
        {
            return true;
        }
    }
    false
}

fn which(name: &str) -> Option<String> {
    Command::new("which").arg(name).output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn read_sysfs(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// Spawn with pkexec (GUI password prompt).
pub fn spawn_pkexec(bin: &str, args: &[&str]) -> bool {
    let b = match find_binary(bin) {
        Some(b) => b,
        None => return false,
    };
    Command::new("pkexec").arg(b).args(args).spawn().is_ok()
}
