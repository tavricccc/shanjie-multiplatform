//! The model rows of the 一般 page (ui/MainWindow.Models.cs): the package list, rescans when the
//! Models folder changes, and the online list with resumable downloads.
use crate::ui::{buttons, Severity, Shared};
use adw::prelude::*;
use shanjie_linux::{catalog, paths};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

/// Runs `work` on a thread and hands its result back on the main loop.
fn background<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(T) + 'static) {
    let (sender, receiver) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = sender.send_blocking(work());
    });
    gtk::glib::spawn_future_local(async move {
        if let Ok(value) = receiver.recv().await {
            done(value);
        }
    });
}

/// MainWindow.RefreshModels: scanning hashes every package, so it runs off the main loop.
fn refresh(ui: &Shared, after: impl FnOnce(&Shared, &[String]) + 'static) {
    let ui = ui.clone();
    background(
        || (catalog::installed(), catalog::issues()),
        move |(installed, issues)| {
            ui.draft.borrow_mut().refresh_models(installed);
            ui.refresh();
            if !issues.is_empty() {
                ui.show(Severity::Info, &format!("未列入模型：\n{}", issues.join("\n")));
            }
            after(&ui, &issues);
        },
    );
}

pub fn rows(ui: &Shared, group: &adw::PreferencesGroup) {
    let list = gtk::StringList::new(&[]);
    let row = adw::ComboRow::builder().title("模型配置").model(&list).build();
    let names = Rc::new(RefCell::new(Vec::<String>::new()));
    {
        let row = row.clone();
        ui.sync(move |d| {
            let current: Vec<String> = d.models.iter().map(|m| m.name.clone()).collect();
            if *names.borrow() != current {
                let labels: Vec<&str> = current.iter().map(String::as_str).collect();
                list.splice(0, list.n_items(), &labels);
                *names.borrow_mut() = current;
            }
            row.set_selected(d.model_index() as u32);
            row.set_subtitle(d.model_help());
        });
    }
    let edit = ui.clone();
    row.connect_selected_notify(move |r| {
        let index = r.selected() as usize;
        edit.edit(|d| d.choose_model(index));
    });
    group.add(&row);

    let (online, folder, rescan) = (ui.clone(), ui.clone(), ui.clone());
    group.add(&buttons(vec![
        ("取得其他模型…", Box::new(move || online_dialog(&online))),
        ("開啟模型資料夾", Box::new(move || open_folder(&folder))),
        (
            "重新掃描",
            Box::new(move || {
                refresh(&rescan, |ui, issues| {
                    if issues.is_empty() {
                        ui.show(Severity::Info, "已重新掃描；只有通過驗證的模型會列入選單。");
                    }
                })
            }),
        ),
    ]));

    refresh(ui, |_, _| {});
    watch(ui);
}

/// Rescan shortly after the Models folder changes (a package published by another window or by hand).
fn watch(ui: &Shared) {
    let folder = paths::models();
    if let Err(error) = std::fs::create_dir_all(&folder) {
        ui.show(Severity::Info, &format!("模型清單無法更新，內建模型仍可使用：{error}"));
        return;
    }
    let Ok(monitor) = gtk::gio::File::for_path(&folder).monitor_directory(gtk::gio::FileMonitorFlags::WATCH_MOVES, None::<&gtk::gio::Cancellable>) else {
        return;
    };
    let pending: Rc<Cell<Option<gtk::glib::SourceId>>> = Rc::default();
    let window = ui.window.clone();
    let ui = ui.clone();
    monitor.connect_changed(move |_, file, _, _| {
        let name = file.basename().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // Downloads in progress and the list cache are not packages.
        if name.starts_with('.') || name == "Assets" || name.starts_with("catalog-cache") {
            return;
        }
        if let Some(previous) = pending.take() {
            previous.remove();
        }
        let (ui, slot) = (ui.clone(), pending.clone());
        pending.set(Some(gtk::glib::timeout_add_local_once(Duration::from_millis(700), move || {
            slot.set(None);
            refresh(&ui, |_, _| {});
        })));
    });
    // The monitor lives as long as the window.
    unsafe { window.set_data("models-monitor", monitor) };
}

