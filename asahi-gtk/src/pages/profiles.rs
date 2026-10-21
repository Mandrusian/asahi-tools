use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let g = adw::PreferencesGroup::builder()
        .title("Power Profiles")
        .build();

    let profiles = [
        (
            "Quiet",
            "Caps fan speed, limits CPU power draw, and dims the display.",
        ),
        (
            "Balanced",
            "Default behavior. Auto fans, medium brightness, 80% charge cap.",
        ),
        (
            "Performance",
            "Maximum fan speed, full brightness, no charge limit.",
        ),
    ];

    for (name, desc) in profiles {
        let row = adw::ActionRow::builder().title(name).subtitle(desc).build();
        let btn = gtk::Button::with_label("Apply");
        btn.set_valign(gtk::Align::Center);
        let name_clone = name.to_lowercase();
        btn.connect_clicked(move |_| {
            let _ = asahi_core::priv_daemon::set_profile(&name_clone);
            if name_clone == "quiet" {
                let _ = asahi_core::charge::set_target(80, None);
            } else if name_clone == "performance" {
                let _ = asahi_core::charge::set_target(100, None);
            }
        });
        row.add_suffix(&btn);
        g.add(&row);
    }
    page.add(&g);
    page
}
