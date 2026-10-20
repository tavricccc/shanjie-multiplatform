//! Package-root launcher (安裝善解.exe): opens the WinUI setup window shipped in ui\.
#![windows_subsystem = "windows"]
use windows::{
    core::{w, PCWSTR},
    Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK},
};

fn main() {
    let started = std::env::current_exe().and_then(|exe| {
        let root = exe.parent().ok_or(std::io::ErrorKind::NotFound)?.to_owned();
        std::process::Command::new(root.join("ui").join("Shanjie.UI.exe"))
            .arg("--setup")
            .current_dir(&root)
            .spawn()
    });
    if let Err(error) = started {
        // Usually opened straight from the zip, where ui\ is not next to this file.
        let text = format!("無法開啟安裝程式：{error}\n請先把 Shanjie-Windows.zip 完整解壓縮，再執行「安裝善解」。");
        unsafe {
            MessageBoxW(None, PCWSTR(shanjie_windows::wide(&text).as_ptr()), w!("安裝善解"), MB_OK | MB_ICONERROR);
        }
    }
}
