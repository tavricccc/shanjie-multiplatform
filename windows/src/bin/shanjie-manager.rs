use shanjie_windows::{
    installer::{self, UserData},
    profile::{self, Result},
    wide,
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    },
};

fn elevate(action: &str) -> Result<()> {
    unsafe {
        let executable = std::env::current_exe()?;
        let arguments = wide(&format!(
            "{action} --user-sid {}",
            shanjie_windows::installation::user_sid()?
        ));
        let file = wide(&executable.to_string_lossy());
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(arguments.as_ptr()),
            nShow: 0,
            ..Default::default()
        };
        ShellExecuteExW(&mut info)?;
        let process = info.hProcess; // SHELLEXECUTEINFOW is packed on x86.
        // This thread may own Shell/TSF windows. Keep dispatching messages and
        // stop waiting after ten minutes without terminating an active install.
        let waited = shanjie_windows::process_wait::wait_for_exit(
            process,
            std::time::Duration::from_secs(600),
        );
        if !process.is_invalid() {
            let _ = CloseHandle(process);
        }
        let code = waited?;
        if code != 0 {
            return Err(format!("操作未完成（退出碼 {code}）。").into());
        }
        Ok(())
    }
}
fn announce(text: &str, dialog: bool) {
    println!("{text}");
    if dialog {
        unsafe {
            MessageBoxW(
                None,
                PCWSTR(wide(text).as_ptr()),
                w!("善解 Windows"),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let Some(at) = args.iter().position(|value| value == "--user-sid") {
        if args.get(at + 1).map(String::as_str)
            != Some(shanjie_windows::installation::user_sid()?.as_str())
        {
            return Err("請使用同一登入帳號提升權限，以便更新該帳號的輸入法清單。".into());
        }
    }
    let explicit = if let Some(at) = args.iter().position(|value| value == "--keyboard") {
        Some(match args.get(at + 1).map(String::as_str) {
            Some("standard") => 1,
            Some("eten") => 2,
            _ => return Err("--keyboard 請填 standard 或 eten。".into()),
        })
    } else {
        None
    };
    let choose = args.iter().any(|value| value == "--choose-keyboard");
    if args.first().is_some_and(|s| s == "--elevate") {
        if args.get(1).is_some_and(|action| action == "install") {
            let Some(mask) =
                installer::choose_keyboard(&installer::install_root(), explicit, choose)?
            else {
                announce("已取消安裝，現有安裝及資料已保留。", true);
                return Ok(());
            };
            return elevate(&format!(
                "install --keyboard {} --dialog",
                if mask == 1 { "standard" } else { "eten" }
            ));
        }
        return elevate(&(args[1..].join(" ") + " --dialog"));
    }
    let dialog = args.iter().any(|s| s == "--dialog");
    let target = installer::install_root();
    if cfg!(target_pointer_width = "32")
        && args
            .first()
            .is_some_and(|s| s == "install" || s == "uninstall")
    {
        return Err("請使用 64 位元善解安裝工具。".into());
    }
    match args.first().map(String::as_str) {
        Some("verify-package") => {
            let path=args.get(1).ok_or("請指定安裝包目錄。")?;
            let files=installer::verified_package(std::path::Path::new(path))?;
            println!("PASS: {} verified package files, latest model only",files.len());
        }
        Some("apply-profile") => {
            let path=args.get(1).ok_or("請指定操作範本。")?;
            profile::apply_preset(std::path::Path::new(path))?;
            announce("已套用操作設定；詞庫、學習與外觀已保留。",dialog);
        }
        Some("profiles") => installer::profiles(args.iter().any(|s|s=="--remove"))?,
        Some("install") => {
            let package=std::env::current_exe()?.parent().unwrap().parent().unwrap().to_owned();
            match installer::install(&package,&target,explicit,choose)? {
                Some(pending)=>announce(&format!("已更新善解，並設定登入後啟動常駐核心。\n舊檔保留於原目錄旁的 .deleted_files。\n另有 {pending} 個使用中的項目暫留原位，關閉相關程式後可再處理。\n請重新開啟使用輸入法的程式，並登出再登入。"),dialog),
                None=>announce("已取消安裝，現有安裝及資料已保留。",dialog),
            }
        }
        Some("uninstall") => {
            let delete=args.iter().any(|s|s=="--delete-user-data");let keep=args.iter().any(|s|s=="--keep-user-data");
            if delete&&keep{return Err("請只選一種個人資料處理方式。".into());}
            let policy=if delete{UserData::Delete}else if keep{UserData::Keep}else{UserData::Ask};
            match installer::uninstall(&target,policy)? {
                Some(pending)=>announce(&format!("已移除善解與登入自動啟動設定。\n檔案保留於原目錄旁的 .deleted_files。\n另有 {pending} 個使用中的項目暫留原位，關閉相關程式後可再處理。"),dialog),
                None=>announce("已取消解除安裝，沒有修改任何資料。",dialog),
            }
        }
        _=>return Err("用法：shanjie-manager [--elevate] install [--keyboard standard|eten|--choose-keyboard]；uninstall [--keep-user-data|--delete-user-data]；另支援 verify-package、apply-profile、profiles。".into()),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        let message = match installer::record_manager_failure(&error.to_string()) {
            Ok(folder) => {
                if error.to_string().contains("紀錄：") {
                    error.to_string()
                } else {
                    format!("{error}\n紀錄：{}", folder.display())
                }
            }
            Err(log) => format!("{error}\n另有紀錄保存失敗：{log}"),
        };
        eprintln!("{message}");
        if std::env::args().any(|s| s == "--dialog") {
            unsafe {
                MessageBoxW(
                    None,
                    PCWSTR(wide(&message).as_ptr()),
                    w!("善解操作未完成"),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        std::process::exit(1);
    }
}
