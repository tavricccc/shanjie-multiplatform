//! The window (ui/MainWindow.xaml): pages in a sidebar, a status bar and the fixed save bar. Every
//! row writes the draft directly; after each change `refresh` re-reads the draft into every row, so
//! enable conditions, help texts and presets stay consistent without per-row wiring.
use crate::draft::Draft;
use adw::prelude::*;
use shanjie_linux::store;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Success,
    Error,
}

pub struct Ui {
    pub draft: RefCell<Draft>,
    pub window: adw::ApplicationWindow,
    syncing: Cell<bool>,
    syncs: RefCell<Vec<Box<dyn Fn(&Draft)>>>,
    status: gtk::Revealer,
    status_box: gtk::Box,
    status_icon: gtk::Image,
    status_label: gtk::Label,
    draft_help: gtk::Label,
    pub sidebar: gtk::ListBox,
}

pub type Shared = Rc<Ui>;

impl Ui {
    /// Registers how a widget shows the draft; it runs now and after every change.
    pub fn sync(&self, f: impl Fn(&Draft) + 'static) {
        let _guard = Syncing::new(self);
        f(&self.draft.borrow());
        self.syncs.borrow_mut().push(Box::new(f));
    }

    pub fn syncing(&self) -> bool {
        self.syncing.get()
    }

    pub fn refresh(&self) {
        let _guard = Syncing::new(self);
        let draft = self.draft.borrow();
        for f in self.syncs.borrow().iter() {
            f(&draft);
        }
        self.draft_help.set_label(draft.draft_help());
    }

    /// Applies a user edit; ignored while the rows are being refreshed from the draft.
    pub fn edit(&self, f: impl FnOnce(&mut Draft)) {
        if self.syncing() {
            return;
        }
        f(&mut self.draft.borrow_mut());
        // A new edit hides a success message, as the WinUI InfoBar does.
        if self.status_box.has_css_class("success") {
            self.status.set_reveal_child(false);
        }
        self.refresh();
    }

    pub fn show(&self, severity: Severity, message: &str) {
        for class in ["info", "success", "error"] {
            self.status_box.remove_css_class(class);
        }
        let (class, icon) = match severity {
            Severity::Info => ("info", "dialog-information-symbolic"),
            Severity::Success => ("success", "object-select-symbolic"),
            Severity::Error => ("error", "dialog-error-symbolic"),
        };
        self.status_box.add_css_class(class);
        self.status_icon.set_icon_name(Some(icon));
        self.status_label.set_label(message);
        self.status.set_reveal_child(!message.trim().is_empty());
    }

    /// ShowResult: run an action that returns its status text.
    pub fn result(&self, success: Severity, f: impl FnOnce(&mut Draft) -> shanjie_shell::profile::Result<String>) {
        let outcome = f(&mut self.draft.borrow_mut());
        self.refresh();
        match outcome {
            Ok(message) => self.show(success, &message),
            Err(error) => self.show(Severity::Error, &error.to_string()),
        }
    }
}

struct Syncing<'a>(&'a Ui, bool);
impl<'a> Syncing<'a> {
    fn new(ui: &'a Ui) -> Self {
        let previous = ui.syncing.replace(true);
        Self(ui, previous)
    }
}
impl Drop for Syncing<'_> {
    fn drop(&mut self) {
        self.0.syncing.set(self.1);
    }
}

// Rows bound to `[section] key`.

pub fn switch(ui: &Shared, title: &str, subtitle: &str, section: &'static str, key: &'static str) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder().title(title).subtitle(subtitle).build();
    let shown = row.clone();
    ui.sync(move |d| shown.set_active(d.flag(section, key)));
    let edit = ui.clone();
    row.connect_active_notify(move |r| {
        let on = r.is_active();
        edit.edit(|d| d.set(section, key, on as u32));
    });
    row
}

/// A choice stored as one of `values`; a value outside the list shows no selection.
pub fn choice(ui: &Shared, title: &str, subtitle: &str, section: &'static str, key: &'static str, options: &'static [(&'static str, &'static str)]) -> adw::ComboRow {
    let labels: Vec<&str> = options.iter().map(|(_, label)| *label).collect();
    let row = adw::ComboRow::builder().title(title).subtitle(subtitle).model(&gtk::StringList::new(&labels)).build();
    let shown = row.clone();
    ui.sync(move |d| {
        let value = d.text(section, key);
        shown.set_selected(options.iter().position(|(v, _)| *v == value).map_or(gtk::INVALID_LIST_POSITION, |i| i as u32));
    });
    let edit = ui.clone();
    row.connect_selected_notify(move |r| {
        if let Some((value, _)) = options.get(r.selected() as usize) {
            edit.edit(|d| d.set(section, key, value));
        }
    });
    row
}

