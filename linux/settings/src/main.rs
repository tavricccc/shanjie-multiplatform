//! shanjie-settings: the GNOME settings app with the pages and items of the Windows WinUI app.
mod draft;
mod models;
mod pages;
mod ui;

use adw::prelude::*;
use shanjie_linux::paths;

fn main() -> gtk::glib::ExitCode {
    let app = adw::Application::builder().application_id("io.github.Nanako0129.Shanjie.Settings").build();
    app.connect_activate(|app| {
        // One settings window at a time, as SettingsInstance.cs does on Windows.
        if let Some(window) = app.active_window() {
            return window.present();
        }
        match draft::Draft::open(paths::path()) {
            Ok(draft) => ui::build(app, draft, pages::all).window.present(),
            Err(error) => {
                let window = adw::ApplicationWindow::builder().application(app).title("善解設定").default_width(480).default_height(200).build();
                let status = adw::StatusPage::builder().icon_name("dialog-error-symbolic").title("無法讀取個人設定").description(error.to_string()).build();
                window.set_content(Some(&status));
                window.present();
            }
        }
    });
    app.run()
}
