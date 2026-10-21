use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::cell::RefCell;
use std::rc::Rc;
use gtk::glib;
use super::{WidgetInstance, WidgetSize};

pub fn build(inst: &WidgetInstance) -> gtk::Widget {
    let size = if let Some(s) = inst.settings.get("__size_hint__").and_then(|v| v.as_str()) {
        WidgetSize::from_str(s)
    } else { inst.size };
    let small = size == WidgetSize::Small;
    let graph_h = if small { 40 } else { 100 };

    let group = adw::PreferencesGroup::builder().title("CPU Usage").build();
    let window = inst.settings.get("window").and_then(|v| v.as_u64()).unwrap_or(60) as usize;
    let history = Arc::new(Mutex::new(VecDeque::from(vec![0.0; window.min(120)])));
    let prev = Rc::new(RefCell::new(None::<(u64, u64)>));

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
        crate::helper::draw_graph(cr, w, h, &data, 0.0, 100.0, (0.86, 0.55, 0.13));
    });
    group.add(&da);

    let hist_tick = history.clone();
    let label_tick = label.clone();
    let prev_tick = prev.clone();
    glib::timeout_add_seconds_local(1, move || {
        if !da.is_mapped() { return glib::ControlFlow::Continue; }
        if let Some((idle, total)) = crate::helper::read_cpu_raw() {
            let mut p = prev_tick.borrow_mut();
            if let Some((pi, pt)) = *p {
                let di = idle.saturating_sub(pi);
                let dt = total.saturating_sub(pt);
                if dt > 0 {
                    let usage = 100.0 * (1.0 - di as f64 / dt as f64);
                    let mut h = hist_tick.lock().unwrap();
                    h.push_back(usage);
                    h.pop_front();
                    drop(h);
                    if let Some(l) = &label_tick {
                        l.set_text(&format!("{:.1}%", usage));
                    }
                }
            }
            *p = Some((idle, total));
        }
        glib::ControlFlow::Continue
    });

    group.upcast()
}