pub fn number(ui: &Shared, title: &str, subtitle: &str, section: &'static str, key: &'static str, (min, max, step): (u32, u32, u32)) -> adw::SpinRow {
    let row = adw::SpinRow::with_range(min as f64, max as f64, step as f64);
    row.set_title(title);
    row.set_subtitle(subtitle);
    row.set_numeric(true);
    let shown = row.clone();
    ui.sync(move |d| shown.set_value(d.number(section, key) as f64));
    let edit = ui.clone();
    row.connect_value_notify(move |r| {
        let value = r.value().round() as u32;
        edit.edit(|d| d.set(section, key, value));
    });
    row
}

pub fn enabled(ui: &Shared, widget: &impl IsA<gtk::Widget>, when: impl Fn(&Draft) -> bool + 'static) {
    let widget = widget.clone().upcast::<gtk::Widget>();
    ui.sync(move |d| widget.set_sensitive(when(d)));
}

pub fn group(title: &str, description: &str) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(title).build();
    if !description.is_empty() {
        group.set_description(Some(description));
    }
    group
}

pub fn button_row(title: &str, icon: &str, f: impl Fn() + 'static) -> adw::ButtonRow {
    let row = adw::ButtonRow::builder().title(title).build();
    if !icon.is_empty() {
        row.set_start_icon_name(Some(icon));
    }
    row.connect_activated(move |_| f());
    row
}

/// A row of plain buttons under a group (the WinUI pages place buttons in a horizontal panel).
pub fn buttons(items: Vec<(&str, Box<dyn Fn()>)>) -> gtk::Box {
    let bar = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(8).margin_top(12).build();
    for (label, f) in items {
        let button = gtk::Button::with_label(label);
        button.connect_clicked(move |_| f());
        bar.append(&button);
    }
    bar
}

pub fn note(text: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).wrap(true).xalign(0.0).margin_top(12).css_classes(["dim-label", "caption"]).build();
    label.set_selectable(true);
    label
}

const CSS: &str = "
.status { padding: 8px 12px; border-radius: 10px; margin: 8px 12px 0 12px; }
.status.info { background: alpha(@accent_bg_color, 0.14); }
.status.success { background: alpha(@success_bg_color, 0.18); }
.status.error { background: alpha(@error_bg_color, 0.18); }
.footer { padding: 8px 12px; }
.punctuation checkbutton { min-width: 140px; }
";

pub struct Page {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub content: adw::PreferencesPage,
}

