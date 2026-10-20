use crate::{profile::Result, wide};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::windows::{fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    process::Command,
};
use windows::{
    core::{w, BOOL, PCWSTR},
    Win32::{
        Foundation::*,
        System::{Com::*, Registry::*},
        UI::{Controls::*, TextServices::*, WindowsAndMessaging::*},
    },
};
mod journal;
mod language_list;
mod logon_task;
mod permissions;
pub mod progress;
mod transaction;
pub const SERVICE: &str = "{8912696E-8A31-4DF3-A7BC-0261F3B6A147}";
pub fn install_root() -> PathBuf {
    PathBuf::from(std::env::var_os("ProgramFiles").expect("Program Files")).join("Shanjie")
}
fn safe_tree(root: &Path) -> Result<()> {
    if root.exists() && fs::symlink_metadata(root)?.file_attributes() & 0x400 != 0 {
        return Err(format!("目錄是連結或 junction，未進行清理：{}", root.display()).into());
    }
    Ok(())
}
fn children(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    safe_tree(root)?;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            children(&path, out)?;
        }
        out.push(path);
    }
    Ok(())
}
/// Preserve retired payloads beside their old location. Never follow reparse points.
pub fn remove_tree(root: &Path) -> Result<usize> {
    if !root.exists() {
        return Ok(0);
    }
    safe_tree(root)?;
    let archive = root
        .parent()
        .ok_or("不可清理根目錄。")?
        .join(".deleted_files");
    safe_tree(&archive)?;
    fs::create_dir_all(&archive)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let destination = archive.join(format!(
        "{stamp:x}-{}",
        root.file_name().ok_or("缺少檔名。")?.to_string_lossy()
    ));
    match fs::rename(root, &destination) {
        Ok(()) => Ok(0),
        // Keep a busy payload in place; no reboot-time permanent deletion.
        Err(e) if [Some(5), Some(32), Some(33)].contains(&e.raw_os_error()) => Ok(1),
        Err(e) => Err(e.into()),
    }
}
fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    children(root, &mut out)?;
    Ok(out.into_iter().filter(|p| p.is_file()).collect())
}
fn digest(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut block = [0u8; 65536];
    loop {
        let read = file.read(&mut block)?;
        if read == 0 {
            break;
        }
        hash.update(&block[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn verified_package(package: &Path) -> Result<Vec<PathBuf>> {
    let mut listed = Vec::new();
    for line in fs::read_to_string(package.join("SHA256SUMS.txt"))?.lines() {
        let (hash, name) = line.split_once("  ").ok_or("校驗清單格式有誤。")?;
        let relative = Path::new(name);
        if relative.is_absolute()
            || relative
                .components()
                .any(|p| !matches!(p, std::path::Component::Normal(_)))
        {
            return Err("安裝包含有不合法的路徑。".into());
        }
        let source = package.join(relative);
        if digest(&source)? != hash {
            return Err(format!("安裝包檔案校驗失敗：{name}").into());
        }
        listed.push(relative.to_owned());
    }
    for required in [
        "x64/Shanjie.dll",
        "x86/Shanjie.dll",
        "ui/Shanjie.UI.exe",
        "data/models/model-v6/bigram.winmap",
        "data/models/model-v6/classes.sjc",
        // Optional to the core (a missing file silently turns its feature off), so the package must
        // carry them, as upstream's check-app requires digits.tsv.
        "data/digits.tsv",
        "data/english-words.txt",
        "data/packs/acg-add.tsv",
        "tools/shanjie-manager.exe",
        "tools/shanjie-core.exe",
    ] {
        if !listed.contains(&PathBuf::from(required)) {
            return Err(format!("安裝包缺少 {required}").into());
        }
    }
    if listed
        .iter()
        .any(|p| p.starts_with("data/models") && !p.starts_with("data/models/model-v6/"))
    {
        return Err("安裝包只內建 model-v6＋classes-v5；其他配套由使用者另行取得。".into());
    }
    Ok(listed)
}
fn regsvr(dll: &Path, arch: &str, remove: bool) -> Result<()> {
    regsvr_selected(dll, arch, remove, 3)
}
fn regsvr_selected(dll: &Path, arch: &str, remove: bool, keyboards: u32) -> Result<()> {
    let system = if arch == "x64" {
        "System32"
    } else {
        "SysWOW64"
    };
    let executable = PathBuf::from(std::env::var_os("WINDIR").unwrap())
        .join(system)
        .join("regsvr32.exe");
    let mut command = Command::new(executable);
    command.env("SHANJIE_INSTALL_KEYBOARDS", keyboards.to_string());
    command.creation_flags(0x08000000).arg("/s");
    if remove {
        command.arg("/u");
    }
    let status = command.arg(dll).status()?;
    if !status.success() {
        return Err(format!("TSF {arch} 註冊操作失敗：{status}").into());
    }
    Ok(())
}
fn registry_dll(view: REG_SAM_FLAGS) -> Result<Option<PathBuf>> {
    unsafe {
        let key = wide(&format!(
            "SOFTWARE\\Classes\\CLSID\\{SERVICE}\\InprocServer32"
        ));
        let mut handle = HKEY::default();
        let result = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key.as_ptr()),
            Some(0),
            KEY_READ | view,
            &mut handle,
        );
        if result == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        result.ok()?;
        let mut data = [0u16; 32768];
        let mut size = (data.len() * 2) as u32;
        let result = RegQueryValueExW(
            handle,
            None,
            None,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut size),
        );
        let _ = RegCloseKey(handle);
        result.ok()?;
        Ok(Some(PathBuf::from(String::from_utf16(
            &data[..size as usize / 2 - 1],
        )?)))
    }
}
pub fn profiles(remove: bool) -> Result<()> {
    reject_live_test_mutation()?;
    let mask = if remove {
        3
    } else {
        crate::installation::load(&install_root())?
            .and_then(|state| state.keyboards)
            .ok_or("請先透過安裝工具選擇鍵盤配置。")?
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let folder =
        std::env::temp_dir().join(format!("Shanjie-language-{}-{stamp:x}", std::process::id()));
    let before = language_list::read(&folder)?;
    profiles_selected(mask, remove)?;
    let desired = language_list::plan(&before, if remove { 0 } else { mask });
    match language_list::apply(&folder, &before, &desired) {
        Ok(_) => Ok(()),
        Err(error) => {
            let restored = language_list::restore(&folder, &before, &desired);
            Err(format!(
                "{error}；語言清單回復：{}；紀錄：{}",
                restored.map_or_else(|e| e.to_string(), |_| "完成".into()),
                folder.display()
            )
            .into())
        }
    }
}
fn profiles_selected(mask: u32, remove: bool) -> Result<()> {
    if mask == 0 {
        return Ok(());
    }
    unsafe {
        use windows::{
            core::PCSTR,
            Win32::System::LibraryLoader::{
                GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
            },
        };
        let module = LoadLibraryExW(w!("input.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)?;
        let address = GetProcAddress(module, PCSTR(c"InstallLayoutOrTip".as_ptr().cast()))
            .ok_or("Windows 缺少 InstallLayoutOrTip。")?;
        let install: unsafe extern "system" fn(PCWSTR, u32) -> BOOL = std::mem::transmute(address);
        let result = install(
            PCWSTR(wide(&crate::installation::tips(mask)).as_ptr()),
            remove as u32,
        );
        FreeLibrary(module)?;
        if !result.as_bool() {
            return Err("Windows 無法更新善解輸入法清單。".into());
        }
    }
    Ok(())
}
const APP_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Shanjie";
const RUN_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
/// Versions before the logon task started the core from this Run value; removing it keeps one launcher.
unsafe fn remove_run_entry() -> Result<()> {
    let mut key = HKEY::default();
    let status = RegOpenKeyExW(
        HKEY_LOCAL_MACHINE,
        PCWSTR(wide(RUN_KEY).as_ptr()),
        Some(0),
        KEY_SET_VALUE | KEY_WOW64_64KEY,
        &mut key,
    );
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    status.ok()?;
    let status = RegDeleteValueW(key, w!("ShanjieCore"));
    let _ = RegCloseKey(key);
    if status == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        status.ok().map_err(Into::into)
    }
}
unsafe fn app_entry(target: &Path, binaries: &Path) -> Result<()> {
    let mut key = HKEY::default();
    RegCreateKeyExW(
        HKEY_LOCAL_MACHINE,
        PCWSTR(wide(APP_KEY).as_ptr()),
        Some(0),
        None,
        REG_OPTION_NON_VOLATILE,
        KEY_ALL_ACCESS | KEY_WOW64_64KEY,
        None,
        &mut key,
        None,
    )
    .ok()?;
    let result = (|| -> Result<()> {
        for (name, value) in [
            ("DisplayName", "善解 Windows".to_owned()),
            ("DisplayVersion", env!("CARGO_PKG_VERSION").to_owned()),
            ("InstallLocation", target.to_string_lossy().into_owned()),
            (
                "DisplayIcon",
                binaries
                    .join("ui/Assets/AppIcon.ico")
                    .to_string_lossy()
                    .into_owned(),
            ),
            (
                "UninstallString",
                format!(
                    "\"{}\" --elevate uninstall",
                    binaries.join("tools/shanjie-manager.exe").display()
                ),
            ),
        ] {
            let value = wide(&value);
            RegSetValueExW(
                key,
                PCWSTR(wide(name).as_ptr()),
                Some(0),
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    value.as_ptr().cast(),
                    value.len() * 2,
                )),
            )
            .ok()?;
        }
        for name in ["NoModify", "NoRepair"] {
            RegSetValueExW(
                key,
                PCWSTR(wide(name).as_ptr()),
                Some(0),
                REG_DWORD,
                Some(&1u32.to_le_bytes()),
            )
            .ok()?;
        }
        Ok(())
    })();
    RegCloseKey(key).ok()?;
    result
}
pub fn choose_keyboard(target: &Path, explicit: Option<u32>, choose: bool) -> Result<Option<u32>> {
    if let Some(mask) = explicit {
        if !matches!(mask, 1 | 2) {
            return Err("請選擇 standard 或 eten。".into());
        }
        return Ok(Some(mask));
    }
    if !choose {
        if let Some(mask) = crate::installation::load(target)?
            .and_then(|state| state.keyboards)
            .filter(|mask| matches!(mask, 1 | 2))
        {
            return Ok(Some(mask));
        }
    }
    let standard = crate::installation::registered(KEY_WOW64_64KEY)?;
    let eten = crate::installation::registered(KEY_WOW64_32KEY)?;
    let user = crate::registry_snapshot::capture(transaction::locations()[6].clone())?;
    let listed = crate::installation::listed(&user);
    let current = if standard != eten {
        "目前鍵盤註冊不一致，這次會依你的選擇修復。"
    } else if listed == 3 {
        "目前清單有標準注音與倚天。"
    } else if listed == 1 {
        "目前清單有標準注音。"
    } else if listed == 2 {
        "目前清單有倚天。"
    } else {
        "請選擇要安裝的鍵盤配置。"
    };
    let content = wide(&format!(
        "{current}\n只安裝選定的配置，個人詞庫與輸入偏好會保留。"
    ));
    let standard_label = wide("標準注音\n大千式排列，一般注音鍵盤");
    let eten_label = wide("倚天注音\n倚天式排列");
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: 101,
            pszButtonText: PCWSTR(standard_label.as_ptr()),
        },
        TASKDIALOG_BUTTON {
            nButtonID: 102,
            pszButtonText: PCWSTR(eten_label.as_ptr()),
        },
    ];
    let config = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_USE_COMMAND_LINKS | TDF_ALLOW_DIALOG_CANCELLATION,
        dwCommonButtons: TDCBF_CANCEL_BUTTON,
        pszWindowTitle: w!("安裝善解"),
        pszMainInstruction: w!("選擇要安裝的鍵盤配置"),
        pszContent: PCWSTR(content.as_ptr()),
        cButtons: buttons.len() as u32,
        pButtons: buttons.as_ptr(),
        nDefaultButton: 101,
        ..Default::default()
    };
    let mut pressed = 0;
    unsafe { TaskDialogIndirect(&config, Some(&mut pressed), None, None)? };
    Ok(match pressed {
        101 => Some(1),
        102 => Some(2),
        _ => None,
    })
}
pub fn install(
    package: &Path,
    target: &Path,
    explicit: Option<u32>,
    choose: bool,
) -> Result<Option<usize>> {
    reject_live_test_mutation()?;
    if !unsafe { windows::Win32::UI::Shell::IsUserAnAdmin().as_bool() } {
        return Err("請以系統管理員身分安裝善解。".into());
    }
    let _owner = install_owner()?;
    progress::step("Verify", "started", "");
    let entries = verified_package(package)?;
    progress::step("Verify", "succeeded", "");
    let Some(keyboards) = choose_keyboard(target, explicit, choose)? else {
        return Ok(None);
    };
    safe_tree(target)?;
    safe_tree(&target.join("versions"))?;
    // Reject external registrations before staging any version.
    for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
        if registry_dll(view)?
            .as_ref()
            .is_some_and(|path| !path.starts_with(target))
        {
            return Err("既有善解 DLL 路徑超出安裝目錄。".into());
        }
    }
    fs::create_dir_all(target)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let version = format!(
        "{}-{stamp:x}",
        &digest(&package.join("x64/Shanjie.dll"))?[..16]
    );
    let binaries = target.join("versions").join(&version);
    fs::create_dir_all(&binaries)?;
    progress::step("Copy", "started", "");
    let staging = (|| -> Result<()> {
        for (done, relative) in entries.iter().enumerate() {
            let destination = binaries.join(relative);
            fs::create_dir_all(destination.parent().unwrap())?;
            fs::copy(package.join(relative), destination)?;
            if done % 25 == 0 {
                progress::count("Copy", done, entries.len());
            }
        }
        fs::copy(
            package.join("SHA256SUMS.txt"),
            binaries.join("SHA256SUMS.txt"),
        )?;
        Ok(())
    })();
    if let Err(error) = staging {
        let _ = remove_tree(&binaries);
        return Err(error);
    }
    progress::step("Copy", "succeeded", "");
    let backup = target
        .join(".installer-recovery")
        .join(format!("{stamp:x}"));
    safe_tree(&target.join(".installer-recovery"))?;
    let result = transaction::execute(&mut transaction::Native {
        target,
        package,
        binaries: &binaries,
        version: &version,
        keyboards,
        backup: &backup,
        previous: None,
    });
    if let Err(error) = result {
        if transaction::rolled_back(&backup) {
            let _ = remove_tree(&binaries);
        }
        return Err(error);
    }
    // Only a fully validated transaction may retire an old version.
    progress::step("Cleanup", "started", "");
    let mut pending = 0;
    for entry in fs::read_dir(target.join("versions"))? {
        let entry = entry?;
        if entry.file_name() != version.as_str() && entry.file_name() != ".deleted_files" {
            pending += retire(&entry.path())?;
        }
    }
    for entry in fs::read_dir(target)? {
        let entry = entry?;
        if ![
            "versions",
            "installed.json",
            "ui-path.txt",
            "manager-path.txt",
            "settings.cmd",
            "uninstall.cmd",
            ".deleted_files",
            ".installer-recovery",
        ]
        .contains(&entry.file_name().to_string_lossy().as_ref())
        {
            pending += retire(&entry.path())?;
        }
    }
    pending += purge_superseded(target, &backup);
    progress::step("Cleanup", "succeeded", "");
    Ok(Some(pending))
}
/// Retire a superseded payload; a busy one is deleted by Windows at the next restart.
fn retire(root: &Path) -> Result<usize> {
    let busy = remove_tree(root)?;
    if busy > 0 {
        let _ = delete_at_reboot(root);
    }
    Ok(busy)
}
/// Children first, then the root: Windows only removes a directory once it is empty.
fn reboot_delete_order(root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if root.is_dir() {
        children(root, &mut paths)?;
    }
    paths.push(root.to_owned());
    Ok(paths)
}
fn delete_at_reboot(root: &Path) -> Result<()> {
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};
    for path in reboot_delete_order(root)? {
        unsafe {
            MoveFileExW(
                PCWSTR(crate::wide(&path.to_string_lossy()).as_ptr()),
                PCWSTR::null(),
                MOVEFILE_DELAY_UNTIL_REBOOT,
            )?;
        }
    }
    Ok(())
}
/// After a fully validated update, permanently delete retired payloads and old recovery
/// checkpoints so only the newest install remains. Best effort: busy items are left in
/// place and counted so a later update retries them. Never follows reparse points.
fn purge_superseded(target: &Path, current_backup: &Path) -> usize {
    let mut busy = 0;
    for archive in [
        target.join("versions").join(".deleted_files"),
        target.join(".deleted_files"),
    ] {
        busy += purge_children(&archive, None);
    }
    busy + purge_children(&target.join(".installer-recovery"), Some(current_backup))
}
fn purge_children(root: &Path, keep: Option<&Path>) -> usize {
    if !root.exists() || safe_tree(root).is_err() {
        return 0;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return 1;
    };
    let mut busy = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if keep == Some(path.as_path()) {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            busy += 1;
            continue;
        };
        // remove_dir_all removes a junction itself without descending into its target.
        let removed = if kind.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if removed.is_err() {
            let _ = delete_at_reboot(&path);
            busy += 1;
        }
    }
    busy
}
fn install_owner() -> Result<InstallOwner> {
    use windows::Win32::System::Threading::*;
    let handle = unsafe { CreateMutexW(None, false, w!("Global\\ShanjieWindowsInstaller"))? };
    let status = unsafe { WaitForSingleObject(handle, 0) };
    if status != WAIT_OBJECT_0 && status != WAIT_ABANDONED {
        unsafe {
            CloseHandle(handle)?;
        }
        return Err("另一個善解安裝或移除作業正在進行，請稍後重試。".into());
    }
    Ok(InstallOwner(handle))
}
struct InstallOwner(HANDLE);
impl Drop for InstallOwner {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::System::Threading::ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}
pub enum UserData {
    Ask,
    Keep,
    Delete,
}
fn choose_data(policy: UserData) -> Option<bool> {
    match policy {
        UserData::Keep => Some(false),
        UserData::Delete => Some(true),
        UserData::Ask => unsafe {
            match MessageBoxW(None,w!("要一併移除個人詞庫、選字學習與設定嗎？\n\n是：移除程式與全部個人資料，檔案保留至 .deleted_files。\n否：移除程式與設定，保留詞庫和學習供重新安裝使用。\n取消：不進行解除安裝。"),w!("解除安裝善解"),MB_YESNOCANCEL|MB_ICONQUESTION|MB_DEFBUTTON3){IDYES=>Some(true),IDNO=>Some(false),_=>None}
        },
    }
}
fn delete_key(root: HKEY, name: &str, view: REG_SAM_FLAGS) -> Result<()> {
    unsafe {
        let mut h = HKEY::default();
        let status = RegOpenKeyExW(
            root,
            PCWSTR(wide(name).as_ptr()),
            Some(0),
            KEY_ALL_ACCESS | view,
            &mut h,
        );
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        status.ok()?;
        let result = RegDeleteTreeW(h, None);
        let _ = RegCloseKey(h);
        result.ok()?;
        let status = RegDeleteKeyExW(root, PCWSTR(wide(name).as_ptr()), view.0, None);
        if status != ERROR_FILE_NOT_FOUND {
            status.ok()?;
        }
        Ok(())
    }
}
unsafe fn remove_user_profiles() -> Result<()> {
    use windows::core::PWSTR;
    let mut root = HKEY::default();
    let status = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        w!("Control Panel\\International\\User Profile"),
        Some(0),
        KEY_READ,
        &mut root,
    );
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    status.ok()?;
    let result = (|| -> Result<()> {
        let mut index = 0;
        loop {
            let mut language = [0u16; 256];
            let mut length = language.len() as u32;
            let status = RegEnumKeyExW(
                root,
                index,
                Some(PWSTR(language.as_mut_ptr())),
                &mut length,
                None,
                None,
                None,
                None,
            );
            if status == ERROR_NO_MORE_ITEMS {
                break;
            }
            status.ok()?;
            index += 1;
            let mut key = HKEY::default();
            RegOpenKeyExW(
                root,
                PCWSTR(language.as_ptr()),
                Some(0),
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                &mut key,
            )
            .ok()?;
            let result = (|| -> Result<()> {
                let mut index = 0;
                loop {
                    let mut name = [0u16; 16384];
                    let mut length = name.len() as u32;
                    let status = RegEnumValueW(
                        key,
                        index,
                        Some(PWSTR(name.as_mut_ptr())),
                        &mut length,
                        None,
                        None,
                        None,
                        None,
                    );
                    if status == ERROR_NO_MORE_ITEMS {
                        break;
                    }
                    status.ok()?;
                    if String::from_utf16(&name[..length as usize])?
                        .to_ascii_uppercase()
                        .contains(&format!(":{SERVICE}"))
                    {
                        RegDeleteValueW(key, PCWSTR(name.as_ptr())).ok()?;
                    } else {
                        index += 1;
                    }
                }
                Ok(())
            })();
            RegCloseKey(key).ok()?;
            result?;
        }
        Ok(())
    })();
    RegCloseKey(root).ok()?;
    result
}
/// Version folder that TSF currently loads, from the 64-bit registration.
pub fn current_binaries() -> Result<Option<PathBuf>> {
    Ok(registry_dll(KEY_WOW64_64KEY)?
        .and_then(|dll| Some(dll.parent()?.parent()?.to_owned())))
}
/// Shell hosts that never exit on their own and restart themselves when ended.
const SHELL_HOSTS: [&str; 4] = [
    "SearchHost.exe",
    "StartMenuExperienceHost.exe",
    "TextInputHost.exe",
    "ShellExperienceHost.exe",
];
/// Ends this user's shell hosts that still hold a retired Shanjie DLL. Runs unelevated.
pub fn restart_stale_hosts(current: &Path) -> Result<Vec<String>> {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    let session = |pid: u32| {
        let mut id = 0;
        unsafe { windows::Win32::System::RemoteDesktop::ProcessIdToSessionId(pid, &mut id) }
            .ok()
            .map(|_| id)
    };
    let mine = session(std::process::id());
    let mut ended = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(0);
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            let pid = entry.th32ProcessID;
            if SHELL_HOSTS.iter().any(|h| h.eq_ignore_ascii_case(&name))
                && session(pid) == mine
                && holds_retired(pid, current)
            {
                if let Ok(process) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                    if TerminateProcess(process, 0).is_ok() {
                        ended.push(name);
                    }
                    let _ = CloseHandle(process);
                }
            }
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    Ok(ended)
}
unsafe fn holds_retired(pid: u32, current: &Path) -> bool {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)
    else {
        return false;
    };
    let mut module = MODULEENTRY32W {
        dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };
    let mut retired = false;
    let mut more = Module32FirstW(snapshot, &mut module).is_ok();
    while more {
        let len = module.szExePath.iter().position(|&c| c == 0).unwrap_or(0);
        let path = PathBuf::from(String::from_utf16_lossy(&module.szExePath[..len]));
        if path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("Shanjie.dll"))
            && !path.starts_with(current)
        {
            retired = true;
            break;
        }
        more = Module32NextW(snapshot, &mut module).is_ok();
    }
    let _ = CloseHandle(snapshot);
    retired
}
pub fn uninstall(target: &Path, policy: UserData) -> Result<Option<usize>> {
    reject_live_test_mutation()?;
    if !unsafe { windows::Win32::UI::Shell::IsUserAnAdmin().as_bool() } {
        return Err("請以系統管理員身分解除安裝善解。".into());
    }
    let _owner = install_owner()?;
    let Some(delete) = choose_data(policy) else {
        return Ok(None);
    };
    progress::step("Stop", "started", "");
    unsafe {
        remove_run_entry()?;
    }
    logon_task::remove()?;
    if let Ok(entries) = fs::read_dir(target.join("versions")) {
        for entry in entries.flatten() {
            if entry.file_name() == ".deleted_files" {
                continue;
            }
            let core = entry.path().join("tools/shanjie-core.exe");
            if core.exists() {
                let _ = Command::new(core)
                    .arg("--stop")
                    .creation_flags(0x08000000)
                    .status();
            }
        }
    }
    progress::step("Stop", "succeeded", "");
    progress::step("Unregister", "started", "");
    safe_tree(target)?;
    profiles(true)?;
    unsafe {
        remove_user_profiles()?;
    }
    for (arch, view) in [("x64", KEY_WOW64_64KEY), ("x86", KEY_WOW64_32KEY)] {
        if let Some(dll) = registry_dll(view)? {
            if !dll.starts_with(target) {
                return Err("註冊 DLL 路徑超出善解安裝目錄。".into());
            }
            if dll.exists() {
                regsvr(&dll, arch, true)?;
            }
        }
        delete_key(
            HKEY_LOCAL_MACHINE,
            &format!("SOFTWARE\\Classes\\CLSID\\{SERVICE}"),
            view,
        )?;
        delete_key(HKEY_LOCAL_MACHINE, APP_KEY, view)?;
        delete_key(
            HKEY_LOCAL_MACHINE,
            &format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{SERVICE}"),
            view,
        )?;
    }
    // Native TSF cleanup also works when the registered DLL has already disappeared.
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let p: ITfInputProcessorProfiles =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
        let id = windows::core::GUID::from_u128(0x8912696e8a314df3a7bc0261f3b6a147);
        let _ = p.Unregister(&id);
        let categories: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        for category in crate::registration::CATEGORIES {
            let _ = categories.UnregisterCategory(&id, &category, &id);
        }
        if initialized.is_ok() {
            CoUninitialize();
        }
    }
    delete_key(
        HKEY_CURRENT_USER,
        &format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{SERVICE}"),
        REG_SAM_FLAGS(0),
    )?;
    progress::step("Unregister", "succeeded", "");
    progress::step("Remove", "started", "");
    let mut pending = remove_tree(target)?;
    progress::step("Remove", "succeeded", "");
    progress::step("Data", "started", "");
    let personal = crate::profile::folder();
    safe_tree(&personal)?;
    if delete {
        pending += remove_tree(&personal)?;
    } else if personal.exists() {
        for file in files(&personal)? {
            let name = file.file_name().unwrap().to_string_lossy();
            if name.starts_with("profile.ini") || name.starts_with("profile.") {
                pending += remove_tree(&file)?;
            }
        }
    }
    progress::step("Data", "succeeded", "");
    Ok(Some(pending))
}
fn reject_live_test_mutation() -> Result<()> {
    if std::env::var("SHANJIE_TEST_PROFILE").as_deref() == Ok("1") {
        return Err("隔離驗證不可變更真實輸入法安裝或語言清單。".into());
    }
    Ok(())
}
pub fn record_manager_failure(message: &str) -> Result<PathBuf> {
    let root = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("個人紀錄位置無法辨識。")?)
        .join("Shanjie/installer-logs");
    safe_tree(&root)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let folder = root.join(format!("{}-{stamp:x}", std::process::id()));
    fs::create_dir_all(&folder)?;
    journal::error(&folder, "Manager", message)?;
    Ok(folder)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_removes_retired_nested_payload() {
        let root = std::env::temp_dir().join(format!("shanjie-cleanup-{}", std::process::id()));
        fs::create_dir_all(root.join("old/data/models/model-v1")).unwrap();
        fs::write(root.join("old/data/models/model-v1/model"), b"old").unwrap();
        assert_eq!(remove_tree(&root).unwrap(), 0);
        assert!(!root.exists());
    }
    #[test]
    fn update_purges_retired_versions_and_old_recovery_but_keeps_newest() {
        let target = std::env::temp_dir().join(format!("shanjie-purge-{}", std::process::id()));
        let _ = fs::remove_dir_all(&target);
        let versions = target.join("versions");
        for old in ["versions/.deleted_files/a/tools", ".deleted_files/b"] {
            fs::create_dir_all(target.join(old)).unwrap();
            fs::write(target.join(old).join("f"), b"x").unwrap();
        }
        fs::create_dir_all(versions.join("current")).unwrap();
        for id in ["1", "2", "3"] {
            fs::create_dir_all(target.join(".installer-recovery").join(id)).unwrap();
            fs::write(target.join(".installer-recovery").join(id).join("error.txt"), b"x").unwrap();
        }
        let keep = target.join(".installer-recovery/3");
        assert_eq!(purge_superseded(&target, &keep), 0);
        assert_eq!(fs::read_dir(versions.join(".deleted_files")).unwrap().count(), 0);
        assert_eq!(fs::read_dir(target.join(".deleted_files")).unwrap().count(), 0);
        assert!(versions.join("current").is_dir());
        let left: Vec<_> = fs::read_dir(target.join(".installer-recovery"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, vec![std::ffi::OsString::from("3")]);
        fs::remove_dir_all(&target).unwrap();
    }
    #[test]
    fn reboot_delete_lists_children_before_their_directory() {
        let root = std::env::temp_dir().join(format!("shanjie-reboot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("a/b/f"), b"x").unwrap();
        let order = reboot_delete_order(&root).unwrap();
        let at = |p: PathBuf| order.iter().position(|x| *x == p).unwrap();
        assert!(at(root.join("a/b/f")) < at(root.join("a/b")));
        assert!(at(root.join("a/b")) < at(root.join("a")));
        assert_eq!(order.last().unwrap(), &root);
        fs::remove_dir_all(&root).unwrap();
    }
}
