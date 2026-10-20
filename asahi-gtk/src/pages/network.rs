use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::process::Command;
use crate::helper::{find_binary, spawn_pkexec};

fn wifi_info() -> (String, String, String, u32) {
    let dev = Command::new("nmcli").args(["-t", "-f", "DEVICE,TYPE", "dev"]).output().ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines()
            .find_map(|l| { let (d, t) = l.split_once(':')?; (t == "wifi").then(|| d.to_string()) }))
        .unwrap_or_default();
    if dev.is_empty() { return ("(none)".into(), "-".into(), "-".into(), 0); }
    let (ssid, sig) = Command::new("nmcli").args(["-t", "-f", "ACTIVE,SSID,SIGNAL", "dev", "wifi"]).output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines()
            .find_map(|l| {
                let mut it = l.rsplitn(3, ':');
                let sig: u32 = it.next()?.parse().ok()?;
                let ssid = it.next()?.to_string();
                let act = it.next()?;
                act.contains('*').then_some((ssid, sig))
            }))
        .unwrap_or(("-".into(), 0));
    let ip = Command::new("ip").args(["-j", "addr", "show", &dev]).output().ok()
        .and_then(|o| {
            #[derive(serde::Deserialize)] struct A { family: String, local: String }
            #[derive(serde::Deserialize)] struct L { #[serde(default)] addr_info: Vec<A> }
            let v: Vec<L> = serde_json::from_slice(&o.stdout).ok()?;
            v.first().and_then(|l| l.addr_info.iter().find(|a| a.family == "inet").map(|a| a.local.clone()))
        })
        .unwrap_or_else(|| "-".into());
    (ssid, ip, dev, sig)
}

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    let wifi_group = adw::PreferencesGroup::builder().title("Wi-Fi").build();
    let ssid_row = adw::ActionRow::builder().title("SSID").subtitle("…").build();
    let ip_row = adw::ActionRow::builder().title("IPv4").subtitle("…").build();
    let sig_row = adw::ActionRow::builder().title("Signal").subtitle("…").build();
    wifi_group.add(&ssid_row);
    wifi_group.add(&ip_row);
    wifi_group.add(&sig_row);

    let restart_wifi = gtk::Button::with_label("Restart Wi-Fi module");
    restart_wifi.set_margin_top(4);
    restart_wifi.set_margin_bottom(4);
    restart_wifi.connect_clicked(move |_| {
        match asahi_core::priv_daemon::restart_module("brcmfmac") {
            Ok(()) => eprintln!("network: wifi module restarted"),
            Err(e) => eprintln!("network: wifi restart failed: {}", e),
        }
    });
    wifi_group.add(&restart_wifi);

    let net_tool_btn = gtk::Button::with_label("Open net-restarter (scan + restart)");
    net_tool_btn.set_margin_bottom(4);
    net_tool_btn.connect_clicked(move |_| { crate::helper::spawn_in_terminal("net-restarter"); });
    wifi_group.add(&net_tool_btn);

    page.add(&wifi_group);

    let bt_group = adw::PreferencesGroup::builder().title("Bluetooth").build();
    let bt_row = adw::ActionRow::builder().title("State").subtitle("…").build();
    bt_group.add(&bt_row);
    let restart_bt = gtk::Button::with_label("Restart Bluetooth module");
    restart_bt.set_margin_top(4);
    restart_bt.set_margin_bottom(4);
    restart_bt.connect_clicked(move |_| {
        match asahi_core::priv_daemon::restart_module("hci_bcm4377") {
            Ok(()) => eprintln!("network: bluetooth module restarted"),
            Err(e) => eprintln!("network: bluetooth restart failed: {}", e),
        }
    });
    bt_group.add(&restart_bt);
    page.add(&bt_group);

    {
        let ssid_row = ssid_row.clone();
        let ip_row = ip_row.clone();
        let sig_row = sig_row.clone();
        let bt_row = bt_row.clone();
        glib::timeout_add_seconds_local(3, move || {
            let (ssid, ip, _, sig) = wifi_info();
            ssid_row.set_subtitle(&ssid);
            ip_row.set_subtitle(&ip);
            sig_row.set_subtitle(&format!("{}%", sig));
            let bt = Command::new("bluetoothctl").args(["devices", "Connected"]).output()
                .ok().map(|o| String::from_utf8_lossy(&o.stdout).lines().filter(|l| l.starts_with("Device ")).count())
                .unwrap_or(0);
            bt_row.set_subtitle(&if bt == 0 { "on · 0 connected".to_string() } else { format!("on · {} connected", bt) });
            glib::ControlFlow::Continue
        });
    }

    let _ = find_binary;  // silence unused import lint
    page
}
