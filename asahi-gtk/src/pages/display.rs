use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::process::Command;
use crate::helper::{find_binary, spawn_pkexec};

fn brightness_path() -> Option<String> {
    for p in [
        "/sys/class/backlight/apple-panel-backlight/brightness",
        "/sys/class/backlight/edp-backlight/brightness",
    ] {
        if std::path::Path::new(p).exists() { return Some(p.to_string()); }
    }
    None
}

fn brightness_max() -> u32 {
    for p in [
        "/sys/class/backlight/apple-panel-backlight/max_brightness",
        "/sys/class/backlight/edp-backlight/max_brightness",
    ] {
        if let Ok(s) = std::fs::read_to_string(p) {
            if let Ok(n) = s.trim().parse() { return n; }
        }
    }
    100
}

fn brightness_cur() -> u32 {
    brightness_path().and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

fn set_brightness(v: u32) {
    let _ = asahi_core::priv_daemon::set_display(v);
}

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_search_enabled(true);

    let bright_group = adw::PreferencesGroup::builder().title("Display brightness").build();
    let bright_row = adw::ActionRow::builder().title("Brightness").subtitle("—").build();
    let bright_slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, brightness_max() as f64, 1.0);
    bright_slider.set_hexpand(true);
    bright_slider.set_size_request(300, -1);
    bright_slider.set_value(brightness_cur() as f64);
    bright_row.add_suffix(&bright_slider);
    bright_group.add(&bright_row);

    {
        bright_slider.connect_value_changed(move |s| {
            set_brightness(s.value() as u32);
        });
    }
    page.add(&bright_group);

    let output_group = adw::PreferencesGroup::builder()
        .title("Display outputs")
        .description("Uses asahi-display (requires wlr-randr or kscreen-doctor)")
        .build();
    for (label, arg) in [
        ("Internal only", "internal"),
        ("External only", "external"),
        ("Extend", "extend"),
    ] {
        let btn = gtk::Button::with_label(label);
        btn.set_margin_top(4);
        btn.set_margin_bottom(4);
        btn.connect_clicked(move |_| {
            if let Some(bin) = find_binary("asahi-display") {
                Command::new(bin).arg(arg).spawn().ok();
            }
        });
        output_group.add(&btn);
    }
    page.add(&output_group);

    {
        let bright_row = bright_row.clone();
        glib::timeout_add_seconds_local(2, move || {
            let cur = brightness_cur();
            let max = brightness_max();
            bright_row.set_subtitle(&format!("{} / {}", cur, max));
            glib::ControlFlow::Continue
        });
    }

    let _ = spawn_pkexec;
    page
}
