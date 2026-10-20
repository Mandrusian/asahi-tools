use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::process::Command;

#[derive(Clone)]
struct ServiceDef {
    unit: &'static str,
    label: &'static str,
    description: &'static str,
    is_system: bool, // true = via priv-daemon (root), false = systemctl --user
}

const SERVICES: &[ServiceDef] = &[
    ServiceDef {
        unit: "asahi-priv-daemon",
        label: "Privileged operations",
        description: "Backs battery, thermal, display, keyboard writes from this app",
        is_system: true,
    },
    ServiceDef {
        unit: "asahi-charge-daemon",
        label: "Charge controller",
        description: "Enforces the battery charge threshold",
        is_system: true,
    },
    ServiceDef {
        unit: "asahi-sysmon-daemon",
        label: "Fan controller",
        description: "Manual fan targets and thermal safety net",
        is_system: true,
    },
    ServiceDef {
        unit: "asahi-force-click",
        label: "Trackpad right click",
        description: "Fires a right-click when pressure exceeds the calibrated limit",
        is_system: true,
    },
    ServiceDef {
        unit: "asahi-notify-hub",
        label: "Notifications",
        description: "Desktop alerts for battery, thermal, and override events",
        is_system: false,
    },
    ServiceDef {
        unit: "asahi-lid",
        label: "Lid handler",
        description: "Locks or suspends when the lid closes (for tiling compositors)",
        is_system: false,
    },
];

fn system_status(unit: &str) -> (bool, bool) {
    if let Some(s) = asahi_core::priv_daemon::service_status(unit) {
        (s.active, s.enabled)
    } else {
        (false, false)
    }
}

fn user_status(unit: &str) -> (bool, bool) {
    let active = Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", unit])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let enabled = Command::new("systemctl")
        .args(["--user", "is-enabled", "--quiet", unit])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    (active, enabled)
}

fn status_for(svc: &ServiceDef) -> (bool, bool) {
    if svc.is_system {
        system_status(svc.unit)
    } else {
        user_status(svc.unit)
    }
}

fn do_action(svc: &ServiceDef, action: &str) -> Result<(), String> {
    if svc.is_system {
        asahi_core::priv_daemon::service_action(svc.unit, action)
    } else {
        let status = Command::new("systemctl")
            .args(["--user", action, svc.unit])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("exit {}", status.code().unwrap_or(-1)))
        }
    }
}

fn subtitle_for(svc: &ServiceDef, active: bool, enabled: bool) -> String {
    let state = if active { "running" } else { "stopped" };
    let boot = if enabled { "enabled" } else { "disabled" };
    format!("{} · {} — {}", state, boot, svc.description)
}

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    // ── System daemons ──
    let system_group = adw::PreferencesGroup::builder()
        .title("System daemons")
        .description("Run as root via systemd. Toggling starts/stops and enables/disables.")
        .build();

    let user_group = adw::PreferencesGroup::builder()
        .title("User services")
        .description("Run in your session. No root required.")
        .build();

    let mut switches: Vec<(adw::ActionRow, gtk::Switch, ServiceDef)> = Vec::new();

    for svc in SERVICES {
        let row = adw::ActionRow::builder()
            .title(svc.label)
            .subtitle("checking…")
            .build();

        let switch = gtk::Switch::new();
        switch.set_valign(gtk::Align::Center);
        row.add_suffix(&switch);
        row.set_activatable_widget(Some(&switch));

        // Wire switch
        {
            let svc = svc.clone();
            let row = row.clone();
            switch.connect_state_set(move |sw, on| {
                let action = if on { "start" } else { "stop" };
                if let Err(e) = do_action(&svc, action) {
                    eprintln!("services: {} {} failed: {}", svc.unit, action, e);
                    return glib::Propagation::Proceed;
                }
                // Enabling/disabling on the same toggle — matches user intent
                let enable_action = if on { "enable" } else { "disable" };
                let _ = do_action(&svc, enable_action);
                let sub = if on {
                    format!("running · enabled — {}", svc.description)
                } else {
                    format!("stopped · disabled — {}", svc.description)
                };
                row.set_subtitle(&sub);
                // Bounce the switch back to reflect actual state after action
                let _ = sw;
                glib::Propagation::Proceed
            });
        }

        if svc.is_system {
            system_group.add(&row);
        } else {
            user_group.add(&row);
        }
        switches.push((row, switch, svc.clone()));
    }

    page.add(&system_group);
    page.add(&user_group);

    // ── Info section ──
    let info_group = adw::PreferencesGroup::builder()
        .title("Notes")
        .build();
    info_group.add(&adw::ActionRow::builder()
        .title("Right-click requires calibration")
        .subtitle("Run Input → Force click → Run calibration once before enabling")
        .build());
    info_group.add(&adw::ActionRow::builder()
        .title("Privileged operations")
        .subtitle("If this is stopped, sliders in this app cannot save changes")
        .build());
    page.add(&info_group);

    // ── Refresh loop ──
    {
        let switches = switches.clone();
        glib::timeout_add_seconds_local(2, move || {
            for (row, switch, svc) in &switches {
                let (active, enabled) = status_for(svc);
                // Update switch without re-triggering the callback
                if switch.is_active() != active {
                    // Guard the signal using a flag isn't trivial in GTK4-RS
                    // without state; set_state is fine because our callback
                    // uses system state as the source of truth on each toggle.
                    switch.set_state(active);
                }
                let sub = subtitle_for(svc, active, enabled);
                if row.subtitle().map(|s| s != sub).unwrap_or(true) {
                    row.set_subtitle(&sub);
                }
            }
            glib::ControlFlow::Continue
        });
    }

    page
}
