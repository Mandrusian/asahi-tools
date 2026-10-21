use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::process::Command;

fn get_net_info() -> Vec<(&'static str, String)> {
    let mut info = vec![
        ("SSID", "Unknown"),
        ("IPv4", "Unknown"),
        ("MAC", "Unknown"),
        ("Signal", "Unknown"),
        ("DNS", "Unknown"),
    ];
    if let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SSID,SIGNAL", "dev", "wifi"])
        .output()
    {
        let txt = String::from_utf8_lossy(&out.stdout);
        for line in txt.lines() {
            if line.starts_with('*') {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    info[0].1 = Box::leak(parts[1].to_string().into_boxed_str());
                    info[3].1 = Box::leak(format!("{}%", parts[2]).into_boxed_str());
                }
            }
        }
    }
    if let Ok(out) = Command::new("ip")
        .args(["-j", "route", "get", "1.1.1.1"])
        .output()
    {
        if let Ok(json) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
            if let Some(obj) = json.first() {
                if let Some(dev) = obj.get("dev").and_then(|v| v.as_str()) {
                    if let Ok(addr_out) = Command::new("ip")
                        .args(["-j", "addr", "show", dev])
                        .output()
                    {
                        if let Ok(addr_json) =
                            serde_json::from_slice::<Vec<serde_json::Value>>(&addr_out.stdout)
                        {
                            if let Some(iface) = addr_json.first() {
                                if let Some(addr_info) =
                                    iface.get("addr_info").and_then(|v| v.as_array())
                                {
                                    for addr in addr_info {
                                        if addr.get("family").and_then(|v| v.as_str())
                                            == Some("inet")
                                        {
                                            info[1].1 = Box::leak(
                                                addr.get("local")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string()
                                                    .into_boxed_str(),
                                            );
                                        }
                                    }
                                }
                                info[2].1 = Box::leak(
                                    iface
                                        .get("address")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string()
                                        .into_boxed_str(),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    info.into_iter().map(|(k, v)| (k, v.to_string())).collect()
}

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let g = adw::PreferencesGroup::builder().title("Wi-Fi Info").build();
    for (label, val) in get_net_info() {
        let row = adw::ActionRow::builder()
            .title(label)
            .subtitle(&val)
            .build();
        if label == "IPv4" || label == "MAC" {
            let btn = gtk::Button::from_icon_name("edit-copy-symbolic");
            btn.set_valign(gtk::Align::Center);
            btn.set_has_frame(false);
            let val_clone = val.clone();
            btn.connect_clicked(move |_| {
                if let Some(display) = gtk::gdk::Display::default() {
                    display.clipboard().set_text(&val_clone);
                }
            });
            row.add_suffix(&btn);
        }
        g.add(&row);
    }
    page.add(&g);
    let g2 = adw::PreferencesGroup::builder().title("Actions").build();
    let btn_restart = gtk::Button::with_label("Restart Wi-Fi Module");
    btn_restart.set_margin_top(4);
    btn_restart.set_margin_bottom(4);
    btn_restart.connect_clicked(move |_| {
        let _ = asahi_core::priv_daemon::restart_module("brcmfmac");
    });
    g2.add(&btn_restart);
    page.add(&g2);
    page
}
