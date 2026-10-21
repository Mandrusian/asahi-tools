use super::WidgetInstance;
use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

pub fn build(inst: &WidgetInstance) -> gtk::Widget {
    let group = adw::PreferencesGroup::builder()
        .title("Quick Actions")
        .build();
    let show_charge = inst
        .settings
        .get("show_charge")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let show_cancel = inst
        .settings
        .get("show_cancel")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let show_kernel = inst
        .settings
        .get("show_kernel")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let show_cal = inst
        .settings
        .get("show_calibrate")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let status = asahi_core::charge::status();
    let override_active = status.as_ref().map(|s| s.override_active).unwrap_or(false);

    if show_charge && !override_active {
        let btn = gtk::Button::with_label("Override Charge to 100%");
        btn.connect_clicked(|_| {
            let _ = asahi_core::charge::override_for(100, 7200);
        });
        group.add(&btn);
    }
    if show_cancel && override_active {
        let btn = gtk::Button::with_label("Cancel Charge Override");
        btn.add_css_class("destructive-action");
        btn.connect_clicked(|_| {
            let _ = asahi_core::charge::cancel_override();
        });
        group.add(&btn);
    }
    if show_kernel {
        let btn = gtk::Button::with_label("Open Kernel Manager");
        btn.connect_clicked(|_| {
            crate::helper::spawn_in_terminal("asahi-kernel-manager");
        });
        group.add(&btn);
    }
    if show_cal {
        let btn = gtk::Button::with_label("Calibrate Trackpad");
        btn.connect_clicked(|_| {
            crate::helper::spawn_in_terminal("asahi-force-click");
        });
        group.add(&btn);
    }

    group.upcast()
}
