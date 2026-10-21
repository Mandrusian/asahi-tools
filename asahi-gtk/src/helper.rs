use std::path::PathBuf;
use std::process::Command;
use std::collections::VecDeque;
use gtk::cairo;

pub fn find_binary(name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    for c in [home.join(".local/bin").join(name), PathBuf::from("/usr/local/bin").join(name)] {
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

pub fn spawn_in_terminal(name: &str) {
    if let Some(bin) = find_binary(name) {
        for term in ["kgx", "konsole", "alacritty", "foot", "xterm"] {
            if Command::new(term).args(["-e", &bin.display().to_string()]).spawn().is_ok() { break; }
        }
    }
}

pub fn read_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn apple_sensor_name(raw: &str) -> Option<&'static str> {
    match raw {
        "Tp01" | "Tp05" | "Tp09" | "Tp0D" => Some("CPU Eff"),
        "Tp0H" | "Tp0L" | "Tp0P" => Some("CPU Perf"),
        "Tg05" | "Tg0b" | "Tg0h" => Some("GPU"),
        "TB0T" | "TB1T" | "TB2T" => Some("Battery"),
        "TN0D" | "TN0P" => Some("SSD"),
        _ => None,
    }
}

pub fn get_hwmon_sensors() -> Vec<(String, f64)> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") {
        for entry in entries.flatten() {
            let path = entry.path();
            let chip = read_file(&format!("{}/name", path.display())).unwrap_or_default();
            for i in 1..=30 {
                let input = format!("{}/temp{}_input", path.display(), i);
                if let Some(val) = read_file(&input) {
                    if let Ok(milli) = val.parse::<f64>() {
                        let mut label = read_file(&format!("{}/temp{}_label", path.display(), i))
                            .unwrap_or_else(|| format!("{} temp{}", chip, i));
                        if let Some(friendly) = apple_sensor_name(&label) { label = friendly.to_string(); }
                        else if label == "NAND" { label = "SSD".into(); }
                        out.push((label, milli / 1000.0));
                    }
                }
            }
        }
    }
    out
}

pub fn get_hwmon_fans() -> Vec<(String, u32, u32, u32)> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = read_file(&format!("{}/name", path.display())).unwrap_or_default();
            for i in 1..=10 {
                let input = format!("{}/fan{}_input", path.display(), i);
                if let Some(val) = read_file(&input) {
                    if let Ok(rpm) = val.parse::<u32>() {
                        let min = read_file(&format!("{}/fan{}_min", path.display(), i))
                            .and_then(|v| v.parse().ok()).unwrap_or(1000);
                        let max = read_file(&format!("{}/fan{}_max", path.display(), i))
                            .and_then(|v| v.parse().ok()).unwrap_or(6500);
                        out.push((format!("{} Fan {}", name, i), rpm, min, max));
                    }
                }
            }
        }
    }
    out
}

pub fn get_cpu_usage() -> Option<f64> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let line = stat.lines().next()?;
    let parts: Vec<f64> = line.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    if parts.len() < 4 { return None; }
    let idle = parts[3];
    let total: f64 = parts.iter().sum();
    if total <= 0.0 { return None; }
    Some(100.0 - (idle / total * 100.0))
}

pub fn get_memory_usage() -> Option<f64> {
    let mem = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = 0.0;
    let mut avail = 0.0;
    for line in mem.lines() {
        if let Some(v) = line.strip_prefix("MemTotal:") {
            total = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        }
        if let Some(v) = line.strip_prefix("MemAvailable:") {
            avail = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        }
    }
    if total <= 0.0 { return None; }
    Some(((total - avail) / total) * 100.0)
}

pub fn get_memory_gb() -> Option<(f64, f64)> {
    let mem = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = 0.0;
    let mut avail = 0.0;
    for line in mem.lines() {
        if let Some(v) = line.strip_prefix("MemTotal:") {
            total = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        }
        if let Some(v) = line.strip_prefix("MemAvailable:") {
            avail = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
        }
    }
    let to_gb = |kb: f64| kb / 1024.0 / 1024.0;
    Some((to_gb(total - avail), to_gb(total)))
}

pub fn get_uptime_secs() -> Option<u64> {
    read_file("/proc/uptime")?
        .split_whitespace().next()?.parse().ok()
}

pub fn get_bootloader() -> Option<String> {
    read_file("/proc/device-tree/chosen/asahi,uboot-version")
        .or_else(|| read_file("/sys/firmware/devicetree/base/chosen/asahi,uboot-version"))
}

pub fn get_kernel_version() -> String {
    read_file("/proc/sys/kernel/osrelease").unwrap_or_else(|| "Unknown".into())
}

