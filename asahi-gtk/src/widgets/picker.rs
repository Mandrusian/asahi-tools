use super::{registry, OnAdd};
use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::rc::Rc;

pub fn open(parent: &gtk::Window, already_present: Vec<String>, on_add: OnAdd) {
    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Add Widget")
        .default_width(480)
        .default_height(560)
        .build();

    let header = adw::HeaderBar::new();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::builder()
        .title("Available widgets")
        .build();

    let win_for_close = win.clone();
    for def in registry() {
        let available = (def.available)();
        let disabled = !available || already_present.contains(&def.widget_type.to_string());

        let row = adw::ActionRow::builder()
            .title(def.name)
            .subtitle(if !available {
                format!("{} — not available on this system", def.description)
            } else if disabled {
                format!("{} — already on dashboard", def.description)
            } else {
                def.description.to_string()
            })
            .sensitive(!disabled)
            .build();

        let img = gtk::Image::from_icon_name(def.icon);
        img.set_pixel_size(24);
        row.add_prefix(&img);

        if !disabled {
            let btn = gtk::Button::with_label("Add");
            btn.add_css_class("suggested-action");
            btn.set_valign(gtk::Align::Center);
            let on_add = on_add.clone();
            let win = win_for_close.clone();
            let wt = def.widget_type;
            btn.connect_clicked(move |_| {
                on_add(wt);
                win.close();
            });
            row.add_suffix(&btn);
        }

        group.add(&row);
    }

    page.add(&group);
    toolbar.set_content(Some(&page));
    win.set_content(Some(&toolbar));
    win.present();
}
