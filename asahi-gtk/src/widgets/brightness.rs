use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use super::WidgetInstance;

pub fn build(inst: &WidgetInstance) -> gtk::Widget {
    let group = adw::PreferencesGroup::builder().title("Brightness").build();
    let Some((cur, max)) = asahi_core::priv_daemon::get_display() else {
        group.add(&gtk::Label::new(Some("Unavailable: privileged daemon not running")));
        return group.upcast();
    };
    if max == 0 {
        group.add(&gtk::Label::new(Some("Unavailable: no backlight device")));
        return group.upcast();
    }
    let _step = inst.settings.get("step").and_then(|v| v.as_f64()).unwrap_or(5.0);
    let row = adw::ActionRow::builder().title("Display").subtitle(&format!("{} / {}", cur, max)).build();
    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 1.0, max as f64, 1.0);
    slider.set_value(cur as f64);
    slider.set_hexpand(true);
    slider.set_size_request(200, -1);
    slider.set_valign(gtk::Align::Center);
    row.add_suffix(&slider);
    group.add(&row);
    let row_clone = row.clone();
    slider.connect_value_changed(move |s| {
        let v = s.value() as u32;
        let _ = asahi_core::priv_daemon::set_display(v);
        row_clone.set_subtitle(&format!("{} / {}", v, max));
    });
    group.upcast()
}
