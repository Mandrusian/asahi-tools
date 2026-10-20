use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;

pub fn build() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_search_enabled(true);
    let g = adw::PreferencesGroup::builder().title("asahi-tools").build();
    g.add(&adw::ActionRow::builder().title("Version").subtitle(env!("CARGO_PKG_VERSION")).build());
    g.add(&adw::ActionRow::builder().title("Repo").subtitle("github.com/Mandrusian/asahi-tools").build());
    g.add(&adw::ActionRow::builder().title("Platform").subtitle("Apple Silicon · Asahi Linux").build());
    page.add(&g);

    let tools_group = adw::PreferencesGroup::builder().title("Bundled tools").build();
    for (n, d) in [
        ("asahi-tui", "Terminal launcher + dashboard"),
        ("asahi-gtk", "This app"),
        ("asahi-charge-daemon", "Battery charge controller"),
        ("sysmon-daemon", "Fan control daemon"),
        ("asahi-notify-hub", "Desktop notifications"),
        ("asahi-force-click", "Trackpad secondary click"),
        ("asahi-kernel-manager", "Kernel update TUI"),
        ("asahi-diag", "Diagnostic bundle"),
    ] {
        tools_group.add(&adw::ActionRow::builder().title(n).subtitle(d).build());
    }
    page.add(&tools_group);
    page
}
