use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use gtk::glib;
use super::{WidgetInstance, WidgetSize};

pub fn build(inst: &WidgetInstance) -> gtk::Widget {
    let size = if let Some(s) = inst.settings.get("__size_hint__").and_then(|v| v.as_str()) {
        WidgetSize::from_str(s)
    } else { inst.size };
    let small = size == WidgetSize::Small;
    let graph_h = if small { 40 } else { 100 };

    let group = adw::PreferencesGroup::builder().title("Memory").build();
    let window = inst.settings.get("window").and_then(|v| v.as_u64()).unwrap_or(60) as usize;
    let history = Arc::new(Mutex::new(VecDeque::from(vec![0.0; window.min(120)])));

    let label: Option<gtk::Label> = if small {
        None
    } else {
        let l = gtk::Label::new(Some("—"));
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
        crate::helper::draw_graph(cr, w, h, &data, 0.0, 100.0, (0.20, 0.68, 0.32));
    });
    group.add(&da);

    let hist_tick = history.clone();
    let label_tick = label.clone();
    glib::timeout_add_seconds_local(2, move || {
        if !da.is_mapped() { return glib::ControlFlow::Continue; }
        if let Some((used, total)) = crate::helper::get_memory_gb() {
            if let Some(pct) = crate::helper::get_memory_usage() {
                let mut h = hist_tick.lock().unwrap();
                h.push_back(pct);
                h.pop_front();
                drop(h);
                if let Some(l) = &label_tick {
                    l.set_text(&format!("{:.1} / {:.1} GiB ({:.0}%)", used, total, pct));
                }
            }
        }
        glib::ControlFlow::Continue
    });

    group.upcast()
}
