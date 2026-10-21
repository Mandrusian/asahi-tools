use super::WidgetInstance;
use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

pub fn build(_inst: &WidgetInstance) -> gtk::Widget {
    let group = adw::PreferencesGroup::builder()
        .title("Active Profile")
        .build();
    let profile = crate::helper::get_platform_profile();
    let subtitle = profile.unwrap_or_else(|| "Unavailable".into());
    group.add(
        &adw::ActionRow::builder()
            .title("Platform")
            .subtitle(&subtitle)
            .build(),
    );
    group.upcast()
}
