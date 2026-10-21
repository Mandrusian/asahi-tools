use super::{WidgetInstance, WidgetSize};
use adw::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub fn build(inst: &WidgetInstance) -> gtk::Widget {
    let size = if let Some(s) = inst.settings.get("__size_hint__").and_then(|v| v.as_str()) {
        WidgetSize::from_str(s)
    } else {
        inst.size
    };
    let small = size == WidgetSize::Small;
    let graph_h = if small { 40 } else { 100 };

    let group = adw::PreferencesGroup::builder()
        .title("Battery History")
        .build();
    let Some(start_pct) = asahi_core::get_battery_capacity().ok() else {
        group.add(&gtk::Label::new(Some("Unavailable: no battery device")));
        return group.upcast();
    };
    let window = inst
        .settings
        .get("window")
        .and_then(|v| v.as_u64())
        .unwrap_or(120) as usize;
    let history = Arc::new(Mutex::new(VecDeque::from(vec![
        start_pct as f64;
        window.min(200)
    ])));

    let label: Option<gtk::Label> = if small {
        None
    } else {
        let l = gtk::Label::new(Some(&format!("Current: {}%", start_pct)));
        l.set_xalign(0.0);
        l.add_css_class("dim-label");
        group.add(&l);
        Some(l)
    };

    let hist_draw = history.clone();
    let da = gtk::DrawingArea::new();
    da.set_content_height(graph_h);
    da.set_hexpand(true);
    da.set_draw_func(move |_, cr, w, h| {
        let data = hist_draw.lock().unwrap().clone();
        crate::helper::draw_graph(cr, w, h, &data, 0.0, 100.0, (0.20, 0.60, 0.90));
    });
    group.add(&da);

    let hist_tick = history.clone();
    let label_tick = label.clone();
    glib::timeout_add_seconds_local(5, move || {
        if !da.is_mapped() {
            return glib::ControlFlow::Continue;
        }
        if let Ok(pct) = asahi_core::get_battery_capacity() {
            let mut h = hist_tick.lock().unwrap();
            h.push_back(pct as f64);
            h.pop_front();
            drop(h);
            if let Some(l) = &label_tick {
                l.set_text(&format!("Current: {}%", pct));
            }
        }
        glib::ControlFlow::Continue
    });

    group.upcast()
}
