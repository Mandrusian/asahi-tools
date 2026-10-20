use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use asahi_core::{find_fan_hwmon, scan_temps, scan_fans, sysmon};

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_search_enabled(true);

    let temps_group = adw::PreferencesGroup::builder().title("Temperatures").build();
    let rows: Vec<adw::ActionRow> = (1..=8)
        .map(|i| adw::ActionRow::builder()
            .title(format!("Sensor {}", i))
            .subtitle("—")
            .visible(false)
            .build())
        .collect();
    for r in &rows { temps_group.add(r); }
    page.add(&temps_group);

    let fans_group = adw::PreferencesGroup::builder()
        .title("Fans")
        .description("Manual target pressure pins the fan; Auto returns to SMC curve")
        .build();

    let fan1_row = adw::ActionRow::builder().title("Fan 1").subtitle("—").build();
    let auto_btn = gtk::Button::with_label("Auto");
    auto_btn.set_valign(gtk::Align::Center);
    let max_btn = gtk::Button::with_label("Max");
    max_btn.set_valign(gtk::Align::Center);
    fan1_row.add_suffix(&auto_btn);
    fan1_row.add_suffix(&max_btn);
    fans_group.add(&fan1_row);

    let slider_row = adw::ActionRow::builder().title("Manual target").build();
    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 1000.0, 6500.0, 250.0);
    slider.set_hexpand(true);
    slider.set_valign(gtk::Align::Center);
    slider.set_draw_value(true);
    slider.set_margin_start(12);
    slider.set_margin_end(12);
    slider_row.add_suffix(&slider);
    fans_group.add(&slider_row);

    {
        slider.connect_value_changed(move |s| {
            let _ = sysmon::set_fan(1, s.value() as u32);
        });
    }
    {
        auto_btn.connect_clicked(move |_| { let _ = sysmon::auto_fan(1); });
    }
    {
        max_btn.connect_clicked(move |_| { let _ = sysmon::max_fan(1); });
    }

    page.add(&fans_group);

    // Refresh
    {
        let rows = rows.clone();
        let fan1_row = fan1_row.clone();
        let slider_row = slider_row.clone();
        glib::timeout_add_seconds_local(2, move || {
            if let Some(h) = find_fan_hwmon() {
                let temps = scan_temps(&h);
                let fans = scan_fans(&h);
                for (i, r) in rows.iter().enumerate() {
                    if let Some(t) = temps.get(i) {
                        r.set_title(&t.name);
                        r.set_subtitle(&format!("{:.1} °C", t.celsius));
                        r.set_visible(true);
                    } else {
                        r.set_visible(false);
                    }
                }
                if let Some(f) = fans.first() {
                    fan1_row.set_subtitle(&format!("{} rpm{}", f.rpm,
                        f.target.map(|t| format!(" (target {})", t)).unwrap_or_default()));
                    slider_row.set_subtitle(&format!("{} rpm", f.target.unwrap_or(f.rpm)));
                }
            }
            glib::ControlFlow::Continue
        });
    }

    page
}
