use std::path::PathBuf;

pub fn db_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local/share/asahi-tools/battery-health.sqlite")
}

pub fn load(limit: usize) -> Vec<u32> {
    let db = db_path();
    if !db.exists() {
        return Vec::new();
    }
    let sql = format!(
        "SELECT capacity FROM samples WHERE capacity IS NOT NULL ORDER BY ts DESC LIMIT {limit}"
    );
    let out = std::process::Command::new("sqlite3")
        .arg(&db)
        .arg(&sql)
        .output();
    let text = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return Vec::new(),
    };
    let mut v: Vec<u32> = text.lines().filter_map(|l| l.trim().parse().ok()).collect();
    v.reverse();
    v
}
