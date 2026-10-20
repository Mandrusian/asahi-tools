use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use asahi_core::{get_battery_capacity, get_battery_status, find_fan_hwmon, scan_temps, scan_fans};

const WIDGETS: &[&str] = &["battery", "thermal", "fans", "memory", "power", "actions"];

fn load_widget_config() -> Vec<String> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let p = home.join(".config/asahi-tools/dashboard.json");
    if let Ok(txt) = std::fs::read_to_string(&p) {
        #[derive(serde::Deserialize)]
        struct Cfg { widgets: Vec<String> }
        if let Ok(c) = serde_json::from_str::<Cfg>(&txt) {
            return c.widgets;
        }
    }
    WIDGETS.iter().map(|s| s.to_string()).collect()
}

fn widget_enabled(name: &str, cfg: &[String]) -> bool {
    cfg.iter().any(|w| w == name)
}

fn read_mem() -> (f64, f64) {
    if let Ok(t) = std::fs::read_to_string("/proc/meminfo") {
        let mut total = 0u64; let mut avail = 0u64;
        for l in t.lines() {
            if let Some(r) = l.strip_prefix("MemTotal:") {
                total = r.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
            }
            if let Some(r) = l.strip_prefix("MemAvailable:") {
                avail = r.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
            }
        }
        let f = |kb: u64| kb as f64 / 1024.0 / 1024.0;
        return (f(total.saturating_sub(avail)), f(total));
    }
    (0.0, 0.0)
}

