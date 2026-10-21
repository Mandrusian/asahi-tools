use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::process::Command;

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    let g1 = adw::PreferencesGroup::builder().title("Keyboard").build();
    let (cur, max) = asahi_core::priv_daemon::get_kbd().unwrap_or((0, 255));
    let row = adw::ActionRow::builder().title("Backlight").build();
    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, max as f64, 1.0);
    slider.set_value(cur as f64);
    slider.set_hexpand(true);
    slider.set_size_request(300, -1);
    slider.set_valign(gtk::Align::Center);
    row.add_suffix(&slider);
    g1.add(&row);
    slider.connect_value_changed(move |s| {
        let _ = asahi_core::priv_daemon::set_kbd(s.value() as u32);
    });

    // Keyboard Layout
    let layout_row = adw::ActionRow::builder()
        .title("Keyboard Layout")
        .subtitle("System default")
        .build();
    let combo = gtk::ComboBoxText::new();
    combo.append_text("us");
    combo.append_text("gb");
    combo.append_text("au");
    combo.append_text("de");
    combo.set_active(Some(0));
    combo.set_valign(gtk::Align::Center);
    combo.connect_changed(move |c| {
        if let Some(txt) = c.active_text() {
            let _ = Command::new("localectl")
                .args(["set-x11-keymap", &txt])
                .spawn();
        }
    });
    layout_row.add_suffix(&combo);
    g1.add(&layout_row);

    let labels = ["Media keys (default)", "F1-F12 keys", "Auto"];
    let cur_fn = asahi_core::priv_daemon::get_fnmode().unwrap_or(1);
    let idx = match cur_fn {
        1 => 0u32,
        2 => 1,
        3 => 2,
        _ => 0,
    };
    let fn_row = adw::ComboRow::builder()
        .title("Fn-row behavior")
        .subtitle("Controls what F1-F12 do")
        .model(&gtk::StringList::new(&labels))
        .selected(idx)
        .build();
    fn_row.connect_selected_notify(move |c| {
        let f = match c.selected() {
            0 => 1,
            1 => 2,
            _ => 3,
        };
        let _ = asahi_core::priv_daemon::set_fnmode(f);
    });
    g1.add(&fn_row);
    page.add(&g1);

    let g2 = adw::PreferencesGroup::builder()
        .title("Trackpad Force Click")
        .description("Secondary click fires when pressure > limit")
        .build();
    let tc = asahi_core::trackpad_config::TrackpadConfig::load();
    let row_e = adw::ActionRow::builder().title("Enabled").build();
    let sw = gtk::Switch::new();
    sw.set_active(tc.enabled);
    sw.set_valign(gtk::Align::Center);
    row_e.add_suffix(&sw);
    g2.add(&row_e);
    sw.connect_active_notify(move |s| {
        let mut c = asahi_core::trackpad_config::TrackpadConfig::load();
        c.enabled = s.is_active();
        let _ = c.save();
    });

    let row_p = adw::ActionRow::builder().title("Press limit").build();
    let spin_p = gtk::SpinButton::with_range(0.0, 6000.0, 50.0);
    spin_p.set_value(tc.hard_press as f64);
    spin_p.set_valign(gtk::Align::Center);
    row_p.add_suffix(&spin_p);
    g2.add(&row_p);
    spin_p.connect_value_changed(move |s| {
        let mut c = asahi_core::trackpad_config::TrackpadConfig::load();
        c.hard_press = s.value() as i32;
        let _ = c.save();
    });

    let btn_cal = gtk::Button::with_label("Run Calibration (in-app)");
    btn_cal.set_margin_top(8);
    btn_cal.set_margin_bottom(8);
    btn_cal.connect_clicked(move |b| {
        if let Some(win) = b.root().and_then(|r| r.downcast::<gtk::Window>().ok()) {
            let dlg = gtk::Window::builder()
                .transient_for(&win)
                .modal(true)
                .title("Calibration")
                .default_width(400)
                .default_height(300)
                .build();
            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
            vbox.set_margin_top(24);
            vbox.set_margin_bottom(24);
            vbox.set_margin_start(24);
            vbox.set_margin_end(24);
            let lbl = gtk::Label::new(Some("Follow the terminal prompts..."));
            vbox.append(&lbl);
            let spinner = gtk::Spinner::new();
            spinner.start();
            vbox.append(&spinner);
            dlg.set_child(Some(&vbox));
            dlg.present();
        }
    });
    g2.add(&btn_cal);

    let btn_viz = gtk::Button::with_label("Open Trackpad Visualizer");
    btn_viz.set_margin_bottom(8);
    btn_viz.connect_clicked(move |b| {
        if let Some(win) = b.root().and_then(|r| r.downcast::<gtk::Window>().ok()) {
            let dlg = gtk::Window::builder()
                .transient_for(&win)
                .modal(true)
                .title("Visualizer")
                .default_width(600)
                .default_height(400)
                .build();
            let lbl = gtk::Label::new(Some("Visualizer requires evdev access. Coming in v0.5."));
            dlg.set_child(Some(&lbl));
            dlg.present();
        }
    });
    g2.add(&btn_viz);
    page.add(&g2);
    page
}
