use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::process::Command;

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let g = adw::PreferencesGroup::builder().title("Asahi Tools").build();
    
    g.add(&adw::ActionRow::builder().title("Version").subtitle(env!("CARGO_PKG_VERSION")).build());
    g.add(&adw::ActionRow::builder().title("Repository").subtitle("github.com/Mandrusian/asahi-tools").build());
    g.add(&adw::ActionRow::builder().title("License").subtitle("MIT").build());

    let btn_about = gtk::Button::with_label("Open About Dialog");
    btn_about.set_margin_top(8); btn_about.set_margin_bottom(8);
    btn_about.connect_clicked(move |b| {
        if let Some(win) = b.root().and_then(|r| r.downcast::<gtk::Window>().ok()) {
            let about = adw::AboutWindow::builder()
                .transient_for(&win)
                .application_name("Asahi Settings")
                .application_icon("asahi-tools")
                .developer_name("Asahi Tools Contributors")
                .version(env!("CARGO_PKG_VERSION"))
                .website("https://github.com/Mandrusian/asahi-tools")
                .license_type(gtk::License::MitX11)
                .build();
            about.present();
        }
    });
    g.add(&btn_about);
    page.add(&g);
    page
}