pub fn get_wifi_signal() -> Option<f64> {
    if let Ok(out) = Command::new("nmcli").args(["-t", "-f", "IN-USE,SIGNAL", "dev", "wifi"]).output() {
        let txt = String::from_utf8_lossy(&out.stdout);
        for line in txt.lines() {
            if line.starts_with('*') {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 2 { return parts[1].parse().ok(); }
            }
        }
    }
    if let Some(content) = read_file("/proc/net/wireless") {
        for line in content.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let q = parts[3].trim_end_matches('.');
                if let Ok(v) = q.parse::<f64>() { return Some((v * 100.0 / 70.0).min(100.0)); }
            }
        }
    }
    None
}

pub fn get_power_draw() -> Option<f64> {
    read_file("/sys/class/power_supply/macsmc-battery/power_now")
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| v / 1_000_000.0)
}

pub fn get_platform_profile() -> Option<String> {
    read_file("/sys/firmware/acpi/platform_profile")
}

pub fn get_disk_usage() -> Option<f64> {
    let s = Command::new("df").args(["-B1", "/"]).output().ok()?;
    let txt = String::from_utf8_lossy(&s.stdout);
    let line = txt.lines().nth(1)?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 5 { return None; }
    let total: f64 = parts[1].parse().ok()?;
    let used: f64 = parts[2].parse().ok()?;
    if total <= 0.0 { return None; }
    Some(used / total * 100.0)
}

pub fn draw_graph(cr: &cairo::Context, w: i32, h: i32, data: &VecDeque<f64>, min: f64, max: f64, color: (f64, f64, f64)) {
    cr.set_source_rgb(0.04, 0.04, 0.04);
    cr.paint().unwrap();
    if data.len() < 2 {
        cr.set_source_rgb(0.4, 0.4, 0.4);
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(12.0);
        let text = "No data yet";
        if let Ok(extents) = cr.text_extents(text) {
            cr.move_to((w as f64 - extents.width()) / 2.0, (h as f64 - extents.height()) / 2.0 + extents.height());
            cr.show_text(text).unwrap();
        }
        return;
    }
    let range = (max - min).max(1.0);
    let step = w as f64 / (data.len() as f64 - 1.0);
    cr.set_source_rgb(color.0, color.1, color.2);
    cr.set_line_width(2.0);
    let mut first = true;
    for (i, &val) in data.iter().enumerate() {
        let x = i as f64 * step;
        let norm = (val - min) / range;
        let y = h as f64 - (norm.clamp(0.0, 1.0) * h as f64);
        if first { cr.move_to(x, y); first = false; } else { cr.line_to(x, y); }
    }
    let _ = cr.stroke();
}

pub fn draw_gauge(cr: &cairo::Context, w: i32, h: i32, pct: f64, color: (f64, f64, f64), label: &str) {
    let center = (w as f64 / 2.0, h as f64 / 2.0);
    let radius = (w.min(h) as f64 / 2.0) - 12.0;
    cr.set_line_width(8.0);
    cr.set_source_rgb(0.1, 0.1, 0.1);
    cr.arc(center.0, center.1, radius, 0.0, 2.0 * std::f64::consts::PI);
    let _ = cr.stroke();
    let sweep = 2.0 * std::f64::consts::PI * (pct / 100.0).clamp(0.0, 1.0);
    cr.set_source_rgb(color.0, color.1, color.2);
    cr.arc(center.0, center.1, radius, -std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2 + sweep);
    let _ = cr.stroke();
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.set_font_size(16.0);
    if let Ok(ext) = cr.text_extents(label) {
        cr.move_to(center.0 - ext.width() / 2.0, center.1 + ext.height() / 2.0);
        let _ = cr.show_text(label);
    }
}

pub fn unavailable_widget(reason: &str) -> gtk::Widget {
    use gtk::prelude::*;
    let label = gtk::Label::new(Some(&format!("Unavailable: {}", reason)));
    label.set_widget_name("unavailable-widget");
    label.add_css_class("dim-label");
    label.set_margin_top(24);
    label.set_margin_bottom(24);
    label.upcast()
}

pub fn read_cpu_raw() -> Option<(u64, u64)> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let line = stat.lines().next()?;
    let parts: Vec<u64> = line.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    if parts.len() < 4 { return None; }
    Some((parts[3], parts.iter().sum()))
}

