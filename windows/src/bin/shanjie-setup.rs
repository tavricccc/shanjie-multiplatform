//! 安裝善解.exe. Next to an extracted package it opens the WinUI setup window in ui\. As the
//! single-file installer it carries the whole package (see setup_bundle): it extracts the
//! window's own files to a temporary folder behind a short progress dialog, opens the window
//! from there and keeps extracting the models and data while the window is open, reporting
//! through `setup-extract.txt`. The folder is removed once the window closes.
#![windows_subsystem = "windows"]
use shanjie_windows::{setup_bundle, wide};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
use windows::{
    core::{w, HRESULT, PCWSTR},
    Win32::{
        Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, S_OK, WPARAM},
        System::Threading::CreateMutexW,
        UI::{
            Controls::*,
            HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
            WindowsAndMessaging::{
                MessageBoxW, SendMessageW, IDCANCEL, MB_ICONERROR, MB_ICONINFORMATION, MB_OK,
                MESSAGEBOX_STYLE,
            },
        },
    },
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

static DONE: AtomicU64 = AtomicU64::new(0);
static EARLY: AtomicU64 = AtomicU64::new(1);
static FINISHED: AtomicBool = AtomicBool::new(false);
/// Every file the setup window needs is written.
static WINDOW_READY: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);

fn main() {
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    if let Err(error) = run() {
        message(&error.to_string(), MB_ICONERROR);
    }
}

fn run() -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut file = fs::File::open(&exe)?;
    let Some(bundle) = setup_bundle::find(&mut file)? else {
        let root = exe.parent().ok_or("找不到安裝程式所在的資料夾。")?;
        // The unbundled stub only runs from a complete package folder (development builds).
        return launch(root).map(drop).map_err(|error| {
            format!("無法開啟安裝程式：{error}\n請改用 Shanjie-Windows-Setup.exe 安裝。").into()
        });
    };
    unsafe {
        CreateMutexW(None, false, w!("Local\\ShanjieSetupBundle"))?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            message("善解安裝程式已經在執行。", MB_ICONINFORMATION);
            return Ok(());
        }
    }
    let folder = scratch()?;
    let result = (|| -> Result<()> {
        EARLY.store(bundle.early.max(1), Ordering::Relaxed);
        let worker = {
            let folder = folder.clone();
            std::thread::spawn(move || -> std::result::Result<bool, String> {
                let status = folder.join(STATUS);
                let total = bundle.extracted.max(1);
                let mut shown = u64::MAX;
                let result = bundle
                    .verify(&mut file)
                    .and_then(|_| {
                        bundle.extract(
                            &mut file,
                            &folder,
                            |done| {
                                DONE.store(done, Ordering::Relaxed);
                                let percent = done * 100 / total;
                                if WINDOW_READY.load(Ordering::Relaxed) && percent != shown {
                                    shown = percent;
                                    let _ = report(&status, &format!("running {done} {total}"));
                                }
                                !CANCEL.load(Ordering::Relaxed)
                            },
                            || {
                                // The window reads this file to know the rest is still coming.
                                if report(&status, &format!("running {} {total}", bundle.early)).is_ok() {
                                    WINDOW_READY.store(true, Ordering::Relaxed);
                                }
                            },
                        )
                    })
                    .map_err(|e| e.to_string());
                let _ = match &result {
                    Ok(true) => report(&status, "done"),
                    Err(error) => report(&status, &format!("failed {error}")),
                    Ok(false) => Ok(()),
                };
                FINISHED.store(true, Ordering::Relaxed);
                result
            })
        };
        if show_progress().is_err() {
            CANCEL.store(true, Ordering::Relaxed);
        }
        if !WINDOW_READY.load(Ordering::Relaxed) || CANCEL.load(Ordering::Relaxed) {
            CANCEL.store(true, Ordering::Relaxed);
            worker.join().map_err(|_| "解壓縮安裝程式時發生錯誤。")??;
            return Ok(());
        }
        let opened = launch(&folder).map(|mut window| window.wait());
        // The window is gone: stop writing files nobody will use, then clean up.
        CANCEL.store(true, Ordering::Relaxed);
        let _ = worker.join();
        opened.map(drop).map_err(|error| format!("無法開啟安裝視窗：{error}").into())
    })();
    remove(&folder);
    result
}

const STATUS: &str = "setup-extract.txt";
/// `running <done> <total>`, `done` or `failed <message>`, replaced as a whole.
fn report(status: &Path, text: &str) -> std::io::Result<()> {
    let temporary = status.with_extension("tmp");
    fs::write(&temporary, text)?;
    fs::rename(temporary, status)
}

fn launch(root: &Path) -> std::io::Result<std::process::Child> {
    std::process::Command::new(root.join("ui").join("Shanjie.UI.exe"))
        .arg("--setup")
        .current_dir(root)
        .spawn()
}

/// A new, empty folder only this run uses, under the user's temp directory.
fn scratch() -> Result<PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let folder = std::env::temp_dir().join(format!("ShanjieSetup-{:x}{stamp:x}", std::process::id()));
    fs::create_dir(&folder)?;
    Ok(folder)
}

/// The window has exited, but its files can stay locked for a moment after.
fn remove(folder: &Path) {
    for _ in 0..20 {
        if fs::remove_dir_all(folder).is_ok() || !folder.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

fn show_progress() -> windows::core::Result<()> {
    let config = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_SHOW_PROGRESS_BAR | TDF_CALLBACK_TIMER | TDF_ALLOW_DIALOG_CANCELLATION,
        dwCommonButtons: TDCBF_CANCEL_BUTTON,
        pszWindowTitle: w!("安裝善解"),
        pszMainInstruction: w!("正在準備安裝程式"),
        pszContent: w!("正在校驗安裝檔案並準備安裝視窗。"),
        pfCallback: Some(progress),
        ..Default::default()
    };
    unsafe { TaskDialogIndirect(&config, None, None, None) }
}

unsafe extern "system" fn progress(
    hwnd: HWND,
    notification: TASKDIALOG_NOTIFICATIONS,
    _: WPARAM,
    _: LPARAM,
    _: isize,
) -> HRESULT {
    match notification {
        TDN_TIMER => {
            let percent = (DONE.load(Ordering::Relaxed) * 100 / EARLY.load(Ordering::Relaxed)).min(100);
            unsafe {
                SendMessageW(hwnd, TDM_SET_PROGRESS_BAR_POS.0 as u32, Some(WPARAM(percent as usize)), None);
                if WINDOW_READY.load(Ordering::Relaxed) || FINISHED.load(Ordering::Relaxed) {
                    SendMessageW(hwnd, TDM_CLICK_BUTTON.0 as u32, Some(WPARAM(IDCANCEL.0 as usize)), None);
                }
            }
        }
        // The timer's own click once the window can open is not a cancellation.
        TDN_BUTTON_CLICKED if !WINDOW_READY.load(Ordering::Relaxed) && !FINISHED.load(Ordering::Relaxed) => {
            CANCEL.store(true, Ordering::Relaxed)
        }
        _ => {}
    }
    S_OK
}

fn message(text: &str, icon: MESSAGEBOX_STYLE) {
    unsafe {
        MessageBoxW(None, PCWSTR(wide(text).as_ptr()), w!("安裝善解"), MB_OK | icon);
    }
}
