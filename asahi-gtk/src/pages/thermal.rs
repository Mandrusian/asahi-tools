use adw::prelude::*;
use asahi_core::sysmon;
use gtk::prelude::*;
use libadwaita as adw;

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();

    let g1 = adw::PreferencesGroup::builder()
        .title("Temperatures")
        .build();
    for (name, temp) in crate::helper::get_hwmon_sensors() {
        let row = adw::ActionRow::builder()
            .title(&name)
            .subtitle(&format!("{:.1} C", temp))
            .build();
        let bar = gtk::LevelBar::new();
        bar.set_min_value(0.0);
        bar.set_max_value(100.0);
        bar.set_value(temp);
        bar.set_valign(gtk::Align::Center);
        bar.set_size_request(150, -1);
        if temp > 80.0 {
            bar.add_css_class("error");
        } else if temp >= 60.0 {
            bar.add_css_class("warning");
        } else {
            bar.add_css_class("success");
        }
        row.add_suffix(&bar);
        g1.add(&row);
    }
    page.add(&g1);

    let g2 = adw::PreferencesGroup::builder().title("Fans").build();
    let fans = crate::helper::get_hwmon_fans();
    for (i, (name, rpm, min, max)) in fans.iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(name)
            .subtitle(&format!("{} rpm", rpm))
            .build();
        let btn_a = gtk::Button::with_label("Auto");
        btn_a.set_valign(gtk::Align::Center);
        let btn_m = gtk::Button::with_label("Max");
        btn_m.set_valign(gtk::Align::Center);
        row.add_suffix(&btn_a);
        row.add_suffix(&btn_m);
        let idx = (i + 1) as u32;
        btn_a.connect_clicked(move |_| {
            let _ = sysmon::auto_fan(idx);
        });
        btn_m.connect_clicked(move |_| {
            let _ = sysmon::max_fan(idx);
        });
        g2.add(&row);

        let slider_row = adw::ActionRow::builder().title("Manual target").build();
        let slider = gtk::Scale::with_range(
            gtk::Orientation::Horizontal,
            *min as f64,
            *max as f64,
            100.0,
        );
        slider.set_hexpand(true);
        slider.set_size_request(250, -1);
        slider.set_valign(gtk::Align::Center);
        slider_row.add_suffix(&slider);
        slider.connect_value_changed(move |s| {
            let _ = sysmon::set_fan(idx, s.value() as u32);
        });
        g2.add(&slider_row);
    }
    page.add(&g2);
    page
}
