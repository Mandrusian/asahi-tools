use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

fn get_service_row(name: &str, desc: &str) -> adw::ActionRow {
    let status = asahi_core::priv_daemon::service_status(name);
    let is_active = status.as_ref().map(|s| s.active).unwrap_or(false);
    let is_enabled = status.as_ref().map(|s| s.enabled).unwrap_or(false);
    
    let state_str = match (is_active, is_enabled) {
        (true, true) => "running, enabled",
        (true, false) => "running, disabled",
        (false, true) => "stopped, enabled",
        _ => "stopped, disabled",
    };
    
    let row = adw::ActionRow::builder().title(name).subtitle(&format!("{} - {}", state_str, desc)).build();
    let sw = gtk::Switch::new();
    sw.set_active(is_active);
    sw.set_valign(gtk::Align::Center);
    row.add_suffix(&sw);
    
    let name_clone = name.to_string();
    sw.connect_active_notify(move |s| {
        let action = if s.is_active() { "start" } else { "stop" };
        let _ = asahi_core::priv_daemon::service_action(&name_clone, action);
    });
    
    row
}

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let g = adw::PreferencesGroup::builder().title("System Daemons").description("Toggle to start/stop").build();
    
    g.add(&get_service_row("asahi-priv-daemon", "Privileged operations"));
    g.add(&get_service_row("asahi-charge-daemon", "Battery threshold enforcer"));
    g.add(&get_service_row("asahi-sysmon-daemon", "Fan controller"));
    g.add(&get_service_row("asahi-force-click", "Trackpad force click"));
    
    page.add(&g);
    page
}
