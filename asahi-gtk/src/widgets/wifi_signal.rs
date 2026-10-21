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
    let large = size == WidgetSize::Large;
    let show_adv = inst
        .settings
        .get("show_advanced")
        .and_then(|v| v.as_bool())
        .unwrap_or(large);

    let group = adw::PreferencesGroup::builder()
        .title("Wi-Fi Signal")
        .build();

    let details = crate::helper::get_wifi_details();
    if !details.available {
        group.add(&gtk::Label::new(Some("Unavailable: no Wi-Fi device")));
        return group.upcast();
    }

    // Signal percentage label
    if !small {
        let label = gtk::Label::new(Some(&format!(
            "{}  ·  {:.0}%  ·  {}",
            details.ssid, details.signal, details.speed
        )));
        label.set_xalign(0.0);
        label.add_css_class("dim-label");
        group.add(&label);
    }

    // Bars + sparkline row
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);

    let bars = gtk::DrawingArea::new();
    bars.set_content_height(if small { 40 } else { 60 });
    bars.set_content_width(if small { 60 } else { 80 });
    let sig_now = details.signal as f64;
    bars.set_draw_func(move |_, cr, w, h| {
        crate::helper::draw_bars(cr, w, h, sig_now, (0.13, 0.55, 0.86));
    });
    row.append(&bars);

    let window = inst
        .settings
        .get("window")
        .and_then(|v| v.as_u64())
        .unwrap_or(60) as usize;
    let history: Arc<Mutex<VecDeque<f64>>> =
        Arc::new(Mutex::new(VecDeque::from(vec![sig_now; window.min(120)])));
    let hist_draw = history.clone();
    let spark = gtk::DrawingArea::new();
    spark.set_content_height(if small { 40 } else { 60 });
    spark.set_hexpand(true);
    spark.set_draw_func(move |_, cr, w, h| {
        let data = hist_draw.lock().unwrap().clone();
        crate::helper::draw_graph(cr, w, h, &data, 0.0, 100.0, (0.13, 0.55, 0.86));
    });
    row.append(&spark);

    group.add(&row);

    // Advanced info
    if show_adv {
        for (k, v) in [
            ("IPv4", details.ipv4.clone()),
            ("IPv6", details.ipv6.clone()),
            ("MAC", details.mac.clone()),
            ("BSSID", details.bssid.clone()),
            ("Gateway", details.gateway.clone()),
            ("DNS", details.dns.clone()),
        ] {
            let r = adw::ActionRow::builder().title(k).subtitle(&v).build();
            if !small {
                let copy_btn = gtk::Button::from_icon_name("edit-copy-symbolic");
                copy_btn.set_has_frame(false);
                copy_btn.set_valign(gtk::Align::Center);
                let val = v.clone();
                copy_btn.connect_clicked(move |_| {
                    if let Some(d) = gtk::gdk::Display::default() {
                        d.clipboard().set_text(&val);
                    }
                });
                r.add_suffix(&copy_btn);
            }
            group.add(&r);
        }
    }

    // Sparkline timer
    let hist_tick = history.clone();
    let spark_weak = spark.downgrade();
    glib::timeout_add_seconds_local(2, move || {
        let Some(_) = spark_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if let Some(sig) = crate::helper::get_wifi_signal() {
            let mut h = hist_tick.lock().unwrap();
            h.push_back(sig);
            h.pop_front();
        }
        glib::ControlFlow::Continue
    });

    group.upcast()
}
