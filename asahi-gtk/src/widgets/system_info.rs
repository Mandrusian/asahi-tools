use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use super::{WidgetInstance, WidgetSize};

pub fn build(inst: &WidgetInstance) -> gtk::Widget {
    let size = if let Some(s) = inst.settings.get("__size_hint__").and_then(|v| v.as_str()) {
        WidgetSize::from_str(s)
    } else { inst.size };
    let small = size == WidgetSize::Small;

    let group = adw::PreferencesGroup::builder().title("System Info").build();
    group.add(&adw::ActionRow::builder().title("Kernel").subtitle(&crate::helper::get_kernel_version()).build());
    if !small {
        let boot = crate::helper::get_bootloader().unwrap_or_else(|| "Unavailable".into());
        group.add(&adw::ActionRow::builder().title("Bootloader").subtitle(&boot).build());
        let uptime = crate::helper::get_uptime_secs()
            .map(|s| format!("{}h {}m", s / 3600, (s % 3600) / 60))
            .unwrap_or_else(|| "Unavailable".into());
        group.add(&adw::ActionRow::builder().title("Uptime").subtitle(&uptime).build());
        let model = crate::helper::read_file("/proc/device-tree/model")
            .map(|s| s.replace('\0', ""))
            .unwrap_or_else(|| "Apple Silicon".into());
        group.add(&adw::ActionRow::builder().title("Model").subtitle(&model).build());
    }
    group.upcast()
}