pub fn draw_bars(cr: &cairo::Context, w: i32, h: i32, pct: f64, color: (f64, f64, f64)) {
    cr.set_source_rgb(0.04, 0.04, 0.04);
    cr.paint().unwrap();
    let bar_w = (w as f64) / 9.0;
    let gap = bar_w * 0.5;
    let start_x = (w as f64 - 4.0 * bar_w - 3.0 * gap) / 2.0;
    for i in 0..4 {
        let x = start_x + (i as f64) * (bar_w + gap);
        let level = (i + 1) as f64 / 4.0;
        let bar_h = h as f64 * 0.15 + (h as f64 * 0.7 * level);
        let y = h as f64 - bar_h - (h as f64 * 0.15);
        let threshold = level * 100.0;
        if pct >= threshold {
            cr.set_source_rgb(color.0, color.1, color.2);
        } else {
            cr.set_source_rgb(0.18, 0.18, 0.18);
        }
        cr.rectangle(x, y, bar_w, bar_h);
        let _ = cr.fill();
    }
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.set_font_size(11.0);
    let text = format!("{:.0}%", pct);
    if let Ok(ext) = cr.text_extents(&text) {
        cr.move_to((w as f64 - ext.width()) / 2.0, h as f64 - 2.0);
        let _ = cr.show_text(&text);
    }
}

pub struct WifiDetails {
    pub available: bool,
    pub ssid: String,
    pub ipv4: String,
    pub ipv6: String,
    pub mac: String,
    pub gateway: String,
    pub dns: String,
    pub bssid: String,
    pub signal: u32,
    pub speed: String,
}

impl Default for WifiDetails {
    fn default() -> Self {
        Self {
            available: false,
            ssid: "—".into(), ipv4: "—".into(), ipv6: "—".into(),
            mac: "—".into(), gateway: "—".into(), dns: "—".into(),
            bssid: "—".into(), signal: 0, speed: "—".into(),
        }
    }
}

fn nmcli_multiline(args: &[&str]) -> Vec<std::collections::HashMap<String, String>> {
    let out = Command::new("nmcli").arg("-m").arg("multiline").args(args).output().ok();
    let mut records = vec![];
    let mut cur = std::collections::HashMap::new();
    if let Some(o) = out {
        let txt = String::from_utf8_lossy(&o.stdout);
        for line in txt.lines() {
            if line.is_empty() {
                if !cur.is_empty() { records.push(std::mem::take(&mut cur)); }
            } else if let Some((k, v)) = line.split_once(':') {
                cur.insert(k.trim().to_string(), v.to_string());
            }
        }
        if !cur.is_empty() { records.push(cur); }
    }
    records
}

pub fn get_wifi_details() -> WifiDetails {
    let mut d = WifiDetails::default();
    let dev = Command::new("nmcli").args(["-t", "-f", "DEVICE,TYPE", "dev"]).output().ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines()
            .find_map(|l| {
                let (dev, ty) = l.split_once(':')?;
                (ty == "wifi").then(|| dev.to_string())
            }));
    let Some(dev) = dev else { return d; };
    d.available = true;

    for r in nmcli_multiline(&["-f", "ACTIVE,SSID,SIGNAL,RATE,BSSID", "dev", "wifi"]) {
        let active = r.get("ACTIVE").map(|v| v.contains('*')).unwrap_or(false);
        if active {
            d.ssid = r.get("SSID").cloned().unwrap_or_else(|| "—".into());
            d.signal = r.get("SIGNAL").and_then(|s| s.parse().ok()).unwrap_or(0);
            d.speed = r.get("RATE").cloned().unwrap_or_else(|| "—".into());
            d.bssid = r.get("BSSID").cloned().unwrap_or_else(|| "—".into());
            break;
        }
    }

    if let Ok(out) = Command::new("ip").args(["-j", "addr", "show", &dev]).output() {
        if let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            if let Some(iface) = arr.first() {
                d.mac = iface.get("address").and_then(|v| v.as_str()).unwrap_or("—").to_string();
                if let Some(addrs) = iface.get("addr_info").and_then(|v| v.as_array()) {
                    for a in addrs {
                        let fam = a.get("family").and_then(|v| v.as_str()).unwrap_or("");
                        let local = a.get("local").and_then(|v| v.as_str()).unwrap_or("");
                        if fam == "inet" && !local.is_empty() { d.ipv4 = local.to_string(); }
                        else if fam == "inet6" && !local.starts_with("fe80") && !local.is_empty() { d.ipv6 = local.to_string(); }
                    }
                }
            }
        }
    }

    if let Ok(out) = Command::new("ip").args(["-j", "route", "get", "1.1.1.1"]).output() {
        if let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            if let Some(obj) = arr.first() {
                d.gateway = obj.get("gateway").and_then(|v| v.as_str()).unwrap_or("—").to_string();
            }
        }
    }

    if let Ok(content) = std::fs::read_to_string("/etc/resolv.conf") {
        let mut list = vec![];
        for line in content.lines() {
            if let Some(rest) = line.trim().strip_prefix("nameserver ") {
                list.push(rest.trim().to_string());
            }
        }
        if !list.is_empty() { d.dns = list.join(", "); }
    }

    d
}
