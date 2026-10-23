//! Local controller; never installs, registers, restarts hosts or reads document text.
#![windows_subsystem = "windows"]
use serde_json::{json, Value};
use shanjie_windows::{input_diagnostics as capture, wide};
use std::{
    fs,
    path::{Path, PathBuf},
};
use windows::{
    core::{w, HRESULT, PCWSTR},
    Win32::{
        Foundation::*,
        System::{Diagnostics::ToolHelp::*, Threading::CreateMutexW},
        UI::{Controls::*, HiDpi::*, Shell::ShellExecuteW, WindowsAndMessaging::*},
    },
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SHIFT: i32 = 101;
const EXCEL: i32 = 102;
const STOP: i32 = 103;
const OPEN: i32 = 104;
const STATUS: i32 = 105;
const WORD: i32 = 106;

fn main() {
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    if let Err(error) = run() {
        let args: Vec<String> = std::env::args().collect();
        if args.len() == 1 {
            message(&error.to_string());
        } else if let Some(index) = args.iter().position(|a| a == "--output") {
            if let Some(path) = args.get(index + 1) {
                let _ = fs::write(path, json!({"error":error.to_string()}).to_string());
            }
        }
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        let output = match args[0].as_str() {
            "--start" => {
                let mode = args.get(1).ok_or("請指定 shift、excel 或 word")?;
                start(mode)?
            }
            "--stop" => stop()?,
            "--status" => status()?,
            _ => {
                return Err(
                    "用法：--start shift|excel|word、--stop、--status；不帶參數開啟控制台".into(),
                )
            }
        };
        // GUI-subsystem executables have no console; automation can request an explicit result file.
        if let Some(index) = args.iter().position(|a| a == "--output") {
            fs::write(
                args.get(index + 1).ok_or("缺少結果檔路徑")?,
                serde_json::to_vec_pretty(&output)?,
            )?;
        }
        return Ok(());
    }
    let scope = format!("Local\\ShanjieInputDiagnostics-{:x}", {
        use sha2::{Digest, Sha256};
        Sha256::digest(capture::root().to_string_lossy().as_bytes())
    });
    let mutex = unsafe { CreateMutexW(None, false, PCWSTR(wide(&scope).as_ptr()))? };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = CloseHandle(mutex);
        }
        return Err("診斷控制台已經開啟，請使用原本的視窗。".into());
    }
    let labels = [
        wide("開始：右 Shift 測試"),
        wide("開始：Excel 首鍵測試"),
        wide("開始：加詞浮層測試"),
        wide("停止紀錄"),
        wide("開啟紀錄資料夾"),
        wide("檢查診斷版與載入狀態"),
    ];
    let buttons: Vec<TASKDIALOG_BUTTON> = labels
        .iter()
        .zip([SHIFT, EXCEL, WORD, STOP, OPEN, STATUS])
        .map(|(text, id)| TASKDIALOG_BUTTON {
            nButtonID: id,
            pszButtonText: PCWSTR(text.as_ptr()),
        })
        .collect();
    let config=TASKDIALOGCONFIG{
        cbSize:std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags:TDF_CALLBACK_TIMER|TDF_ALLOW_DIALOG_CANCELLATION|TDF_SIZE_TO_CONTENT,
        dwCommonButtons:TDCBF_CLOSE_BUTTON,pszWindowTitle:w!("善解 0.12.1 診斷控制台（第三版）"),
        pszMainInstruction:w!("選擇測試，再到目標程式操作"),
        pszContent:w!("目前未開始紀錄。\n\n加詞浮層：在 AyuGram／Win+R／Excel 編輯欄位／記事本，反白已提交的中文字，連按兩下左 Ctrl。\n本輪只需加詞測試；右 Shift／Excel 首鍵模式仍保留。\n\n每次最多 5 分鐘；不按 Enter 送出訊息或執行指令，不按「加入」保存詞條。"),
        pszFooter:w!("僅保存測試按鍵名稱、事件、範圍與長度；不保存文字、剪貼簿、私人詞庫或學習資料。關閉控制台會停止紀錄。"),
        cButtons:buttons.len() as u32,pButtons:buttons.as_ptr(),nDefaultButton:STATUS,
        pfCallback:Some(callback),cxWidth:360,..Default::default()};
    let result = unsafe { TaskDialogIndirect(&config, None, None, None) };
    let _ = stop();
    unsafe {
        let _ = CloseHandle(mutex);
    };
    result?;
    Ok(())
}
fn start(mode: &str) -> Result<Value> {
    let snapshot = status()?;
    if std::env::var("SHANJIE_TEST_PROFILE").as_deref() != Ok("1")
        && snapshot["installed_matches_package"] != true
    {
        return Err("目前安裝的 DLL 與這份診斷包不同。請先執行包內「安裝善解.exe」，安裝後重新開啟 AyuGram／Excel，再開始紀錄。此控制台不會自行安裝或重新啟動程式。".into());
    }
    let session = capture::start(mode)?;
    fs::write(
        capture::report(&session).join("loaded-at-start.json"),
        serde_json::to_vec_pretty(&snapshot)?,
    )?;
    Ok(json!({"session":session,"folder":capture::report(&session)}))
}
fn stop() -> Result<Value> {
    let Some(session) = capture::stop()? else {
        return Ok(json!({"stopped":true,"events":0}));
    };
    let mut files = Vec::new();
    let mut events = 0;
    for file in fs::read_dir(capture::report(&session))? {
        let file = file?;
        let path = file.path();
        if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
            let bytes = fs::read(&path)?;
            let count = bytes
                .split(|b| *b == b'\n')
                .filter(|line| !line.is_empty())
                .count();
            events += count;
            files.push(json!({"file":file.file_name().to_string_lossy(),"events":count,"bytes":bytes.len()}));
        }
    }
    let summary = json!({"stopped":true,"session":session,"events":events,"files":files,
        "loaded_at_stop":status()?,"folder":capture::report(&session),
        "notice":if events==0{"沒有事件；請檢查目標程式是否載入診斷 DLL，以及測試模式是否正確。"}else{"請回報這次測試是否出現問題。"}});
    fs::write(
        capture::report(&session).join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(summary)
}
fn hash(path: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    fs::read(path)
        .ok()
        .map(|b| format!("{:x}", Sha256::digest(b)))
}
fn status() -> Result<Value> {
    let package = std::env::current_exe()?
        .parent()
        .ok_or("控制台路徑無效")?
        .to_owned();
    let expected: [Option<String>; 2] = [
        hash(&package.join("x64/Shanjie.dll")),
        hash(&package.join("x86/Shanjie.dll")),
    ];
    let installed = shanjie_windows::installer::current_binaries()
        .ok()
        .flatten();
    let installed_hashes = installed.as_ref().map(|base| {
        [
            hash(&base.join("x64/Shanjie.dll")),
            hash(&base.join("x86/Shanjie.dll")),
        ]
    });
    let matches =
        expected.iter().all(Option::is_some) && installed_hashes.as_ref() == Some(&expected);
    Ok(
        json!({"build":capture::BUILD,"installed_matches_package":matches,"installed":installed,
        "expected_sha256":expected,"installed_sha256":installed_hashes,"processes":processes(&expected),
        "active":capture::load_at(&capture::root()).filter(|s|s.valid(capture::now_ms()))}),
    )
}
fn processes(expected: &[Option<String>; 2]) -> Vec<Value> {
    let mut rows = Vec::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return rows;
        };
        let mut p = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut next = Process32FirstW(snapshot, &mut p);
        while next.is_ok() {
            let name = String::from_utf16_lossy(
                &p.szExeFile[..p
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(p.szExeFile.len())],
            );
            if capture::allowed("shift", &name) || capture::allowed("excel", &name) {
                let mut modules = Vec::new();
                let mut error = None;
                match CreateToolhelp32Snapshot(
                    TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32,
                    p.th32ProcessID,
                ) {
                    Ok(handle) => {
                        let mut m = MODULEENTRY32W {
                            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
                            ..Default::default()
                        };
                        let mut item = Module32FirstW(handle, &mut m);
                        while item.is_ok() {
                            let module = String::from_utf16_lossy(
                                &m.szModule[..m
                                    .szModule
                                    .iter()
                                    .position(|c| *c == 0)
                                    .unwrap_or(m.szModule.len())],
                            );
                            if module.eq_ignore_ascii_case("Shanjie.dll") {
                                let path = PathBuf::from(String::from_utf16_lossy(
                                    &m.szExePath[..m
                                        .szExePath
                                        .iter()
                                        .position(|c| *c == 0)
                                        .unwrap_or(m.szExePath.len())],
                                ));
                                let digest = hash(&path);
                                let diagnostic = digest.is_some() && expected.contains(&digest);
                                modules.push(
                                    json!({"path":path,"sha256":digest,"diagnostic":diagnostic}),
                                );
                            }
                            item = Module32NextW(handle, &mut m);
                        }
                        let _ = CloseHandle(handle);
                    }
                    Err(e) => error = Some(e.code().0),
                }
                rows.push(json!({"process":name,"pid":p.th32ProcessID,"modules":modules,"inspection_error":error}));
            }
            next = Process32NextW(snapshot, &mut p);
        }
        let _ = CloseHandle(snapshot);
    }
    rows
}
unsafe extern "system" fn callback(
    hwnd: HWND,
    notification: TASKDIALOG_NOTIFICATIONS,
    button: WPARAM,
    _: LPARAM,
    _: isize,
) -> HRESULT {
    if notification == TDN_BUTTON_CLICKED {
        let result=match button.0 as i32 {
            SHIFT=>start("shift").map(|v|format!("正在記錄右 Shift 測試（最多 5 分鐘）。\n\n請到 AyuGram／Win+R／記事本單按右 Shift，完成後回來按「停止紀錄」。\n\n紀錄位置：{}",v["folder"].as_str().unwrap_or(""))),
            EXCEL=>start("excel").map(|v|format!("正在記錄 Excel 首鍵測試（最多 5 分鐘）。\n\n在空白活頁簿：方向鍵換格後打 123；換另一格，按 F2 後打 123。完成後回來停止。\n\n紀錄位置：{}",v["folder"].as_str().unwrap_or(""))),
            WORD=>start("word").map(|v|format!("正在記錄加詞浮層測試（最多 5 分鐘）。\n\n反白已提交的中文字，連按兩下左 Ctrl。測 AyuGram／Win+R／Excel 編輯欄位及記事本對照；不用重測 Shift 或 123，不按「加入」。完成後回來停止。\n\n紀錄位置：{}",v["folder"].as_str().unwrap_or(""))),
            STOP=>stop().map(|v|format!("已停止紀錄。事件數：{}\n\n{}\n\n位置：{}",v["events"],v["notice"].as_str().unwrap_or("尚未開始測試。"),v["folder"].as_str().unwrap_or(""))),
            OPEN=>{let path=capture::reports();fs::create_dir_all(&path).map_err(Into::into).map(|_|{unsafe{ShellExecuteW(None,w!("open"),PCWSTR(wide(&path.to_string_lossy()).as_ptr()),None,None,SW_SHOWNORMAL);}"已開啟紀錄資料夾。".to_owned()})},
            STATUS=>status().map(|v|{
                let mut text=if v["installed_matches_package"]==true{"診斷版已安裝。".to_owned()}else{"尚未安裝這份診斷版；請先執行包內「安裝善解.exe」。".to_owned()};
                for p in v["processes"].as_array().into_iter().flatten(){
                    let label=if p["inspection_error"].is_number(){"無法檢查（可能以管理員權限開啟）"}
                        else if p["modules"].as_array().is_some_and(|m|m.iter().any(|m|m["diagnostic"]==true)){"已載入診斷 DLL"}
                        else if p["modules"].as_array().is_some_and(|m|!m.is_empty()){"仍載入舊 DLL，請重新開啟／登出登入"}
                        else{"尚未載入善解，請點進輸入欄位並選善解"};
                    text.push_str(&format!("\n{}：{label}",p["process"].as_str().unwrap_or("")));
                }text}),
            _=>return S_OK,
        };
        let content = wide(&result.unwrap_or_else(|e| format!("操作未完成：{e}")));
        unsafe {
            SendMessageW(
                hwnd,
                TDM_SET_ELEMENT_TEXT.0 as u32,
                Some(WPARAM(TDE_CONTENT.0 as usize)),
                Some(LPARAM(content.as_ptr() as isize)),
            );
        }
        return S_FALSE;
    }
    if notification == TDN_TIMER
        && capture::load_at(&capture::root())
            .is_some_and(|s| s.active && !s.valid(capture::now_ms()))
    {
        let _ = stop();
        let text =
            wide("已達 5 分鐘或控制檔已失效，紀錄自動停止。\n請按「開啟紀錄資料夾」查看結果。");
        unsafe {
            SendMessageW(
                hwnd,
                TDM_SET_ELEMENT_TEXT.0 as u32,
                Some(WPARAM(TDE_CONTENT.0 as usize)),
                Some(LPARAM(text.as_ptr() as isize)),
            );
        }
    }
    S_OK
}
fn message(text: &str) {
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(wide(text).as_ptr()),
            w!("善解輸入診斷"),
            MB_OK | MB_ICONERROR,
        );
    }
}
