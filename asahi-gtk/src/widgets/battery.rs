use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use super::{WidgetInstance, WidgetSize};

pub fn build(inst: &WidgetInstance) -> gtk::Widget {
    let size = if let Some(s) = inst.settings.get("__size_hint__").and_then(|v| v.as_str()) {
        WidgetSize::from_str(s)
    } else { inst.size };
    let small = size == WidgetSize::Small;
    let h = if small { 60 } else { 120 };
    let w = if small { 60 } else { 120 };

    let group = adw::PreferencesGroup::builder().title("Battery").build();
    let Some(pct) = asahi_core::get_battery_capacity().ok() else {
        group.add(&gtk::Label::new(Some("Unavailable: battery device not found")));
        return group.upcast();
    };
    let pct = pct as f64;
    let da = gtk::DrawingArea::new();
    da.set_content_height(h);
    da.set_content_width(w);
    da.set_hexpand(true);
    da.set_halign(gtk::Align::Center);
    da.set_draw_func(move |_, cr, dw, dh| {
        let (r, g, b) = if pct > 80.0 { (0.88, 0.11, 0.14) }
                        else if pct >= 60.0 { (0.96, 0.65, 0.15) }
                        else { (0.18, 0.76, 0.49) };
        crate::helper::draw_gauge(cr, dw, dh, pct, (r, g, b), &format!("{:.0}%", pct));
    });
    group.add(&da);
    group.upcast()
}
