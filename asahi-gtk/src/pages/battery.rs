use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use asahi_core::{charge, get_battery_capacity, get_battery_status, get_charge_thresholds};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use gtk::glib;

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let g1 = adw::PreferencesGroup::builder().title("Status").build();
    let cap = get_battery_capacity().unwrap_or(0) as f64;
    let temp = crate::helper::read_file("/sys/class/power_supply/macsmc-battery/temp").and_then(|s| s.parse::<f64>().ok()).map(|t| t / 10.0).unwrap_or(0.0);
    g1.add(&adw::ActionRow::builder().title("Charge").subtitle(&format!("{}%", cap)).build());
    g1.add(&adw::ActionRow::builder().title("Temp").subtitle(&format!("{:.1} C", temp)).build());
    g1.add(&adw::ActionRow::builder().title("State").subtitle(&get_battery_status().unwrap_or_default()).build());
    page.add(&g1);

    let g_graph = adw::PreferencesGroup::builder().title("Charge History").build();
    let history = Arc::new(Mutex::new(VecDeque::from(vec![cap; 30])));
    let hist_clone = history.clone();
    let da = gtk::DrawingArea::new();
    da.set_content_height(120);
    da.set_draw_func(move |_, cr, w, h| {
        crate::helper::draw_graph(cr, w, h, &hist_clone.lock().unwrap(), 0.0, 100.0, (0.2, 0.6, 0.9));
    });
    g_graph.add(&da);
    page.add(&g_graph);

    let g2 = adw::PreferencesGroup::builder().title("Charge Thresholds").build();
    let (start, end) = get_charge_thresholds().unwrap_or((70, 80));
    let row_e = adw::ActionRow::builder().title("Stop at").build();
    let spin_e = gtk::SpinButton::with_range(50.0, 100.0, 5.0); spin_e.set_value(end as f64); spin_e.set_valign(gtk::Align::Center);
    row_e.add_suffix(&spin_e); g2.add(&row_e);
    let row_s = adw::ActionRow::builder().title("Resume at").build();
    let spin_s = gtk::SpinButton::with_range(40.0, 95.0, 5.0); spin_s.set_value(start as f64); spin_s.set_valign(gtk::Align::Center);
    row_s.add_suffix(&spin_s); g2.add(&row_s);
    let save = gtk::Button::with_label("Save"); save.add_css_class("suggested-action"); save.set_margin_top(8); save.set_margin_bottom(8);
    let e_clone = spin_e.clone();
    save.connect_clicked(move |_| { let _ = charge::set_target(e_clone.value() as u32, None); });
    g2.add(&save);
    page.add(&g2);

    let g3 = adw::PreferencesGroup::builder().title("Temporary Full Charge").build();
    let status = charge::status();
    let override_active = status.as_ref().map(|s| s.override_active).unwrap_or(false);
    if override_active {
        let rem = status.as_ref().map(|s| s.override_remaining_secs.unwrap_or(0)).unwrap_or(0);
        let btn_c = gtk::Button::with_label(&format!("Cancel override ({}m left)", rem / 60)); 
        btn_c.add_css_class("destructive-action"); btn_c.set_margin_bottom(8);
        btn_c.connect_clicked(move |_| { let _ = charge::cancel_override(); });
        g3.add(&btn_c);
    } else {
        let btn_o = gtk::Button::with_label("Override to 100% for 2 hours"); btn_o.set_margin_top(4); btn_o.set_margin_bottom(4);
        btn_o.connect_clicked(move |_| { let _ = charge::override_for(100, 7200); });
        g3.add(&btn_o);
    }
    page.add(&g3);

    let hist2 = history.clone();
    glib::timeout_add_seconds_local(2, move || {
        if let Ok(c) = get_battery_capacity() { let mut h = hist2.lock().unwrap(); h.push_back(c as f64); h.pop_front(); }
        glib::ControlFlow::Continue
    });
    page
}
