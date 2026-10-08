use shanjie_windows::{
    installer::{self, UserData},
    profile::Result,
    wide,
};
use windows::{
    core::{w, PCWSTR},
    Win32::{Foundation::*, System::Threading::*, UI::Shell::*},
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
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--elevate") {
        return elevate(&args[1..].join(" "));
    }
    let target = installer::install_root();
    match args.first().map(String::as_str) {
        Some("install")=>{let package=std::env::current_exe()?.parent().unwrap().parent().unwrap().to_owned();let pending=installer::install(&package,&target)?;println!("已更新善解並清理舊版；{pending} 個使用中的檔案／目錄將在重新啟動後刪除。請重新開啟使用輸入法的程式。");}
        Some("uninstall")=>{let policy=if args.iter().any(|s|s=="--delete-user-data"){UserData::Delete}else if args.iter().any(|s|s=="--keep-user-data"){UserData::Keep}else{UserData::Ask};match installer::uninstall(&target,policy)? {Some(p)=>println!("已移除善解與系統整合；{p} 個使用中的檔案／目錄將在重新啟動後刪除。"),None=>println!("已取消解除安裝，沒有修改任何資料。")}}
        _=>return Err("用法：shanjie-manager [--elevate] install|uninstall [--keep-user-data|--delete-user-data]".into())
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
