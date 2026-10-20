use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::process::Command;
use crate::helper::{find_binary, spawn_in_terminal};

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    // ── Kernel ──
    let kernel_group = adw::PreferencesGroup::builder().title("Kernel").build();
    let ver_row = adw::ActionRow::builder().title("Current").subtitle("…").build();
    kernel_group.add(&ver_row);
    let up_row = adw::ActionRow::builder().title("Uptime").subtitle("…").build();
    kernel_group.add(&up_row);

    let km_btn = gtk::Button::with_label("Open kernel manager (updates + m1n1)");
    km_btn.set_margin_top(4);
    km_btn.set_margin_bottom(4);
    km_btn.connect_clicked(move |_| { spawn_in_terminal("asahi-kernel-manager"); });
    kernel_group.add(&km_btn);

    page.add(&kernel_group);

    // ── Snapshots ──
    let snap_group = adw::PreferencesGroup::builder()
        .title("Snapshots")
        .description("Btrfs snapshot before major updates")
        .build();
    let create_snap = gtk::Button::with_label("Create pre-update snapshot");
    create_snap.set_margin_top(4);
    create_snap.set_margin_bottom(4);
    create_snap.connect_clicked(move |_| {
        if let Some(b) = find_binary("asahi-snapshot") {
            let _ = Command::new("pkexec").args([b.to_str().unwrap_or("asahi-snapshot"), "before-update"]).spawn();
        }
    });
    snap_group.add(&create_snap);

    let list_snap = gtk::Button::with_label("List snapshots");
    list_snap.set_margin_bottom(8);
    list_snap.connect_clicked(move |_| { spawn_in_terminal("asahi-snapshot"); });
    snap_group.add(&list_snap);

    page.add(&snap_group);

    // ── Diagnostics ──
    let diag_group = adw::PreferencesGroup::builder()
        .title("Diagnostics")
        .description("Bug report helpers")
        .build();
    let diag_btn = gtk::Button::with_label("Generate diagnostic bundle");
    diag_btn.set_margin_top(4);
    diag_btn.set_margin_bottom(4);
    diag_btn.connect_clicked(move |_| {
        if let Some(bin) = find_binary("asahi-diag") {
            Command::new(bin).arg("/tmp").spawn().ok();
        }
    });
    diag_group.add(&diag_btn);

    let sensors_btn = gtk::Button::with_label("List all sensors (opens terminal)");
    sensors_btn.set_margin_bottom(8);
    sensors_btn.connect_clicked(move |_| { spawn_in_terminal("asahi-sensor-explorer"); });
    diag_group.add(&sensors_btn);

    page.add(&diag_group);

    // ── Refresh ──
    {
        let ver_row = ver_row.clone();
        let up_row = up_row.clone();
        glib::timeout_add_seconds_local(5, move || {
            if let Ok(o) = Command::new("uname").arg("-r").output() {
                ver_row.set_subtitle(&String::from_utf8_lossy(&o.stdout).trim());
            }
            if let Ok(txt) = std::fs::read_to_string("/proc/uptime") {
                let s: u64 = txt.split_whitespace().next()
                    .and_then(|v| v.parse().ok()).unwrap_or(0);
                let d = s / 86400; let h = (s % 86400) / 3600; let m = (s % 3600) / 60;
                up_row.set_subtitle(&if d > 0 { format!("{}d {}h", d, h) }
                    else if h > 0 { format!("{}h {}m", h, m) }
                    else { format!("{}m", m) });
            }
            glib::ControlFlow::Continue
        });
    }

    page
}
