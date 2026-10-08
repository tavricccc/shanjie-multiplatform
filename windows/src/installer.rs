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
        Storage::FileSystem::*,
        System::{Com::*, Registry::*},
        UI::{TextServices::*, WindowsAndMessaging::*},
    },
};
pub const SERVICE: &str = "{8912696E-8A31-4DF3-A7BC-0261F3B6A147}";
const TIPS:&str="0x0404:{8912696E-8A31-4DF3-A7BC-0261F3B6A147}{AC59A613-F813-4DAD-BA97-D221A87A6ACE};0x0404:{8912696E-8A31-4DF3-A7BC-0261F3B6A147}{338145B4-047A-4564-A0DE-223DC22C1AE1}";
pub fn install_root() -> PathBuf {
    PathBuf::from(
        std::env::var_os("ProgramW6432")
            .or_else(|| std::env::var_os("ProgramFiles"))
            .expect("Program Files"),
    )
    .join("Shanjie")
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
/// Files precede their parent directories in the reboot queue. Never follow reparse points.
pub fn remove_tree(root: &Path) -> Result<usize> {
    if !root.exists() {
        return Ok(0);
    }
    safe_tree(root)?;
    let mut paths = Vec::new();
    if root.is_dir() {
        children(root, &mut paths)?;
    }
    paths.push(root.to_owned());
    let mut pending = 0;
    for path in paths {
        if path != root && !path.starts_with(root) {
            return Err("清理路徑超出指定目錄。".into());
        }
        let result = if path.is_dir() {
            fs::remove_dir(&path)
        } else {
            fs::remove_file(&path)
        };
        if let Err(error) = result {
            if ![Some(5), Some(32), Some(33), Some(145)].contains(&error.raw_os_error()) {
                return Err(error.into());
            }
            unsafe {
                MoveFileExW(
                    PCWSTR(wide(&path.to_string_lossy()).as_ptr()),
                    PCWSTR::null(),
                    MOVEFILE_DELAY_UNTIL_REBOOT,
                )?;
            }
            pending += 1;
        }
    }
    Ok(pending)
}
fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    children(root, &mut out)?;
    Ok(out.into_iter().filter(|p| p.is_file()).collect())
}
fn digest(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}
fn verified_package(package: &Path) -> Result<Vec<PathBuf>> {
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
        "data/models/model-v4/bigram.winmap",
        "data/models/model-v4/classes.sjc",
        "tools/shanjie-manager.exe",
    ] {
        if !listed.contains(&PathBuf::from(required)) {
            return Err(format!("安裝包缺少 {required}").into());
        }
    }
    if listed
        .iter()
        .any(|p| p.starts_with("data/models") && !p.starts_with("data/models/model-v4"))
    {
        return Err("安裝包只允許最新 model-v4＋classes-v2。".into());
    }
    Ok(listed)
}
fn regsvr(dll: &Path, arch: &str, remove: bool) -> Result<()> {
    let system = if arch == "x64" {
        "System32"
    } else {
        "SysWOW64"
    };
    let executable = PathBuf::from(std::env::var_os("WINDIR").unwrap())
        .join(system)
        .join("regsvr32.exe");
    let mut command = Command::new(executable);
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
fn profiles(remove: bool) -> Result<()> {
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
        let result = install(PCWSTR(wide(TIPS).as_ptr()), remove as u32);
        FreeLibrary(module)?;
        if !result.as_bool() {
            return Err("Windows 無法更新善解輸入法清單。".into());
        }
    }
    Ok(())
}
#[derive(serde::Serialize, serde::Deserialize)]
struct Installed {
    version: String,
}
pub fn install(package: &Path, target: &Path) -> Result<usize> {
    let entries = verified_package(package)?;
    safe_tree(target)?;
    fs::create_dir_all(target)?;
    // A fresh directory prevents old PendingFileRenameOperations from deleting a later reinstall.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let version = format!(
        "{}-{stamp:x}",
        &digest(&package.join("x64/Shanjie.dll"))?[..16]
    );
    let binaries = target.join("versions").join(&version);
    fs::create_dir_all(&binaries)?;
    for relative in &entries {
        let destination = binaries.join(relative);
        fs::create_dir_all(destination.parent().unwrap())?;
        fs::copy(package.join(relative), &destination)?;
    }
    let previous: Vec<_> = [("x64", KEY_WOW64_64KEY), ("x86", KEY_WOW64_32KEY)]
        .into_iter()
        .map(|(arch, view)| Ok((arch, registry_dll(view)?)))
        .collect::<Result<_>>()?;
    for arch in ["x64", "x86"] {
        if let Err(error) = regsvr(&binaries.join(arch).join("Shanjie.dll"), arch, false) {
            for (arch, path) in &previous {
                if let Some(path) = path {
                    regsvr(path, arch, false)?;
                }
            }
            let _ = remove_tree(&binaries);
            return Err(error);
        }
    }
    profiles(false)?;
    fs::write(
        target.join("ui-path.txt"),
        format!("versions\\{version}\\ui\\Shanjie.UI.exe"),
    )?;
    fs::write(
        target.join("manager-path.txt"),
        format!("versions\\{version}\\tools\\shanjie-manager.exe"),
    )?;
    fs::write(
        target.join("installed.json"),
        serde_json::to_vec_pretty(&Installed {
            version: version.clone(),
        })?,
    )?;
    for launcher in ["settings.cmd", "uninstall.cmd"] {
        fs::copy(package.join(launcher), target.join(launcher))?;
    }
    // Sweep the entire retired payload, including old shared data, scripts and WinUI hosts.
    let mut pending = 0;
    for entry in fs::read_dir(target.join("versions"))? {
        let entry = entry?;
        if entry.file_name() != version.as_str() {
            pending += remove_tree(&entry.path())?;
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
        ]
        .contains(&entry.file_name().to_string_lossy().as_ref())
        {
            pending += remove_tree(&entry.path())?;
        }
    }
    let profile = crate::profile::path();
    if profile.exists() {
        crate::profile::load(&profile)?;
    }
    Ok(pending)
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
            match MessageBoxW(None,w!("要一併刪除個人詞庫、選字學習與設定嗎？\n\n是：完整刪除所有善解資料。\n否：移除程式與設定，保留詞庫和學習供重新安裝使用。\n取消：不進行解除安裝。"),w!("解除安裝善解"),MB_YESNOCANCEL|MB_ICONQUESTION|MB_DEFBUTTON3){IDYES=>Some(true),IDNO=>Some(false),_=>None}
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
            KEY_WRITE | view,
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
pub fn uninstall(target: &Path, policy: UserData) -> Result<Option<usize>> {
    let Some(delete) = choose_data(policy) else {
        return Ok(None);
    };
    safe_tree(target)?;
    profiles(true)?;
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
        for category in [
            GUID_TFCAT_TIP_KEYBOARD,
            GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
            GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
            GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
        ] {
            categories.UnregisterCategory(&id, &category, &id)?;
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
    let mut pending = remove_tree(target)?;
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
    Ok(Some(pending))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_removes_retired_nested_payload_and_rejects_junction() {
        let root = std::env::temp_dir().join(format!("shanjie-cleanup-{}", std::process::id()));
        fs::create_dir_all(root.join("old/data/models/model-v1")).unwrap();
        fs::write(root.join("old/data/models/model-v1/model"), b"old").unwrap();
        assert_eq!(remove_tree(&root).unwrap(), 0);
        assert!(!root.exists());
    }
}
