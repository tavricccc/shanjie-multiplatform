use shanjie_windows::{
    installer::{self, UserData},
    profile::{self, Result},
    wide,
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::*,
        System::Threading::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    },
};

fn elevate(action: &str) -> Result<()> {
    unsafe {
        let executable = std::env::current_exe()?;
        let arguments = wide(action);
        let file = wide(&executable.to_string_lossy());
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: PCWSTR(arguments.as_ptr()),
            nShow: 0,
            ..Default::default()
        };
        ShellExecuteExW(&mut info)?;
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 0;
        GetExitCodeProcess(info.hProcess, &mut code)?;
        CloseHandle(info.hProcess)?;
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
    if args.first().is_some_and(|s| s == "--elevate") {
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
            let pending=installer::install(&package,&target)?;
            announce(&format!("已更新善解並清理舊版。\n{pending} 個使用中的檔案／目錄將在重新啟動後刪除。\n請重新開啟使用輸入法的程式，或登出再登入。"),dialog);
        }
        Some("uninstall") => {
            let delete=args.iter().any(|s|s=="--delete-user-data");let keep=args.iter().any(|s|s=="--keep-user-data");
            if delete&&keep{return Err("請只選一種個人資料處理方式。".into());}
            let policy=if delete{UserData::Delete}else if keep{UserData::Keep}else{UserData::Ask};
            match installer::uninstall(&target,policy)? {
                Some(pending)=>announce(&format!("已移除善解與系統整合。\n{pending} 個使用中的檔案／目錄將在重新啟動後刪除。"),dialog),
                None=>announce("已取消解除安裝，沒有修改任何資料。",dialog),
            }
        }
        _=>return Err("用法：shanjie-manager [--elevate] install|uninstall [--keep-user-data|--delete-user-data]；另支援 verify-package、apply-profile、profiles。".into()),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        if std::env::args().any(|s| s == "--dialog") {
            unsafe {
                MessageBoxW(
                    None,
                    PCWSTR(wide(&error.to_string()).as_ptr()),
                    w!("善解操作未完成"),
                    MB_OK | MB_ICONERROR,
                );
            }
        }
        std::process::exit(1);
    }
}
