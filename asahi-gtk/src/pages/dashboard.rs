use libadwaita as adw;
use adw::prelude::*;
use gtk::prelude::*;
use gtk::glib;
use std::rc::Rc;
use std::cell::{Cell, RefCell};
use crate::widgets::{self, WidgetInstance, WidgetSize, config::DashboardConfig};

pub fn build() -> gtk::Widget {
    let cfg: Rc<RefCell<DashboardConfig>> = Rc::new(RefCell::new(widgets::config::load()));
    let edit: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    let holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let sw = gtk::ScrolledWindow::new();
    sw.set_child(Some(&root));
    sw.set_vexpand(true);
    sw.set_hexpand(true);
    sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

    let rebuild: Rc<dyn Fn()> = {
        let root = root.clone();
        let cfg = cfg.clone();
        let edit = edit.clone();
        let holder = holder.clone();

        Rc::new(move || {
            // Clear
            while let Some(c) = root.first_child() {
                root.remove(&c);
            }

            // Retrieve self-reference (callable form)
            let this_rebuild: Rc<dyn Fn()> = match holder.borrow().as_ref() {
                Some(r) => r.clone(),
                None => return,
            };

            // Container with margins
            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 16);
            vbox.set_margin_start(24);
            vbox.set_margin_end(24);
            vbox.set_margin_top(24);
            vbox.set_margin_bottom(24);

            // Header
            let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let title = gtk::Label::new(None);
            title.set_markup("<span size='large' weight='bold'>Dashboard</span>");
            title.set_xalign(0.0);
            title.set_hexpand(true);
            header.append(&title);

            let edit_btn = gtk::Button::with_label(if edit.get() { "Done" } else { "Edit Dashboard" });
            if edit.get() { edit_btn.add_css_class("suggested-action"); }
            header.append(&edit_btn);

            let add_btn = gtk::Button::with_label("Add Widget");
            add_btn.add_css_class("suggested-action");
            add_btn.set_visible(edit.get());
            header.append(&add_btn);

            let reset_btn = gtk::Button::with_label("Reset");
            reset_btn.add_css_class("destructive-action");
            reset_btn.set_visible(edit.get());
            header.append(&reset_btn);

            vbox.append(&header);

            // Grid
            let grid = gtk::Grid::new();
            grid.set_column_spacing(16);
            grid.set_row_spacing(16);
            grid.set_column_homogeneous(true);

            let mut sorted: Vec<WidgetInstance> = cfg.borrow().widgets.clone();
            sorted.sort_by_key(|w| w.order);

            let mut col: i32 = 0;
            let mut row: i32 = 0;

            for inst in &sorted {
                let Some(def) = widgets::find_def(&inst.widget_type) else { continue };
                let content = (def.build)(inst);

                let size = inst.settings.get("__size_hint__")
                    .and_then(|v| v.as_str()).map(WidgetSize::from_str)
                    .unwrap_or(inst.size);

                // on_remove
                let cfg_r = cfg.clone();
                let this_r = this_rebuild.clone();
                let on_remove: widgets::OnRemove = Rc::new(move |id: String| {
                    cfg_r.borrow_mut().widgets.retain(|w| w.id != id);
                    let _ = widgets::config::save(&cfg_r.borrow());
                    this_r();
                });

                // on_move
                let cfg_m = cfg.clone();
                let this_m = this_rebuild.clone();
                let on_move: widgets::OnMove = Rc::new(move |id: String, dir: i32| {
                    let mut cfg = cfg_m.borrow_mut();
                    if let Some(i) = cfg.widgets.iter().position(|w| w.id == id) {
                        let new_i = (i as i32 + dir).max(0) as usize;
                        if new_i < cfg.widgets.len() && new_i != i {
                            cfg.widgets.swap(i, new_i);
                            for (j, w) in cfg.widgets.iter_mut().enumerate() {
                                w.order = j as u32;
                            }
                            let _ = widgets::config::save(&cfg);
                        }
                    }
                    drop(cfg);
                    this_m();
                });

                // apply
                let cfg_a = cfg.clone();
                let this_a = this_rebuild.clone();
                let apply: widgets::ApplySettings = Rc::new(move |id: String, json: serde_json::Value| {
                    let new_size = json.get("__size_hint__")
                        .and_then(|v| v.as_str()).map(WidgetSize::from_str);
                    let mut clean = json.clone();
                    if let Some(obj) = clean.as_object_mut() {
                        obj.remove("__size_hint__");
                    }
                    let mut cfg = cfg_a.borrow_mut();
                    if let Some(w) = cfg.widgets.iter_mut().find(|w| w.id == id) {
                        w.settings = clean;
                        if let Some(sz) = new_size { w.size = sz; }
                    }
                    let _ = widgets::config::save(&cfg);
                    drop(cfg);
                    this_a();
                });

                let wrapped = widgets::frame::wrap(
                    inst, content, edit.get(),
                    on_remove, on_move, apply,
                );
                widgets::frame::apply_size(&wrapped, size);

                let span = size.columns();
                grid.attach(&wrapped, col, row, span, 1);
                col += span;
                if col >= 2 { col = 0; row += 1; }
            }

            vbox.append(&grid);
            root.append(&vbox);

            // Wire header buttons (using this_rebuild clone)
            {
                let edit = edit.clone();
                let this = this_rebuild.clone();
                edit_btn.connect_clicked(move |_| {
                    edit.set(!edit.get());
                    this();
                });
            }
            {
                let cfg = cfg.clone();
                let this = this_rebuild.clone();
                add_btn.connect_clicked(move |btn| {
                    let Some(parent) = btn.root().and_then(|r| r.downcast::<gtk::Window>().ok()) else {
                        return;
                    };
                    let existing: Vec<String> = cfg.borrow().widgets.iter().map(|w| w.widget_type.clone()).collect();
                    let cfg2 = cfg.clone();
                    let this2 = this.clone();
                    widgets::picker::open(&parent, existing, Rc::new(move |wt: &'static str| {
                        let mut c = cfg2.borrow_mut();
                        let order = c.widgets.len() as u32;
                        c.widgets.push(widgets::new_instance(wt, order));
                        let _ = widgets::config::save(&c);
                        drop(c);
                        this2();
                    }));
                });
            }
            {
                let this = this_rebuild.clone();
                reset_btn.connect_clicked(move |btn| {
                    let Some(parent) = btn.root().and_then(|r| r.downcast::<gtk::Window>().ok()) else {
                        return;
                    };
                    let dlg = gtk::MessageDialog::builder()
                        .transient_for(&parent)
                        .modal(true)
                        .text("Reset dashboard to defaults?")
                        .secondary_text("Your custom widget layout will be replaced.")
                        .build();
                    dlg.add_button("Cancel", gtk::ResponseType::Cancel);
                    dlg.add_button("Reset", gtk::ResponseType::Accept);
                    let this = this.clone();
                    dlg.connect_response(move |d, resp| {
                        if resp == gtk::ResponseType::Accept {
                            let _ = widgets::config::reset();
                            this();
                        }
                        d.close();
                    });
                    dlg.present();
                });
            }
        })
    };

    // Store self-reference before first call
    *holder.borrow_mut() = Some(rebuild.clone());

    // Defer initial rebuild to idle so root() resolves the real parent window
    let rebuild_initial = rebuild.clone();
    glib::idle_add_local_once(move || {
        rebuild_initial();
    });

    sw.upcast()
}