pub fn build(app: &adw::Application, draft: Draft, pages: impl FnOnce(&Shared) -> Vec<Page>) -> Shared {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    let window = adw::ApplicationWindow::builder().application(app).title("善解設定").default_width(840).default_height(680).build();
    let status_icon = gtk::Image::new();
    let status_label = gtk::Label::builder().wrap(true).xalign(0.0).hexpand(true).selectable(true).build();
    let close = gtk::Button::builder().icon_name("window-close-symbolic").css_classes(["flat", "circular"]).valign(gtk::Align::Center).tooltip_text("關閉訊息").build();
    let status_box = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(10).css_classes(["status"]).build();
    status_box.append(&status_icon);
    status_box.append(&status_label);
    status_box.append(&close);
    let status = gtk::Revealer::builder().child(&status_box).transition_type(gtk::RevealerTransitionType::SlideUp).build();
    let reveal = status.clone();
    close.connect_clicked(move |_| reveal.set_reveal_child(false));

    let draft_help = gtk::Label::builder().css_classes(["dim-label"]).build();
    let sidebar = gtk::ListBox::builder().css_classes(["navigation-sidebar"]).build();
    let ui = Rc::new(Ui {
        draft: RefCell::new(draft),
        window: window.clone(),
        syncing: Cell::new(false),
        syncs: RefCell::new(Vec::new()),
        status: status.clone(),
        status_box,
        status_icon,
        status_label,
        draft_help: draft_help.clone(),
        sidebar: sidebar.clone(),
    });

    // Theme: 0 follows the desktop, 1 light, 2 dark (the settings window; GNOME draws the candidates).
    ui.sync(|d| {
        let scheme = match d.number("appearance", "theme") {
            1 => adw::ColorScheme::ForceLight,
            2 => adw::ColorScheme::ForceDark,
            _ => adw::ColorScheme::Default,
        };
        adw::StyleManager::default().set_color_scheme(scheme);
    });

    let pages = pages(&ui);
    let stack = gtk::Stack::builder().transition_type(gtk::StackTransitionType::Crossfade).build();
    for page in &pages {
        stack.add_named(&page.content, Some(page.id));
        let row = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(12).margin_top(6).margin_bottom(6).margin_start(6).build();
        row.append(&gtk::Image::from_icon_name(page.icon));
        row.append(&gtk::Label::builder().label(page.title).xalign(0.0).build());
        sidebar.append(&row);
    }
    let content_page = adw::NavigationPage::builder().title(pages[0].title).child(&{
        let view = adw::ToolbarView::new();
        view.add_top_bar(&adw::HeaderBar::new());
        view.set_content(Some(&stack));
        view
    }).build();
    let split = adw::NavigationSplitView::builder()
        .sidebar(&adw::NavigationPage::builder().title("善解設定").child(&{
            let view = adw::ToolbarView::new();
            view.add_top_bar(&adw::HeaderBar::new());
            view.set_content(Some(&gtk::ScrolledWindow::builder().child(&sidebar).hscrollbar_policy(gtk::PolicyType::Never).build()));
            view
        }).build())
        .content(&content_page)
        .min_sidebar_width(208.0)
        .max_sidebar_width(208.0)
        .build();
    let titles: Vec<_> = pages.iter().map(|p| (p.id, p.title, p.content.clone())).collect();
    {
        let (split, stack, content_page) = (split.clone(), stack.clone(), content_page.clone());
        sidebar.connect_row_selected(move |_, row| {
            let Some(row) = row else { return };
            let (id, title, page) = &titles[row.index() as usize];
            stack.set_visible_child_name(id);
            content_page.set_title(title);
            page.scroll_to_top();
            split.set_show_content(true);
        });
    }
    sidebar.select_row(sidebar.row_at_index(0).as_ref());

    // The fixed save bar.
    let reload = gtk::Button::with_label("重新載入已儲存設定");
    let restart = gtk::Button::builder().label("重新啟動輸入法").tooltip_text("儲存設定、提交目前組字並重載善解核心").build();
    let cancel = gtk::Button::with_label("取消");
    let save = gtk::Button::builder().label("儲存").css_classes(["suggested-action"]).build();
    let footer = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(8).css_classes(["footer"]).build();
    footer.append(&reload);
    footer.append(&draft_help);
    footer.append(&gtk::Box::builder().hexpand(true).build());
    footer.append(&restart);
    footer.append(&cancel);
    footer.append(&save);
    let bottom = gtk::Box::builder().orientation(gtk::Orientation::Vertical).build();
    bottom.append(&status);
    bottom.append(&footer);

    let outer = adw::ToolbarView::builder().bottom_bar_style(adw::ToolbarStyle::Raised).content(&split).build();
    outer.add_bottom_bar(&bottom);
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(adw::BreakpointConditionLengthType::MaxWidth, 600.0, adw::LengthUnit::Sp));
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(breakpoint);
    window.set_content(Some(&outer));

    {
        let ui = ui.clone();
        reload.connect_clicked(move |_| {
            let loaded = ui.draft.borrow_mut().load();
            ui.refresh();
            match loaded {
                Ok(()) => ui.status.set_reveal_child(false),
                Err(error) => ui.show(Severity::Error, &format!("無法讀取已儲存設定：{error}")),
            }
            crate::pages::refresh_presets(&ui);
        });
    }
    {
        let window = window.clone();
        cancel.connect_clicked(move |_| window.close());
    }
    {
        let ui = ui.clone();
        save.connect_clicked(move |_| {
            save_draft(&ui);
        });
    }
    {
        let ui = ui.clone();
        restart.connect_clicked(move |_| restart_input(&ui));
    }
    ui.refresh();
    ui
}

/// MainWindow.SaveDraft.
pub fn save_draft(ui: &Shared) -> bool {
    let saved = ui.draft.borrow_mut().save();
    ui.refresh();
    match saved {
        Ok(message) => {
            ui.show(Severity::Success, message);
            true
        }
        Err(error) => {
            ui.show(Severity::Error, &error.to_string());
            false
        }
    }
}

/// MainWindow.RestartInput: save a pending draft, reload it and rebuild the core in every context.
pub fn restart_input(ui: &Shared) {
    if ui.draft.borrow().changed && !save_draft(ui) {
        return;
    }
    let result = ui.draft.borrow_mut().load().map_err(|e| format!("無法讀取已儲存設定：{e}")).and_then(|_| store::restart().map_err(|e| format!("無法重新啟動輸入法：{e}")));
    ui.refresh();
    match result {
        Ok(()) => ui.show(Severity::Success, "已通知善解重新啟動並讀取已儲存設定。正在輸入的組字會先提交。"),
        Err(message) => ui.show(Severity::Error, &message),
    }
}
