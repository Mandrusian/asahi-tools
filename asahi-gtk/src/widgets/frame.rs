use gtk::prelude::*;
use std::rc::Rc;
use super::{WidgetInstance, WidgetSize, ApplySettings, OnRemove, OnMove, find_def};

pub fn wrap(
    instance: &WidgetInstance,
    content: gtk::Widget,
    edit_mode: bool,
    on_remove: OnRemove,
    on_move: OnMove,
    apply: ApplySettings,
) -> gtk::Widget {
    if !edit_mode {
        return content;
    }

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 4);
    outer.add_css_class("widget-edit-frame");

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header.add_css_class("widget-edit-header");

    let up_btn = gtk::Button::from_icon_name("go-up-symbolic");
    up_btn.add_css_class("widget-edit-btn");
    up_btn.set_tooltip_text(Some("Move up"));
    let down_btn = gtk::Button::from_icon_name("go-down-symbolic");
    down_btn.add_css_class("widget-edit-btn");
    down_btn.set_tooltip_text(Some("Move down"));

    let def = find_def(&instance.widget_type);
    let name = def.map(|d| d.name).unwrap_or(&instance.widget_type).to_string();
    let title = gtk::Label::new(Some(&name));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.add_css_class("dim-label");

    let gear_btn = gtk::Button::from_icon_name("emblem-system-symbolic");
    gear_btn.add_css_class("widget-edit-btn");
    gear_btn.set_tooltip_text(Some("Settings"));
    let remove_btn = gtk::Button::from_icon_name("window-close-symbolic");
    remove_btn.add_css_class("widget-edit-btn");
    remove_btn.set_tooltip_text(Some("Remove"));

    header.append(&up_btn);
    header.append(&down_btn);
    header.append(&title);
    header.append(&gear_btn);
    header.append(&remove_btn);

    outer.append(&header);
    outer.append(&content);

    {
        let id = instance.id.clone();
        let on_move = on_move.clone();
        up_btn.connect_clicked(move |_| on_move(id.clone(), -1));
    }
    {
        let id = instance.id.clone();
        let on_move = on_move.clone();
        down_btn.connect_clicked(move |_| on_move(id.clone(), 1));
    }
    {
        let id = instance.id.clone();
        let on_remove = on_remove.clone();
        remove_btn.connect_clicked(move |_| on_remove(id.clone()));
    }
    {
        let inst = instance.clone();
        let apply = apply.clone();
        let name = name.clone();
        gear_btn.connect_clicked(move |btn| {
            let Some(parent) = btn.root().and_then(|r| r.downcast::<gtk::Window>().ok()) else {
                return;
            };
            let schema = find_def(&inst.widget_type)
                .map(|d| (d.settings_schema)(&inst))
                .unwrap_or_default();
            let id = inst.id.clone();
            let apply = apply.clone();
            super::settings::open(
                &parent,
                &format!("{} Settings", name),
                &inst,
                schema,
                Rc::new(move |mut json| {
                    let size = json.as_object_mut()
                        .and_then(|m| m.remove("__size__"))
                        .and_then(|v| v.as_str().map(|s| s.to_string()));
                    if let Some(sz) = size {
                        if let Some(obj) = json.as_object_mut() {
                            obj.insert("__size_hint__".into(), serde_json::json!(sz));
                        }
                    }
                    apply(id.clone(), json);
                }),
            );
        });
    }

    outer.upcast()
}

pub fn apply_size(widget: &gtk::Widget, size: WidgetSize) {
    widget.set_valign(gtk::Align::Start);
    match size {
        WidgetSize::Small => widget.add_css_class("widget-small"),
        WidgetSize::Medium => widget.add_css_class("widget-medium"),
        WidgetSize::Large => widget.add_css_class("widget-large"),
    }
}