fn open_folder(ui: &Shared) {
    let folder = paths::models();
    if let Err(error) = std::fs::create_dir_all(&folder) {
        return ui.show(Severity::Info, &format!("無法開啟模型資料夾：{error}"));
    }
    let launcher = gtk::FileLauncher::new(Some(&gtk::gio::File::for_path(&folder)));
    let shown = ui.clone();
    launcher.launch(Some(&ui.window), None::<&gtk::gio::Cancellable>, move |result| {
        if let Err(error) = result {
            shown.show(Severity::Info, &format!("無法開啟模型資料夾：{error}"));
        }
    });
}

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
}

/// MainWindow.OnlineModels.
fn online_dialog(ui: &Shared) {
    if OPEN.replace(true) {
        return;
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let busy = Rc::new(Cell::new(false));
    let rows: Rc<RefCell<Vec<catalog::Row>>> = Rc::default();

    let choices = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Single).css_classes(["boxed-list"]).build();
    let scroll = gtk::ScrolledWindow::builder().child(&choices).max_content_height(260).propagate_natural_height(true).hscrollbar_policy(gtk::PolicyType::Never).build();
    let progress = gtk::ProgressBar::new();
    let info = gtk::Label::builder().label("正在取得線上清單…").wrap(true).xalign(0.0).build();
    let note = gtk::Label::builder().label("只列相容配套供下載。完成後選入設定草稿，按主視窗「儲存」才套用；個人詞庫與學習資料保留。").wrap(true).xalign(0.0).css_classes(["dim-label", "caption"]).build();
    let stop = gtk::Button::builder().label("取消下載").sensitive(false).build();
    let download = gtk::Button::builder().label("下載並選取").css_classes(["suggested-action"]).sensitive(false).build();
    let actions = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(8).halign(gtk::Align::End).build();
    actions.append(&stop);
    actions.append(&download);
    let body = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).margin_top(12).margin_bottom(18).margin_start(18).margin_end(18).width_request(420).build();
    for widget in [scroll.upcast_ref::<gtk::Widget>(), progress.upcast_ref(), info.upcast_ref(), note.upcast_ref(), actions.upcast_ref()] {
        body.append(widget);
    }
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&body));
    let dialog = adw::Dialog::builder().title("取得其他模型").child(&view).content_width(480).build();

    let pulse = {
        let progress = progress.clone();
        gtk::glib::timeout_add_local(Duration::from_millis(120), move || {
            progress.pulse();
            gtk::glib::ControlFlow::Continue
        })
    };
    let pulse = Rc::new(Cell::new(Some(pulse)));
    let still = {
        let (pulse, progress) = (pulse.clone(), progress.clone());
        move || {
            if let Some(id) = pulse.take() {
                id.remove();
            }
            progress.set_fraction(0.0);
        }
    };

    {
        let (rows, busy, download) = (rows.clone(), busy.clone(), download.clone());
        choices.connect_row_selected(move |_, row| {
            let available = row.is_some_and(|r| rows.borrow().get(r.index() as usize).is_some_and(|x| x.available));
            download.set_sensitive(!busy.get() && available);
        });
    }
    {
        let (cancel, info) = (cancel.clone(), info.clone());
        stop.connect_clicked(move |_| {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            info.set_label("正在取消；下載片段會保留供下次續傳。");
        });
    }
    // Closing during a download cancels it; the dialog closes once the work stops.
    let close_requested = Rc::new(Cell::new(false));
    {
        let (busy, cancel, close_requested) = (busy.clone(), cancel.clone(), close_requested.clone());
        dialog.set_can_close(true);
        dialog.connect_close_attempt(move |_| {
            close_requested.set(true);
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = busy;
        });
    }
    {
        let cancel = cancel.clone();
        let still = still.clone();
        dialog.connect_closed(move |_| {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            still();
            OPEN.set(false);
        });
    }
    {
        let (ui, rows, busy, cancel, choices, info, stop, _, dialog_ref, pulse, still, close_requested) =
            (ui.clone(), rows.clone(), busy.clone(), cancel.clone(), choices.clone(), info.clone(), stop.clone(), download.clone(), dialog.clone(), pulse.clone(), still.clone(), close_requested.clone());
        let progress = progress.clone();
        download.connect_clicked(move |button| {
            let Some(index) = choices.selected_row().map(|r| r.index() as usize) else { return };
            let Some(spec) = rows.borrow().get(index).filter(|r| r.available).and_then(|r| r.spec.clone()) else { return };
            busy.set(true);
            dialog_ref.set_can_close(false);
            choices.set_sensitive(false);
            button.set_sensitive(false);
            stop.set_sensitive(true);
            if pulse.take().is_none() {
                progress.set_fraction(0.0);
            }
            let (sender, receiver) = async_channel::unbounded::<String>();
            let cancel_flag = cancel.clone();
            let (result_sender, result_receiver) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let progress = |s: String| {
                    let _ = sender.send_blocking(s);
                };
                let outcome = catalog::download(&spec, &progress, &cancel_flag).map_err(|e| (catalog::is_cancelled(e.as_ref()), e.to_string()));
                let _ = result_sender.send_blocking(outcome);
            });
            {
                let info = info.clone();
                gtk::glib::spawn_future_local(async move {
                    while let Ok(text) = receiver.recv().await {
                        info.set_label(&text);
                    }
                });
            }
            let (ui, busy, cancel, choices, info, stop, button, dialog, still, close_requested) =
                (ui.clone(), busy.clone(), cancel.clone(), choices.clone(), info.clone(), stop.clone(), button.clone(), dialog_ref.clone(), still.clone(), close_requested.clone());
            gtk::glib::spawn_future_local(async move {
                let Ok(outcome) = result_receiver.recv().await else { return };
                busy.set(false);
                dialog.set_can_close(true);
                choices.set_sensitive(true);
                stop.set_sensitive(false);
                still();
                match outcome {
                    Ok(choice) => {
                        let id = choice.id.clone();
                        refresh(&ui, move |ui, _| {
                            let selected = ui.draft.borrow_mut().select_model(&id);
                            ui.refresh();
                            match selected {
                                Ok(()) => ui.show(Severity::Info, "模型已下載、驗證並選取，按儲存後套用。其他設定草稿已保留。"),
                                Err(error) => ui.show(Severity::Error, &error.to_string()),
                            }
                        });
                        dialog.close();
                        return;
                    }
                    Err((true, _)) => info.set_label("已取消，原模型保持不變；重新開啟清單可續傳。"),
                    Err((false, error)) => info.set_label(&format!("尚未套用模型：{error}")),
                }
                if close_requested.get() {
                    dialog.close();
                } else {
                    button.set_sensitive(!cancel.load(std::sync::atomic::Ordering::Relaxed));
                }
            });
        });
    }
    dialog.present(Some(&ui.window));

    // Fetch the list.
    let (rows_slot, choices_list, info_label, still_list) = (rows.clone(), choices.clone(), info.clone(), still.clone());
    let cancel_list = cancel.clone();
    background(
        move || catalog::online(&cancel_list).map_err(|e| (catalog::is_cancelled(e.as_ref()), e.to_string())),
        move |result| {
            still_list();
            match result {
                Ok((rows, status)) => {
                    for row in &rows {
                        let line = adw::ActionRow::builder().title(&row.name).subtitle(&row.state).build();
                        line.set_sensitive(row.available);
                        choices_list.append(&line);
                    }
                    *rows_slot.borrow_mut() = rows;
                    info_label.set_label(&status);
                }
                Err((true, _)) => {}
                Err((false, error)) => info_label.set_label(&format!("無法取得線上清單：{error}")),
            }
        },
    );
}
