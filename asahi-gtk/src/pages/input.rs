use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::process::Command;
use asahi_core::trackpad_config::TrackpadConfig;
use crate::helper::{find_binary, spawn_in_terminal, spawn_pkexec};

fn kbd_brightness() -> (u32, u32) {
    let cur = std::fs::read_to_string("/sys/class/leds/kbd_backlight/brightness").ok()
        .and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let max = std::fs::read_to_string("/sys/class/leds/kbd_backlight/max_brightness").ok()
        .and_then(|s| s.trim().parse().ok()).unwrap_or(255);
    (cur, max)
}

fn set_kbd(v: u32) {
    let _ = asahi_core::priv_daemon::set_kbd(v);
}

fn fn_mode() -> String {
    std::fs::read_to_string("/sys/module/hid_apple/parameters/fnmode")
        .map(|s| s.trim().to_string()).unwrap_or_else(|_| "?".into())
}

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_search_enabled(true);

    // ── Keyboard ──
    let kbd_group = adw::PreferencesGroup::builder().title("Keyboard backlight").build();
    let (cur, max) = kbd_brightness();
    let kbd_row = adw::ActionRow::builder().title("Brightness").subtitle(&format!("{} / {}", cur, max)).build();
    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, max as f64, 1.0);
    slider.set_hexpand(true);
    slider.set_size_request(300, -1);
    slider.set_value(cur as f64);
    kbd_row.add_suffix(&slider);
    kbd_group.add(&kbd_row);
    slider.connect_value_changed(move |s| set_kbd(s.value() as u32));

    let fn_row = adw::ActionRow::builder().title("Fn-row mode").subtitle(&format!("fnmode={}", fn_mode())).build();
    let fn_btn = gtk::Button::with_label("Cycle");
    fn_btn.set_valign(gtk::Align::Center);
    fn_row.add_suffix(&fn_btn);
    fn_btn.connect_clicked(move |_| {
        let cur = asahi_core::priv_daemon::get_fnmode().unwrap_or(1);
        // Cycle only through useful modes: media(1) → fn(2) → auto(3) → media(1)
        // Never land on 0 (disabled) — that kills brightness/volume entirely.
        let next = match cur {
            1 => 2,
            2 => 3,
            _ => 1,
        };
        let _ = asahi_core::priv_daemon::set_fnmode(next);
    });
    kbd_group.add(&fn_row);

    page.add(&kbd_group);

    // ── Force click ──
    let tc = TrackpadConfig::load();
    let tp_group = adw::PreferencesGroup::builder()
        .title("Trackpad force click")
        .description("Secondary click fires when pressure exceeds the press limit")
        .build();

    let enabled_row = adw::ActionRow::builder()
        .title("Enabled")
        .subtitle(if tc.is_calibrated() { "calibrated" } else { "not calibrated" })
        .build();
    let switch = gtk::Switch::new();
    switch.set_active(tc.enabled);
    switch.set_valign(gtk::Align::Center);
    enabled_row.add_suffix(&switch);
    enabled_row.set_activatable_widget(Some(&switch));
    tp_group.add(&enabled_row);
    {
        let row = enabled_row.clone();
        switch.connect_active_notify(move |sw| {
            let mut c = TrackpadConfig::load();
            c.enabled = sw.is_active();
            let _ = c.save();
            row.set_subtitle(if sw.is_active() { "enabled" } else { "disabled" });
        });
    }

    let hp_row = adw::ActionRow::builder()
        .title("Press limit").subtitle(&tc.hard_press.to_string()).build();
    let hp_spin = gtk::SpinButton::with_range(0.0, 6000.0, 50.0);
    hp_spin.set_valign(gtk::Align::Center);
    hp_spin.set_width_chars(6);
    hp_spin.set_value(tc.hard_press as f64);
    hp_row.add_suffix(&hp_spin);
    tp_group.add(&hp_row);
    {
        let row = hp_row.clone();
        hp_spin.connect_value_changed(move |s| {
            let v = s.value() as i32;
            row.set_subtitle(&v.to_string());
            let mut c = TrackpadConfig::load();
            c.hard_press = v;
            let _ = c.save();
        });
    }

    let rel_row = adw::ActionRow::builder()
        .title("Release limit").subtitle(&tc.release.to_string()).build();
    let rel_spin = gtk::SpinButton::with_range(0.0, 6000.0, 50.0);
    rel_spin.set_valign(gtk::Align::Center);
    rel_spin.set_width_chars(6);
    rel_spin.set_value(tc.release as f64);
    rel_row.add_suffix(&rel_spin);
    tp_group.add(&rel_row);
    {
        let row = rel_row.clone();
        rel_spin.connect_value_changed(move |s| {
            let v = s.value() as i32;
            row.set_subtitle(&v.to_string());
            let mut c = TrackpadConfig::load();
            c.release = v;
            let _ = c.save();
        });
    }

    if let (Some(l), Some(h)) = (tc.observed_light_max, tc.observed_hard_max) {
        let obs_row = adw::ActionRow::builder()
            .title("Observed range")
            .subtitle(&format!("tap max {}, force max {}", l, h))
            .build();
        tp_group.add(&obs_row);
    }

    let calibrate_btn = gtk::Button::with_label("Run calibration (opens terminal)");
    calibrate_btn.set_margin_top(8);
    calibrate_btn.set_margin_bottom(8);
    calibrate_btn.connect_clicked(move |_| {
        if let Some(bin) = find_binary("asahi-force-click") {
            for term in ["kgx", "konsole", "alacritty", "xterm"] {
                let ok = Command::new(term)
                    .args(["-e", &format!("pkexec {} --calibrate", bin.display())])
                    .spawn().is_ok();
                if ok { break; }
            }
        }
    });
    tp_group.add(&calibrate_btn);

    let viz_btn = gtk::Button::with_label("Open pressure visualizer");
    viz_btn.set_margin_bottom(8);
    viz_btn.connect_clicked(move |_| { spawn_in_terminal("asahi-trackpad-viz"); });
    tp_group.add(&viz_btn);

    page.add(&tp_group);

    let _ = spawn_pkexec;
    page
}
