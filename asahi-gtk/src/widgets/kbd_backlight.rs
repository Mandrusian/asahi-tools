use super::{WidgetInstance, WidgetSize};
use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

pub fn build(inst: &WidgetInstance) -> gtk::Widget {
    let size = if let Some(s) = inst.settings.get("__size_hint__").and_then(|v| v.as_str()) {
        WidgetSize::from_str(s)
    } else {
        inst.size
    };
    let small = size == WidgetSize::Small;

    let group = adw::PreferencesGroup::builder()
        .title("Keyboard Backlight")
        .build();
    let Some((cur, max)) = asahi_core::priv_daemon::get_kbd() else {
        group.add(&gtk::Label::new(Some(
            "Unavailable: privileged daemon not running",
        )));
        return group.upcast();
    };
    if max == 0 {
        group.add(&gtk::Label::new(Some(
            "Unavailable: no kbd backlight device",
        )));
        return group.upcast();
    }
    let _step = inst
        .settings
        .get("step")
        .and_then(|v| v.as_f64())
        .unwrap_or(5.0);
    let sub = if small {
        String::new()
    } else {
        format!("{} / {}", cur, max)
    };
    let row = adw::ActionRow::builder()
        .title("Brightness")
        .subtitle(&sub)
        .build();
    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, max as f64, 1.0);
    slider.set_value(cur as f64);
    slider.set_hexpand(true);
    slider.set_size_request(180, -1);
    slider.set_valign(gtk::Align::Center);
    row.add_suffix(&slider);
    group.add(&row);
    let row_c = row.clone();
    let show_label = !small;
    slider.connect_value_changed(move |s| {
        let v = s.value() as u32;
        let _ = asahi_core::priv_daemon::set_kbd(v);
        if show_label {
            row_c.set_subtitle(&format!("{} / {}", v, max));
        }
    });
    group.upcast()
}
