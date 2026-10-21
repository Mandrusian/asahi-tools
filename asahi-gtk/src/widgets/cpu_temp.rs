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
        .title("CPU Temperature")
        .build();
    let sensors = crate::helper::get_hwmon_sensors();
    if sensors.is_empty() {
        group.add(&gtk::Label::new(Some("Unavailable: no hwmon sensors")));
        return group.upcast();
    }

    let window = inst
        .settings
        .get("window")
        .and_then(|v| v.as_u64())
        .unwrap_or(60) as usize;
    let history: Arc<Mutex<VecDeque<f64>>> =
        Arc::new(Mutex::new(VecDeque::from(vec![0.0; window.min(120)])));

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
        let (r, g, b) = if let Some(last) = data.back() {
            if *last > 80.0 {
                (0.88, 0.11, 0.14)
            } else if *last >= 60.0 {
                (0.96, 0.65, 0.15)
            } else {
                (0.18, 0.76, 0.49)
            }
        } else {
            (0.5, 0.5, 0.5)
        };
        crate::helper::draw_graph(cr, w, h, &data, 20.0, 100.0, (r, g, b));
    });
    group.add(&da);

    let hist_tick = history.clone();
    let label_tick = label.clone();
    let target_h = graph_h;
    let _ = target_h;
    glib::timeout_add_seconds_local(2, move || {
        if !da.is_mapped() {
            return glib::ControlFlow::Continue;
        }
        let sensors = crate::helper::get_hwmon_sensors();
        let hottest = sensors.iter().map(|(_, t)| *t).fold(0.0_f64, f64::max);
        if hottest > 0.0 {
            let mut h = hist_tick.lock().unwrap();
            h.push_back(hottest);
            h.pop_front();
            drop(h);
            if let Some(l) = &label_tick {
                l.set_text(&format!("Hottest: {:.1} °C", hottest));
            }
        }
        glib::ControlFlow::Continue
    });

    group.upcast()
}
