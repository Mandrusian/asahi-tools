use std::io::Write;
use std::path::PathBuf;
fn main() {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let db = PathBuf::from(home).join(".local/share/asahi-tools/sleep-log.jsonl");
    std::fs::create_dir_all(db.parent().unwrap()).ok();
    let cap = asahi_core::get_battery_capacity().ok();
    let temp = asahi_core::find_fan_hwmon().and_then(|h| {
        asahi_core::scan_temps(&h).iter().map(|t| t.celsius).fold(None, |a: Option<f64>, t| Some(a.map_or(t, |x| x.max(t))))
    });
    let entry = serde_json::json!({"ts": chrono::Local::now().to_rfc3339(), "capacity": cap, "max_temp": temp});
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&db) {
        writeln!(f, "{}", entry).ok();
    }
    println!("{}", entry);
}
