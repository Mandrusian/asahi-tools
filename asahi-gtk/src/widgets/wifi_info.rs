use super::{WidgetInstance, WidgetSize};
use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

pub fn build(inst: &WidgetInstance) -> gtk::Widget {
    let size = if let Some(s) = inst.settings.get("__size_hint__").and_then(|v| v.as_str()) {
        WidgetSize::from_str(s)
    } else {
        inst.size
    };
    let small = size == WidgetSize::Small;

    let group = adw::PreferencesGroup::builder().title("Wi-Fi Info").build();
    let details = crate::helper::get_wifi_details();
    if !details.available {
        group.add(&gtk::Label::new(Some("Unavailable: no Wi-Fi device")));
        return group.upcast();
    }

    let hide = inst
        .settings
        .get("hide_sensitive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut rows: Vec<(&str, String, bool)> = vec![
        ("SSID", details.ssid.clone(), hide),
        ("IPv4", details.ipv4.clone(), hide),
        ("Gateway", details.gateway.clone(), hide),
        ("DNS", details.dns.clone(), hide),
    ];
    if !small {
        rows.push(("IPv6", details.ipv6.clone(), hide));
        rows.push(("MAC", details.mac.clone(), hide));
        rows.push(("BSSID", details.bssid.clone(), hide));
        rows.push(("Speed", details.speed.clone(), false));
    }

    for (k, v, hidden) in rows {
        let display = if hidden && !v.is_empty() && v != "—" {
            "••••••"
        } else {
            &v
        };
        let row = adw::ActionRow::builder().title(k).subtitle(display).build();
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
            row.add_suffix(&copy_btn);
        }
        group.add(&row);
    }

    group.upcast()
}
