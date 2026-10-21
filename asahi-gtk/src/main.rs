mod helper;
mod widgets;
mod pages;

use gtk::gdk;
use gtk::prelude::*;
use gtk::glib;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

fn load_theme() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("theme/amoled.css"));
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("no display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id("com.github.mandrusian.asahi-tools").build();
    app.connect_startup(|_| load_theme());
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app).title("Asahi Settings").default_width(1050).default_height(720).build();

    let header = adw::HeaderBar::new();
    let title = adw::WindowTitle::new("Asahi Settings", "Dashboard");
    header.set_title_widget(Some(&title));

    let menu_btn = gtk::Button::from_icon_name("open-menu-symbolic");
    header.pack_start(&menu_btn);

    let banner = adw::Banner::new("Privileged Daemon Offline - Write controls disabled");
    banner.set_revealed(!asahi_core::priv_daemon::is_running());

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);

    let pages_map: Vec<(&str, &str, &str)> = vec![
        ("dash", "Dashboard", "view-app-grid-symbolic"),
        ("batt", "Battery", "battery-good-symbolic"),
        ("therm", "Thermal", "temperature-symbolic"),
        ("prof", "Profiles", "emblem-favorite-symbolic"),
        ("net", "Network", "network-wireless-symbolic"),
        ("disp", "Display", "video-display-symbolic"),
        ("input", "Input", "input-keyboard-symbolic"),
        ("sys", "System", "applications-system-symbolic"),
        ("svc", "Services", "system-run-symbolic"),
        ("about", "About", "help-about-symbolic"),
    ];

    let sidebar_list = gtk::ListBox::new();
    sidebar_list.add_css_class("sidebar-list");
    sidebar_list.set_selection_mode(gtk::SelectionMode::Single);

    let mut labels_to_toggle = Vec::new();
    let mut rows = Vec::new();

    for (id, name, icon) in &pages_map {
        let page: gtk::Widget = match *id {
            "dash" => pages::dashboard::build().upcast(),
            "batt" => pages::battery::build().upcast(),
            "therm" => pages::thermal::build().upcast(),
            "prof" => pages::profiles::build().upcast(),
            "net" => pages::network::build().upcast(),
            "disp" => pages::display::build().upcast(),
            "input" => pages::input::build().upcast(),
            "sys" => pages::system::build().upcast(),
            "svc" => pages::services::build().upcast(),
            "about" => pages::about::build().upcast(),
            _ => unreachable!(),
        };
        stack.add_titled(&page, Some(id), name);

        let row = gtk::ListBoxRow::new();
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let img = gtk::Image::from_icon_name(icon); img.set_pixel_size(18);
        let lbl = gtk::Label::new(Some(name)); lbl.set_xalign(0.0);
        hbox.append(&img); hbox.append(&lbl);
        row.set_child(Some(&hbox));
        sidebar_list.append(&row);
        rows.push(row);
        labels_to_toggle.push(lbl);
    }

    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar_box.add_css_class("sidebar"); sidebar_box.set_width_request(240);
    let sidebar_scroll = gtk::ScrolledWindow::builder().child(&sidebar_list).vexpand(true).build();
    sidebar_box.append(&sidebar_scroll);

    let split_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    split_box.append(&sidebar_box);
    split_box.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    split_box.append(&stack);
    stack.set_hexpand(true);

    let is_collapsed = Rc::new(Cell::new(false));
    {
        let sidebar = sidebar_box.clone();
        let labels = labels_to_toggle.clone();
        let collapsed_state = is_collapsed.clone();
        menu_btn.connect_clicked(move |_| {
            if collapsed_state.get() {
                sidebar.set_width_request(240);
                for l in &labels { l.set_visible(true); }
                collapsed_state.set(false);
            } else {
                sidebar.set_width_request(68);
                for l in &labels { l.set_visible(false); }
                collapsed_state.set(true);
            }
        });
    }

    {
        let stack = stack.clone();
        let map = pages_map.clone();
        sidebar_list.connect_row_activated(move |_, row| {
            if let Some((id, _, _)) = map.get(row.index() as usize) {
                stack.set_visible_child_name(id);
            }
        });
    }
    {
        let title = title.clone();
        let s_rows = rows.clone();
        let map = pages_map.clone();
        stack.connect_visible_child_name_notify(move |s| {
            let current = s.visible_child_name().map(|g| g.to_string()).unwrap_or_default();
            if let Some(idx) = map.iter().position(|(id, _, _)| *id == current) {
                title.set_subtitle(map[idx].1);
                s_rows[idx].activate();
            }
        });
    }

    sidebar_list.select_row(Some(&rows[0]));
    stack.set_visible_child_name("dash");

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    vbox.append(&banner);
    vbox.append(&split_box);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&vbox));
    window.set_content(Some(&toolbar));
    window.present();
}
