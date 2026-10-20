use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use asahi_core::{get_battery_capacity, get_battery_status, find_fan_hwmon, scan_temps, scan_fans};

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_search_enabled(true);

    let status = adw::PreferencesGroup::builder().title("Status").build();

    let bat = adw::ActionRow::builder().title("Battery").subtitle("…").build();
    status.add(&bat);
    let therm = adw::ActionRow::builder().title("Thermal").subtitle("…").build();
    status.add(&therm);
    let fan = adw::ActionRow::builder().title("Fan 1").subtitle("…").build();
    status.add(&fan);
    let mem = adw::ActionRow::builder().title("Memory").subtitle("…").build();
    status.add(&mem);

    page.add(&status);

    {
        let bat = bat.clone();
        let therm = therm.clone();
        let fan = fan.clone();
        let mem = mem.clone();
        glib::timeout_add_seconds_local(2, move || {
            let pct = get_battery_capacity().unwrap_or(0);
            let st = get_battery_status().unwrap_or_default();
            bat.set_subtitle(&format!("{}% · {}", pct, st));

            if let Some(h) = find_fan_hwmon() {
                let temps = scan_temps(&h);
                let fans = scan_fans(&h);
                let max_t = temps.iter().map(|t| t.celsius).fold(0.0_f64, f64::max);
                therm.set_subtitle(&format!("{:.1} °C", max_t));
                if let Some(f) = fans.first() {
                    fan.set_subtitle(&format!("{} rpm{}", f.rpm,
                        f.target.map(|t| format!(" (target {})", t)).unwrap_or_default()));
                }
            }
            if let Ok(txt) = std::fs::read_to_string("/proc/meminfo") {
                let mut total = 0u64; let mut avail = 0u64;
                for line in txt.lines() {
                    if let Some(r) = line.strip_prefix("MemTotal:") {
                        total = r.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
                    }
                    if let Some(r) = line.strip_prefix("MemAvailable:") {
                        avail = r.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
                    }
                }
                if total > 0 {
                    let used = (total - avail) as f64 / (1024.0 * 1024.0);
                    let tot = total as f64 / (1024.0 * 1024.0);
                    mem.set_subtitle(&format!("{:.1} / {:.1} GiB", used, tot));
                }
            }
            glib::ControlFlow::Continue
        });
    }

    page
}
