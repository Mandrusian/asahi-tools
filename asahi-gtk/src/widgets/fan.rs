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
    let graph_h = if small { 50 } else { 100 };

    let group = adw::PreferencesGroup::builder().title("Fan Speed").build();
    let fans = crate::helper::get_hwmon_fans();
    if fans.is_empty() {
        group.add(&gtk::Label::new(Some("Unavailable: no fan devices")));
        return group.upcast();
    }
    let fan_idx = inst
        .settings
        .get("fan_index")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .max(1) as usize
        - 1;
    let Some((name, rpm, min, max)) = fans.get(fan_idx).cloned() else {
        group.add(&gtk::Label::new(Some(&format!(
            "Unavailable: fan {} not found",
            fan_idx + 1
        ))));
        return group.upcast();
    };
    let style = inst
        .settings
        .get("style")
        .and_then(|v| v.as_str())
        .unwrap_or("gauge")
        .to_string();

    let label: Option<gtk::Label> = if small {
        None
    } else {
        let l = gtk::Label::new(Some(&format!("{} — {} rpm", name, rpm)));
        l.set_xalign(0.0);
        l.add_css_class("dim-label");
        group.add(&l);
        Some(l)
    };

    let history = Arc::new(Mutex::new(VecDeque::from(vec![rpm as f64; 30])));
    let hist_draw = history.clone();
    let da = gtk::DrawingArea::new();
    da.set_content_height(graph_h);
    da.set_hexpand(true);
    let style_clone = style.clone();
    let min_f = min as f64;
    let max_f = max as f64;
    da.set_draw_func(move |_, cr, w, h| {
        if style_clone == "gauge" {
            let data = hist_draw.lock().unwrap();
            let cur = *data.back().unwrap_or(&0.0);
            let pct = ((cur - min_f) / (max_f - min_f).max(1.0)).clamp(0.0, 1.0) * 100.0;
            crate::helper::draw_gauge(cr, w, h, pct, (0.13, 0.55, 0.86), &format!("{:.0}", cur));
        } else {
            let data = hist_draw.lock().unwrap();
            crate::helper::draw_graph(cr, w, h, &data, min_f, max_f, (0.13, 0.55, 0.86));
        }
    });
    group.add(&da);

    let hist_tick = history.clone();
    let label_tick = label.clone();
    let idx = fan_idx;
    let n = name.clone();
    let da_weak = da.downgrade();
    glib::timeout_add_seconds_local(2, move || {
        let Some(_da) = da_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if let Some((_, rpm, _, _)) = crate::helper::get_hwmon_fans().get(idx).cloned() {
            let mut h = hist_tick.lock().unwrap();
            h.push_back(rpm as f64);
            h.pop_front();
            drop(h);
            if let Some(l) = &label_tick {
                l.set_text(&format!("{} — {} rpm", n, rpm));
            }
        }
        glib::ControlFlow::Continue
    });

    group.upcast()
}
