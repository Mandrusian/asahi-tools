use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::process::Command;

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let g1 = adw::PreferencesGroup::builder().title("System Info").build();
    let kernel = crate::helper::read_file("/proc/sys/kernel/osrelease").unwrap_or_else(|| "Unknown".into());
    let uptime = crate::helper::read_file("/proc/uptime").and_then(|s| s.split_whitespace().next().and_then(|v| v.parse::<u64>().ok())).map(|s| format!("{}h {}m", s / 3600, (s % 3600) / 60)).unwrap_or_else(|| "Unknown".into());
    let m1n1 = crate::helper::read_file("/proc/device-tree/chosen/asahi,uboot-version").unwrap_or_else(|| "Unknown".into());
    g1.add(&adw::ActionRow::builder().title("Kernel").subtitle(&kernel).build());
    g1.add(&adw::ActionRow::builder().title("Uptime").subtitle(&uptime).build());
    g1.add(&adw::ActionRow::builder().title("Bootloader").subtitle(&m1n1).build());
    page.add(&g1);

    let g2 = adw::PreferencesGroup::builder().title("Maintenance").build();
    let btn_diag = gtk::Button::with_label("Generate Diagnostic Bundle"); btn_diag.set_margin_top(4); btn_diag.set_margin_bottom(4);
    if crate::helper::find_binary("asahi-diag").is_none() { btn_diag.set_sensitive(false); btn_diag.set_label("Diagnostic tool not installed"); }
    g2.add(&btn_diag);

    let btn_snap = gtk::Button::with_label("Create Btrfs Snapshot"); btn_snap.set_margin_bottom(8);
    if crate::helper::find_binary("asahi-snapshot").is_none() { btn_snap.set_sensitive(false); btn_snap.set_label("Snapshot tool not installed"); }
    g2.add(&btn_snap);
    page.add(&g2);
    page
}
