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
    let h = if small { 70 } else { 130 };

    let group = adw::PreferencesGroup::builder().title("Disk Usage").build();
    let Some(pct) = crate::helper::get_disk_usage() else {
        group.add(&gtk::Label::new(Some("Unavailable: df failed")));
        return group.upcast();
    };
    let da = gtk::DrawingArea::new();
    da.set_content_height(h);
    da.set_content_width(h);
    da.set_hexpand(true);
    da.set_halign(gtk::Align::Center);
    da.set_draw_func(move |_, cr, w, hh| {
        let (r, g, b) = if pct > 90.0 {
            (0.88, 0.11, 0.14)
        } else if pct > 75.0 {
            (0.96, 0.65, 0.15)
        } else {
            (0.18, 0.76, 0.49)
        };
        crate::helper::draw_gauge(cr, w, hh, pct, (r, g, b), &format!("{:.0}%", pct));
    });
    group.add(&da);
    group.upcast()
}
