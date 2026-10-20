use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::process::Command;

fn current_profile() -> String {
    std::fs::read_to_string("/sys/firmware/acpi/platform_profile")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

fn set_profile(p: &str) -> bool {
    asahi_core::priv_daemon::set_profile(p).is_ok()
}

fn read_power_rails() -> Vec<(String, f64)> {
    let hw = match asahi_core::find_fan_hwmon() {
        Some(h) => h, None => return Vec::new(),
    };
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
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_search_enabled(true);

    let profiles_group = adw::PreferencesGroup::builder()
        .title("Power profile")
        .description("Writes to /sys/firmware/acpi/platform_profile — PKEXEC prompt on change")
        .build();

    let current_row = adw::ActionRow::builder().title("Current").subtitle(&current_profile()).build();
    profiles_group.add(&current_row);

    for (label, val) in [("Quiet", "quiet"), ("Balanced", "balanced"), ("Performance", "performance")] {
        let btn = gtk::Button::with_label(label);
        btn.set_margin_top(4);
        btn.set_margin_bottom(4);
        let v = val.to_string();
        let r = current_row.clone();
        btn.connect_clicked(move |_| {
            set_profile(&v);
            let _ = r;
        });
        profiles_group.add(&btn);
    }
    page.add(&profiles_group);

    let rails_group = adw::PreferencesGroup::builder()
        .title("Power rails")
        .description("Live SMC power readings (0.5s refresh)")
        .build();
    let rails: Vec<adw::ActionRow> = (0..8)
        .map(|_| adw::ActionRow::builder().title("—").subtitle("—").visible(false).build())
        .collect();
    for r in &rails { rails_group.add(r); }
    page.add(&rails_group);

    {
        let current_row = current_row.clone();
        let rails = rails.clone();
        glib::timeout_add_seconds_local(1, move || {
            current_row.set_subtitle(&current_profile());
            let pw = read_power_rails();
            for (i, r) in rails.iter().enumerate() {
                if let Some((n, v)) = pw.get(i) {
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

    page
}
