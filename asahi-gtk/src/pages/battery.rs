use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use asahi_core::{charge, get_battery_capacity, get_battery_status, get_charge_thresholds};

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    // ── Status ──
    let status = adw::PreferencesGroup::builder().title("Status").build();
    let cap_row = adw::ActionRow::builder().title("Charge").subtitle("…").build();
    status.add(&cap_row);
    let state_row = adw::ActionRow::builder().title("State").subtitle("…").build();
    status.add(&state_row);
    let override_row = adw::ActionRow::builder().title("Override").subtitle("none").build();
    status.add(&override_row);
    page.add(&status);

    // ── Thresholds ──
    let thresholds = adw::PreferencesGroup::builder()
        .title("Charge thresholds")
        .description("Charging stops at the end value, resumes at the start value")
        .build();

    let end_row = adw::ActionRow::builder().title("Stop at").subtitle("…").build();
    let end_spin = gtk::SpinButton::with_range(50.0, 100.0, 5.0);
    end_spin.set_valign(gtk::Align::Center);
    end_spin.set_width_chars(4);
    end_row.add_suffix(&end_spin);
    thresholds.add(&end_row);

    let start_row = adw::ActionRow::builder().title("Resume at").subtitle("…").build();
    let start_spin = gtk::SpinButton::with_range(40.0, 95.0, 5.0);
    start_spin.set_valign(gtk::Align::Center);
    start_spin.set_width_chars(4);
    start_row.add_suffix(&start_spin);
    thresholds.add(&start_row);

    let save_btn = gtk::Button::with_label("Save");
    save_btn.add_css_class("suggested-action");
    save_btn.set_margin_top(8);
    save_btn.set_margin_bottom(8);
    thresholds.add(&save_btn);

    // Load initial
    if let Ok((s, e)) = get_charge_thresholds() {
        start_spin.set_value(s as f64);
        end_spin.set_value(e as f64);
        start_row.set_subtitle(&format!("{}%", s));
        end_row.set_subtitle(&format!("{}%", e));
    }

    // Save handler → charge daemon
    {
        let s_spin = start_spin.clone();
        let e_spin = end_spin.clone();
        let s_row = start_row.clone();
        let e_row = end_row.clone();
        save_btn.connect_clicked(move |_| {
            let s = s_spin.value() as u32;
            let e = e_spin.value() as u32;
            if s >= e { return; }
            let _ = charge::set_target(e, None);
            s_row.set_subtitle(&format!("{}%", s));
            e_row.set_subtitle(&format!("{}%", e));
        });
    }

    page.add(&thresholds);

    // ── Override ──
    let override_group = adw::PreferencesGroup::builder()
        .title("Temporary full charge")
        .description("Charge to 100% for meetings, flights, etc.")
        .build();

    let override_btn = gtk::Button::with_label("Override to 100% for 2 hours");
    override_btn.set_margin_top(8);
    override_btn.set_margin_bottom(8);
    override_group.add(&override_btn);

    let cancel_btn = gtk::Button::with_label("Cancel override");
    cancel_btn.add_css_class("destructive-action");
    cancel_btn.set_margin_bottom(8);
    override_group.add(&cancel_btn);

    {
        let btn = override_btn.clone();
        btn.connect_clicked(move |_| { let _ = charge::override_for(100, 7200); });
    }
    {
        cancel_btn.connect_clicked(move |_| { let _ = charge::cancel_override(); });
    }

    page.add(&override_group);

    // ── Refresh ──
    {
        let cap_row = cap_row.clone();
        let state_row = state_row.clone();
        let override_row = override_row.clone();
        glib::timeout_add_seconds_local(2, move || {
            let pct = get_battery_capacity().unwrap_or(0);
            cap_row.set_subtitle(&format!("{}%", pct));
            state_row.set_subtitle(&get_battery_status().unwrap_or_default());
            if let Some(cs) = charge::status() {
                if cs.override_active {
                    override_row.set_subtitle(&format!(
                        "{} remaining",
                        charge::fmt_duration(cs.override_remaining_secs.unwrap_or(0))
                    ));
                } else {
                    override_row.set_subtitle("none");
                }
            }
            glib::ControlFlow::Continue
        });
    }

    page
}
