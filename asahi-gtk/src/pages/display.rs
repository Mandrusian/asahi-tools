use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::process::Command;

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let (cur, max) = asahi_core::priv_daemon::get_display().unwrap_or((0, 420));
    let g = adw::PreferencesGroup::builder().title("Display").build();
    let row = adw::ActionRow::builder()
        .title("Brightness")
        .subtitle(&format!("{} / {}", cur, max))
        .build();
    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, max as f64, 1.0);
    slider.set_value(cur as f64);
    slider.set_hexpand(true);
    slider.set_size_request(300, -1);
    slider.set_valign(gtk::Align::Center);
    row.add_suffix(&slider);
    g.add(&row);
    slider.connect_value_changed(move |s| {
        let _ = asahi_core::priv_daemon::set_display(s.value() as u32);
    });
    page.add(&g);

    let g2 = adw::PreferencesGroup::builder()
        .title("Arrangement")
        .build();
    let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    for (lbl, arg) in [
        ("Internal", "internal"),
        ("External", "external"),
        ("Extend", "extend"),
    ] {
        let btn = gtk::Button::with_label(lbl);
        btn.set_size_request(100, 60);
        btn.connect_clicked(move |_| {
            if let Some(bin) = crate::helper::find_binary("asahi-display") {
                let _ = Command::new(bin).arg(arg).spawn();
            }
        });
        box_.append(&btn);
    }
    g2.add(&box_);
    page.add(&g2);
    page
}
