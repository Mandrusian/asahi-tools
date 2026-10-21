use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use super::{WidgetInstance, WidgetSize};

pub fn build(inst: &WidgetInstance) -> gtk::Widget {
    let size = if let Some(s) = inst.settings.get("__size_hint__").and_then(|v| v.as_str()) {
        WidgetSize::from_str(s)
    } else { inst.size };
    let small = size == WidgetSize::Small;
    let immediate = inst.settings.get("immediate").and_then(|v| v.as_bool()).unwrap_or(false);

    let group = adw::PreferencesGroup::builder().title("Charge Limit").build();
    let (start, end) = asahi_core::get_charge_thresholds().unwrap_or((70, 80));

    let end_row = adw::ActionRow::builder().title("Stop at").build();
    let end_spin = gtk::SpinButton::with_range(50.0, 100.0, 5.0);
    end_spin.set_value(end as f64);
    end_spin.set_valign(gtk::Align::Center);
    end_row.add_suffix(&end_spin);
    group.add(&end_row);

    if !small {
        let start_row = adw::ActionRow::builder().title("Resume at").build();
        let start_spin = gtk::SpinButton::with_range(40.0, 95.0, 5.0);
        start_spin.set_value(start as f64);
        start_spin.set_valign(gtk::Align::Center);
        start_row.add_suffix(&start_spin);
        group.add(&start_row);
    }

    let apply_btn = gtk::Button::with_label("Apply");
    apply_btn.add_css_class("suggested-action");
    apply_btn.set_margin_top(8);
    apply_btn.set_margin_bottom(8);
    group.add(&apply_btn);

    let end_clone = end_spin.clone();
    let apply_btn_clone = apply_btn.clone();
    end_spin.connect_value_changed(move |s| {
        if immediate {
            let _ = asahi_core::charge::set_target(s.value() as u32, None);
        }
    });

    if !immediate {
        apply_btn.connect_clicked(move |_| {
            let _ = asahi_core::charge::set_target(end_clone.value() as u32, None);
        });
    } else {
        apply_btn_clone.set_sensitive(false);
        apply_btn_clone.set_label("Applies immediately");
    }

    group.upcast()
}
