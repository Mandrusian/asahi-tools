use super::{WidgetInstance, WidgetSize};
use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

pub enum Field {
    Number {
        key: &'static str,
        label: &'static str,
        min: f64,
        max: f64,
        step: f64,
    },
    Toggle {
        key: &'static str,
        label: &'static str,
    },
    Choice {
        key: &'static str,
        label: &'static str,
        options: &'static [(&'static str, &'static str)],
    },
}

pub fn open(
    parent: &gtk::Window,
    title: &str,
    instance: &WidgetInstance,
    fields: Vec<Field>,
    on_save: Rc<dyn Fn(serde_json::Value)>,
) {
    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(title)
        .default_width(420)
        .default_height(480)
        .build();

    let header = adw::HeaderBar::new();
    let save_btn = gtk::Button::with_label("Save");
    save_btn.add_css_class("suggested-action");
    header.pack_end(&save_btn);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);

    let page = adw::PreferencesPage::new();

    let size_group = adw::PreferencesGroup::builder().title("Size").build();
    let size_row = adw::ComboRow::builder()
        .title("Widget size")
        .model(&gtk::StringList::new(&["Small", "Medium", "Large"]))
        .selected(match instance.size {
            WidgetSize::Small => 0,
            WidgetSize::Medium => 1,
            WidgetSize::Large => 2,
        })
        .build();
    size_group.add(&size_row);
    page.add(&size_group);

    let values: Rc<RefCell<serde_json::Map<String, serde_json::Value>>> = Rc::new(RefCell::new(
        instance.settings.as_object().cloned().unwrap_or_default(),
    ));

    if !fields.is_empty() {
        let group = adw::PreferencesGroup::builder().title("Options").build();
        for field in fields {
            match field {
                Field::Number {
                    key,
                    label,
                    min,
                    max,
                    step,
                } => {
                    let cur = instance
                        .settings
                        .get(key)
                        .and_then(|v| v.as_f64())
                        .unwrap_or(min);
                    let row = adw::ActionRow::builder().title(label).build();
                    let spin = gtk::SpinButton::with_range(min, max, step);
                    spin.set_value(cur);
                    spin.set_valign(gtk::Align::Center);
                    row.add_suffix(&spin);
                    let v = values.clone();
                    spin.connect_value_changed(move |s| {
                        v.borrow_mut()
                            .insert(key.to_string(), serde_json::json!(s.value()));
                    });
                    group.add(&row);
                }
                Field::Toggle { key, label } => {
                    let cur = instance
                        .settings
                        .get(key)
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    let row = adw::ActionRow::builder().title(label).build();
                    let sw = gtk::Switch::new();
                    sw.set_active(cur);
                    sw.set_valign(gtk::Align::Center);
                    row.add_suffix(&sw);
                    let v = values.clone();
                    sw.connect_active_notify(move |s| {
                        v.borrow_mut()
                            .insert(key.to_string(), serde_json::json!(s.is_active()));
                    });
                    group.add(&row);
                }
                Field::Choice {
                    key,
                    label,
                    options,
                } => {
                    let cur = instance
                        .settings
                        .get(key)
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let idx = options.iter().position(|(id, _)| *id == cur).unwrap_or(0) as u32;
                    let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
                    let row = adw::ComboRow::builder()
                        .title(label)
                        .model(&gtk::StringList::new(&labels))
                        .selected(idx)
                        .build();
                    let ids: Vec<&'static str> = options.iter().map(|(id, _)| *id).collect();
                    let v = values.clone();
                    row.connect_selected_notify(move |r| {
                        if let Some(id) = ids.get(r.selected() as usize) {
                            v.borrow_mut()
                                .insert(key.to_string(), serde_json::json!(*id));
                        }
                    });
                    group.add(&row);
                }
            }
        }
        page.add(&group);
    }

    toolbar.set_content(Some(&page));
    win.set_content(Some(&toolbar));

    {
        let win = win.clone();
        let values = values.clone();
        let size_row = size_row.clone();
        let on_save = on_save.clone();
        save_btn.connect_clicked(move |_| {
            let new_size = match size_row.selected() {
                0 => WidgetSize::Small,
                2 => WidgetSize::Large,
                _ => WidgetSize::Medium,
            };
            let mut map = values.borrow().clone();
            map.insert("__size__".into(), serde_json::json!(new_size.as_str()));
            on_save(serde_json::Value::Object(map));
            win.close();
        });
    }

    win.present();
}
