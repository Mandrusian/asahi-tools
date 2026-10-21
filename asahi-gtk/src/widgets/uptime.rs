use super::WidgetInstance;
use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

pub fn build(_inst: &WidgetInstance) -> gtk::Widget {
    let group = adw::PreferencesGroup::builder().title("Uptime").build();
    let text = crate::helper::get_uptime_secs()
        .map(|s| format!("{}h {}m", s / 3600, (s % 3600) / 60))
        .unwrap_or_else(|| "Unavailable".into());
    group.add(
        &adw::ActionRow::builder()
            .title("Uptime")
            .subtitle(&text)
            .build(),
    );
    group.upcast()
}