fn read_power_rails() -> Vec<(String, f64)> {
    let hw = match find_fan_hwmon() { Some(h) => h, None => return Vec::new() };
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&hw) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("power") && n.ends_with("_input") {
                if let Ok(s) = std::fs::read_to_string(e.path()) {
                    if let Ok(v) = s.trim().parse::<f64>() {
                        out.push((n.trim_end_matches("_input").to_string(), v / 1_000_000.0));
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn battery_color(pct: u32) -> &'static str {
    if pct < 20 { "error" } else if pct < 40 { "warning" } else { "success" }
}

fn temp_color(c: f64) -> &'static str {
    if c > 80.0 { "error" } else if c > 60.0 { "warning" } else { "success" }
}

fn add_battery_card(page: &adw::PreferencesPage) {
    let group = adw::PreferencesGroup::builder().title("Battery").build();
    let row = adw::ActionRow::builder().title("Charge").subtitle("…").build();
    let pct_lbl = gtk::Label::new(Some("—"));
    pct_lbl.add_css_class("title-1");
    pct_lbl.set_valign(gtk::Align::Center);
    row.add_suffix(&pct_lbl);
    group.add(&row);

    let bar_row = adw::ActionRow::builder().title("").build();
    let bar = gtk::LevelBar::new();
    bar.set_min_value(0.0);
    bar.set_max_value(100.0);
    bar.set_hexpand(true);
    bar.set_valign(gtk::Align::Center);
    bar.set_size_request(-1, 8);
    bar_row.add_suffix(&bar);
    bar_row.set_activatable(false);
    group.add(&bar_row);

    page.add(&group);

    let row_c = row.clone(); let pct_c = pct_lbl.clone(); let bar_c = bar.clone();
    glib::timeout_add_seconds_local(2, move || {
        let pct = get_battery_capacity().unwrap_or(0);
        let st = get_battery_status().unwrap_or_default();
        row_c.set_subtitle(&st);
        pct_c.set_text(&format!("{}%", pct));
        bar_c.set_value(pct as f64);
        let cls = battery_color(pct);
        for c in ["success", "warning", "error"] { pct_c.remove_css_class(c); }
        pct_c.add_css_class(cls);
        glib::ControlFlow::Continue
    });
}

fn add_thermal_card(page: &adw::PreferencesPage) {
    let group = adw::PreferencesGroup::builder().title("Thermal").build();
    let rows: Vec<(adw::ActionRow, gtk::LevelBar, gtk::Label)> = (0..6).map(|_| {
        let row = adw::ActionRow::builder().title("—").build();
        let bar = gtk::LevelBar::new();
        bar.set_min_value(0.0);
        bar.set_max_value(100.0);
        bar.set_hexpand(true);
        bar.set_valign(gtk::Align::Center);
        bar.set_size_request(200, 8);
        let lbl = gtk::Label::new(Some("—"));
        lbl.set_valign(gtk::Align::Center);
        lbl.add_css_class("dim-label");
        row.add_suffix(&bar);
        row.add_suffix(&lbl);
        group.add(&row);
        (row, bar, lbl)
    }).collect();
    page.add(&group);

    let rows_c: Vec<_> = rows.into_iter().map(|(r, b, l)| (r, b, l)).collect();
    glib::timeout_add_seconds_local(2, move || {
        if let Some(h) = find_fan_hwmon() {
            let temps = scan_temps(&h);
            let mut sorted = temps.clone();
            sorted.sort_by(|a, b| b.celsius.partial_cmp(&a.celsius).unwrap_or(std::cmp::Ordering::Equal));
            for (i, (row, bar, lbl)) in rows_c.iter().enumerate() {
                if let Some(t) = sorted.get(i) {
                    row.set_title(&t.name);
                    row.set_visible(true);
                    bar.set_value(t.celsius.min(100.0));
                    lbl.set_text(&format!("{:.1} °C", t.celsius));
                    let cls = temp_color(t.celsius);
                    for c in ["success", "warning", "error"] { bar.remove_css_class(c); }
                    bar.add_css_class(cls);
                } else {
                    row.set_visible(false);
                }
            }
        }
        glib::ControlFlow::Continue
    });
}

fn add_fans_card(page: &adw::PreferencesPage) {
    let group = adw::PreferencesGroup::builder().title("Fans").build();
    let rows: Vec<(adw::ActionRow, gtk::LevelBar, gtk::Label)> = (0..4).map(|_| {
        let row = adw::ActionRow::builder().title("—").build();
        let bar = gtk::LevelBar::new();
        bar.set_min_value(0.0);
        bar.set_max_value(1.0);
        bar.set_hexpand(true);
        bar.set_valign(gtk::Align::Center);
        bar.set_size_request(200, 8);
        let lbl = gtk::Label::new(Some("—"));
        lbl.set_valign(gtk::Align::Center);
        lbl.add_css_class("dim-label");
        row.add_suffix(&bar);
        row.add_suffix(&lbl);
        group.add(&row);
        (row, bar, lbl)
    }).collect();
    page.add(&group);

    glib::timeout_add_seconds_local(2, move || {
        if let Some(h) = find_fan_hwmon() {
            let fans = scan_fans(&h);
            for (i, (row, bar, lbl)) in rows.iter().enumerate() {
                if let Some(f) = fans.get(i) {
                    row.set_title(&format!("Fan {}", f.index));
                    row.set_visible(true);
                    let max = f.max.unwrap_or(6500) as f64;
                    bar.set_value((f.rpm as f64 / max).clamp(0.0, 1.0));
                    lbl.set_text(&format!("{} rpm", f.rpm));
                } else {
                    row.set_visible(false);
                }
            }
        }
        glib::ControlFlow::Continue
    });
}

fn add_memory_card(page: &adw::PreferencesPage) {
    let group = adw::PreferencesGroup::builder().title("Memory").build();
    let row = adw::ActionRow::builder().title("RAM").subtitle("…").build();
    let bar = gtk::LevelBar::new();
    bar.set_min_value(0.0);
    bar.set_max_value(1.0);
    bar.set_hexpand(true);
    bar.set_valign(gtk::Align::Center);
    bar.set_size_request(200, 8);
    row.add_suffix(&bar);
    group.add(&row);
    page.add(&group);

    glib::timeout_add_seconds_local(3, move || {
        let (used, total) = read_mem();
        row.set_subtitle(&format!("{:.1} / {:.1} GiB", used, total));
        bar.set_value(if total > 0.0 { used / total } else { 0.0 });
        glib::ControlFlow::Continue
    });
}

fn add_power_card(page: &adw::PreferencesPage) {
    let group = adw::PreferencesGroup::builder().title("Power draw").build();
    let total_row = adw::ActionRow::builder().title("Total").subtitle("…").build();
    group.add(&total_row);
    let rail_rows: Vec<adw::ActionRow> = (0..4).map(|_| {
        adw::ActionRow::builder().title("—").subtitle("—").visible(false).build()
    }).collect();
    for r in &rail_rows { group.add(r); }
    page.add(&group);

    glib::timeout_add_seconds_local(1, move || {
        let rails = read_power_rails();
        let total: f64 = rails.iter().map(|(_, v)| v).sum();
        total_row.set_subtitle(&format!("{:.2} W", total));
        for (i, r) in rail_rows.iter().enumerate() {
            if let Some((n, v)) = rails.get(i) {
                r.set_title(n);
                r.set_subtitle(&format!("{:.2} W", v));
                r.set_visible(true);
            } else {
                r.set_visible(false);
            }
        }
        glib::ControlFlow::Continue
    });
}

fn add_actions_card(page: &adw::PreferencesPage) {
    let group = adw::PreferencesGroup::builder().title("Quick actions").build();
    for (label, cmd) in [
        ("Override charge to 100% for 2h", "charge"),
        ("Cancel charge override", "cancel"),
        ("Open kernel manager", "kernel"),
        ("Generate diagnostic bundle", "diag"),
        ("Open trackpad visualizer", "viz"),
    ] {
        let btn = gtk::Button::with_label(label);
        btn.set_margin_top(4);
        btn.set_margin_bottom(4);
        match cmd {
            "charge" => { btn.connect_clicked(move |_| { let _ = asahi_core::charge::override_for(100, 7200); }); }
            "cancel" => { btn.connect_clicked(move |_| { let _ = asahi_core::charge::cancel_override(); }); }
            "kernel" | "diag" | "viz" => {
                let bin = match cmd { "kernel" => "asahi-kernel-manager", "diag" => "asahi-diag", _ => "asahi-trackpad-viz" };
                let bin_s = bin.to_string();
                btn.connect_clicked(move |_| { let _ = crate::helper::spawn_in_terminal(&bin_s); });
            }
            _ => {}
        }
        group.add(&btn);
    }
    page.add(&group);
}

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_search_enabled(true);
    let cfg = load_widget_config();

    if widget_enabled("battery", &cfg) { add_battery_card(&page); }
    if widget_enabled("thermal", &cfg) { add_thermal_card(&page); }
    if widget_enabled("fans", &cfg) { add_fans_card(&page); }
    if widget_enabled("memory", &cfg) { add_memory_card(&page); }
    if widget_enabled("power", &cfg) { add_power_card(&page); }
    if widget_enabled("actions", &cfg) { add_actions_card(&page); }

    page
}
