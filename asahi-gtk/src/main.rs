mod helper;
mod pages;

use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("com.github.mandrusian.asahi-tools")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Asahi Settings")
        .default_width(960)
        .default_height(720)
        .build();

    let header = adw::HeaderBar::new();
    let title_widget = adw::WindowTitle::new("Asahi Settings", "Battery · Thermal · Power · Input");
    header.set_title_widget(Some(&title_widget));

    let about_btn = gtk::Button::from_icon_name("help-about-symbolic");
    about_btn.set_tooltip_text(Some("About asahi-tools"));
    {
        let window_for_about = window.clone();
        about_btn.connect_clicked(move |_| {
            let about = adw::AboutWindow::builder()
                .application_name("Asahi Settings")
                .application_icon("asahi-tools")
                .developer_name("asahi-tools")
                .version(env!("CARGO_PKG_VERSION"))
                .website("https://github.com/Mandrusian/asahi-tools")
                .issue_url("https://github.com/Mandrusian/asahi-tools/issues")
                .license_type(gtk::License::MitX11)
                .developers(vec!["asahi-tools contributors"])
                .copyright("© 2026 asahi-tools contributors")
                .build();
            about.present(Some(&window_for_about));
        });
    }
    header.pack_end(&about_btn);

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);

    stack.add_titled(&pages::dashboard::build(), Some("dash"), "Dashboard");
    stack.add_titled(&pages::battery::build(), Some("batt"), "Battery");
    stack.add_titled(&pages::thermal::build(), Some("therm"), "Thermal");
    stack.add_titled(&pages::power::build(), Some("power"), "Power");
    stack.add_titled(&pages::network::build(), Some("net"), "Network");
    stack.add_titled(&pages::display::build(), Some("disp"), "Display");
    stack.add_titled(&pages::input::build(), Some("input"), "Input");
    stack.add_titled(&pages::system::build(), Some("sys"), "System");
    stack.add_titled(&pages::services::build(), Some("svc"), "Services");
    stack.add_titled(&pages::profiles::build(), Some("prof"), "Profiles");
    stack.add_titled(&pages::about::build(), Some("about"), "About");

    let switcher = gtk::StackSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_margin_top(6);
    switcher.set_margin_bottom(6);

    let top_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    top_box.append(&switcher);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.add_top_bar(&top_box);
    toolbar.set_content(Some(&stack));

    window.set_content(Some(&toolbar));
    window.present();
}
