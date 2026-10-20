use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
struct Profile {
    name: String,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    description: Option<String>,
    settings: serde_json::Value,
}

#[derive(Deserialize)]
struct ProfilesFile {
    profiles: Vec<Profile>,
}

fn load_profiles() -> Vec<Profile> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let p = home.join(".config/asahi-tools/profiles.json");
    match std::fs::read_to_string(&p) {
        Ok(s) => serde_json::from_str::<ProfilesFile>(&s)
            .map(|f| f.profiles)
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn apply_profile(profile: &Profile) {
    let s = &profile.settings;

    if let Some(platform) = s.get("platform_profile").and_then(|v| v.as_str()) {
        let _ = asahi_core::priv_daemon::set_profile(platform);
    }
    if let Some(disp) = s.get("display_percent").and_then(|v| v.as_u64()) {
        let max = asahi_core::priv_daemon::get_display().map(|(_, m)| m).unwrap_or(100);
        let val = (disp as u32 * max) / 100;
        let _ = asahi_core::priv_daemon::set_display(val);
    }
    if let Some(kbd) = s.get("kbd_percent").and_then(|v| v.as_u64()) {
        let max = asahi_core::priv_daemon::get_kbd().map(|(_, m)| m).unwrap_or(255);
        let val = (kbd as u32 * max) / 100;
        let _ = asahi_core::priv_daemon::set_kbd(val);
    }
    if let Some(target) = s.get("charge_target").and_then(|v| v.as_u64()) {
        let _ = asahi_core::charge::set_target(target as u32, None);
    }
    if let Some(rpm) = s.get("fan_max_rpm") {
        if rpm.is_null() {
            let _ = asahi_core::sysmon::auto_fan(1);
        } else if let Some(v) = rpm.as_u64() {
            let _ = asahi_core::sysmon::set_fan(1, v as u32);
        }
    }
}

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    let group = adw::PreferencesGroup::builder()
        .title("Quick profiles")
        .description("One click sets platform profile, brightness, charge threshold, and fans")
        .build();

    let profiles = load_profiles();
    if profiles.is_empty() {
        group.add(&adw::ActionRow::builder()
            .title("No profiles found")
            .subtitle("Add profiles to ~/.config/asahi-tools/profiles.json")
            .build());
    } else {
        for profile in &profiles {
            let row = adw::ActionRow::builder()
                .title(&profile.name)
                .subtitle(profile.description.as_deref().unwrap_or(""))
                .build();

            if let Some(icon_name) = &profile.icon {
                let icon = gtk::Image::from_icon_name(icon_name);
                icon.set_pixel_size(32);
                row.add_prefix(&icon);
            }

            let btn = gtk::Button::with_label("Apply");
            btn.add_css_class("suggested-action");
            btn.set_valign(gtk::Align::Center);
            let p_clone = profile.clone();
            btn.connect_clicked(move |_| {
                apply_profile(&p_clone);
                eprintln!("profiles: applied {}", p_clone.name);
            });
            row.add_suffix(&btn);
            row.set_activatable_widget(Some(&btn));

            group.add(&row);
        }
    }

    page.add(&group);

    // Info
    let info = adw::PreferencesGroup::builder().title("Notes").build();
    info.add(&adw::ActionRow::builder()
        .title("Config file")
        .subtitle("~/.config/asahi-tools/profiles.json")
        .build());
    info.add(&adw::ActionRow::builder()
        .title("Custom profiles")
        .subtitle("Edit the JSON file and reopen the app to see changes")
        .build());
    page.add(&info);

    page
}
