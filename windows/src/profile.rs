use std::{
    fs,
    path::{Path, PathBuf},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{PostMessageW, RegisterWindowMessageW},
    },
};

pub use shanjie_shell::profile::*;

pub(crate) const CHANGED_MESSAGE: &str = "Shanjie.ProfileChanged.v2";
pub(crate) const RESTART_MESSAGE: &str = "Shanjie.Restart.v2";

pub fn folder() -> PathBuf {
    // Only explicit, owned test fixtures can redirect user data via environment.
    if std::env::var("SHANJIE_TEST_PROFILE").as_deref()==Ok("1") {
        return PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("test LOCALAPPDATA")).join("Shanjie");
    }
    static USER:std::sync::OnceLock<PathBuf>=std::sync::OnceLock::new();
    USER.get_or_init(user_folder).clone()
}
pub(crate) fn user_folder()->PathBuf {
    use windows::Win32::{UI::Shell::*,System::{Com::CoTaskMemFree,Threading::*},Security::*,Foundation::*};
    unsafe {
        // An explicit process token makes Shell use this user's profile rather
        // than expanding a host-supplied USERPROFILE environment variable.
        let mut token=HANDLE::default();
        OpenProcessToken(GetCurrentProcess(),TOKEN_QUERY|TOKEN_IMPERSONATE,&mut token).expect("Windows user token unavailable");
        struct Token(HANDLE);impl Drop for Token{fn drop(&mut self){unsafe{let _=CloseHandle(self.0);}}}
        let _token=Token(token);
        let flags=KF_FLAG_NO_PACKAGE_REDIRECTION|KF_FLAG_DONT_VERIFY;
        let result=SHGetKnownFolderPath(&FOLDERID_LocalAppData,flags,Some(token));
        match result {
            Ok(value)=>{
                let path=PathBuf::from(value.to_string().expect("known folder UTF-16"));
                CoTaskMemFree(Some(value.0.cast()));
                path.join("Shanjie")
            },
            Err(e)=>{
                crate::runtime_log::io("profile.known_folder",&std::io::Error::from_raw_os_error(e.code().0));
                // A trusted profile path still avoids a packaged/systemprofile
                // environment inherited from a host. Do not create the folder.
                let value=SHGetKnownFolderPath(&FOLDERID_Profile,flags,Some(token)).expect("Windows user profile unavailable");
                let path=PathBuf::from(value.to_string().expect("profile UTF-16"));
                CoTaskMemFree(Some(value.0.cast()));
                path.join("AppData/Local/Shanjie")
            }
        }
    }
}
pub fn path() -> PathBuf {
    folder().join("profile.ini")
}
pub fn load(path: &Path) -> Result<Document> {
    // Sandboxed hosts cannot see the user folder (exists() is false there), so the core supplies it.
    if path == self::path() && crate::broker::app_container() {
        if let Some(text) = crate::broker::remote_profile() {
            if let Ok(mut doc) = Document::parse(&text) {
                doc.migrate();
                return Ok(doc);
            }
        }
    }
    if !path.exists() {
        return Document::parse(include_str!("../../profiles/windows.ini"));
    }
    let original = match fs::read_to_string(path) {
        // Sandboxed hosts cannot read the user profile; type with defaults instead of failing.
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Document::parse(include_str!("../../profiles/windows.ini"));
        }
        other => other?,
    };
    let mut doc = Document::parse(&original)?;
    // TSF reads this on notification callbacks. Persisting a migration here
    // makes old/new DLLs rewrite each other's schema and broadcast forever.
    doc.migrate();
    Ok(doc)
}
fn write_profile(path: &Path, text: &str) -> Result<()> {
    let mut doc = Document::parse(text)?;
    if doc.number("profile", "schema", 1) > SCHEMA {
        return Err("設定檔由較新版本建立，請更新善解後再儲存。".into());
    }
    if path.exists() {
        let original = fs::read_to_string(path)?;
        let schema = Document::parse(&original)
            .map(|d| d.number("profile", "schema", 1))
            .unwrap_or(0);
        if schema > SCHEMA {
            return Err("設定檔已由較新版本更新，請更新善解後再儲存。".into());
        }
        // Back up the on-disk legacy settings only when the user saves.
        if schema < SCHEMA {
            let backup = path.with_extension(format!("ini.schema{}.bak", schema));
            if !backup.exists() {
                fs::copy(path, backup)?;
            }
        }
    }
    doc.migrate();
    fs::create_dir_all(path.parent().unwrap())?;
    let temporary = path.with_extension("ini.tmp");
    fs::write(&temporary, doc.content())?;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    unsafe {
        MoveFileExW(
            PCWSTR(crate::wide(&temporary.to_string_lossy()).as_ptr()),
            PCWSTR(crate::wide(&path.to_string_lossy()).as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )?;
    }
    Ok(())
}
fn notify(name: &str) -> Result<()> {
    if std::env::var("SHANJIE_TEST_PROFILE").as_deref() == Ok("1") {
        return Ok(());
    }
    unsafe {
        let message = RegisterWindowMessageW(PCWSTR(crate::wide(name).as_ptr()));
        // Old DLLs must not receive a save/restart that makes them migrate back.
        // Post even to our own thread to avoid re-entering the settings callback.
        PostMessageW(
            Some(HWND(0xffffusize as _)),
            message,
            Default::default(),
            Default::default(),
        )?;
    }
    Ok(())
}
pub fn save(path: &Path, text: &str) -> Result<()> {
    let _guard = crate::word_store::lock(path)?;
    write_profile(path, text)?;
    if path == self::path() { notify(CHANGED_MESSAGE) } else { Ok(()) }
}
/// Installing a new version writes an older profile in the current schema, with the usual backup, so the
/// file names what the core already uses. Nothing is broadcast: the new DLLs read it as they load.
pub fn upgrade_file(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let _guard = crate::word_store::lock(path)?;
    let original = fs::read_to_string(path)?;
    if Document::parse(&original)?.number("profile", "schema", 1) >= SCHEMA {
        return Ok(());
    }
    write_profile(path, &original)
}
/// Changes only the chosen model, keeping every other saved value.
pub fn set_model(path: &Path, id: &str) -> Result<()> {
    let _guard = crate::word_store::lock(path)?;
    let mut doc = load(path)?;
    doc.set("engine", "model_id", id);
    write_profile(path, &doc.content())
}
pub fn restart() -> Result<()> {
    notify(RESTART_MESSAGE)
}
pub fn save_dictionary(text: &str, folder: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let content = dictionary_content(text)?;
    let name = format!("dictionary-{:x}.tsv", Sha256::digest(content.as_bytes()));
    crate::dictionary_store::publish(folder, &name, content.as_bytes())?;
    Ok(name)
}

pub fn clear_learning() -> Result<()> {
    use windows::Win32::{
        Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0},
        System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
    };
    let folder = folder();
    let name = format!(
        "Local\\ShanjieLearning-{}",
        folder.to_str().unwrap().to_uppercase().replace('\\', "_")
    );
    unsafe {
        let mutex = CreateMutexW(None, false, PCWSTR(crate::wide(&name).as_ptr()))?;
        let acquired = WaitForSingleObject(mutex, 2000);
        if acquired != WAIT_OBJECT_0 && acquired != WAIT_ABANDONED {
            CloseHandle(mutex)?;
            return Err("其他程式正在更新選字學習，請稍後再試。".into());
        }
        let result = (|| -> Result<()> {
            fs::create_dir_all(&folder)?;
            for name in ["learning.tsv", "learning.tsv.tmp", "learning.tsv.corrupt"] {
                let p = folder.join(name);
                if p.exists() {
                    if !fs::symlink_metadata(&p)?.is_file() {
                        return Err("學習資料的檔案類型異常，清除操作未完成。".into());
                    }
                    if crate::installer::remove_tree(&p)? != 0 {
                        return Err("部分學習資料仍在使用中，清除操作未完成；請稍後重試。".into());
                    }
                }
            }
            fs::write(
                folder.join("learning.epoch"),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos()
                    .to_string(),
            )?;
            Ok(())
        })();
        ReleaseMutex(mutex)?;
        CloseHandle(mutex)?;
        result
    }
}

pub fn apply_preset(source: &Path) -> Result<()> {
    let mut preset = Document::parse(&fs::read_to_string(source)?)?;
    if path().exists() {
        return save(&path(), &merge_preset(&load(&path())?, preset));
    }
    preset.migrate();
    save(&path(), &preset.content())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("shanjie-profile-{name}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root.join("profile.ini")
    }
    #[test]
    fn load_migrates_in_memory_without_rewriting_or_backing_up() {
        let path = fixture("read-only");
        let legacy = "[profile]\nschema=5\n[input]\nplain_punctuation=1\ndictionary=mine.tsv\n[appearance]\nfont_size=24\n";
        fs::write(&path, legacy).unwrap();
        for _ in 0..32 {
            let doc = load(&path).unwrap();
            assert_eq!(doc.number("profile", "schema", 0), SCHEMA);
            assert_eq!(doc.get("input", "dictionary", ""), "mine.tsv");
            assert_eq!(doc.number("appearance", "font_size", 0), 24);
            assert_eq!(fs::read_to_string(&path).unwrap(), legacy);
        }
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }
    #[test]
    fn explicit_save_backs_up_legacy_profile_and_preserves_first_backup() {
        let path = fixture("explicit-save");
        let legacy = "[profile]\nschema=5\n[input]\nplain_punctuation=1\ndictionary=mine.tsv\n";
        fs::write(&path, legacy).unwrap();
        let draft = load(&path).unwrap();
        write_profile(&path, &draft.content()).unwrap();
        let backup = path.with_extension("ini.schema5.bak");
        assert_eq!(fs::read_to_string(&backup).unwrap(), legacy);
        assert_eq!(Document::parse(&fs::read_to_string(&path).unwrap()).unwrap().number("profile", "schema", 0), SCHEMA);
        fs::write(&path, "[profile]\nschema=5\n").unwrap();
        write_profile(&path, &draft.content()).unwrap();
        assert_eq!(fs::read_to_string(&backup).unwrap(), legacy);
    }
    #[test]
    fn install_upgrade_writes_the_current_schema_once() {
        let path = fixture("install-upgrade");
        let legacy = "[profile]\nschema=7\ninteraction=windows\n[engine]\nmodel_id=model-v6-classes-v5\n[appearance]\nfont_size=24\n";
        fs::write(&path, legacy).unwrap();
        upgrade_file(&path).unwrap();
        let doc = Document::parse(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc.number("profile", "schema", 0), SCHEMA);
        assert_eq!(doc.model_id(), shanjie_shell::profile::BUILTIN);
        assert_eq!(doc.number("appearance", "font_size", 0), 24);
        assert_eq!(fs::read_to_string(path.with_extension("ini.schema7.bak")).unwrap(), legacy);
        let current = fs::read_to_string(&path).unwrap();
        upgrade_file(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), current);
        upgrade_file(&path.with_file_name("missing.ini")).unwrap();
        assert!(!path.with_file_name("missing.ini").exists());
    }
    #[test]
    fn future_schema_is_not_downgraded_or_overwritten() {
        let path = fixture("future");
        let future = "[profile]\nschema=99\n[future]\nchoice=keep\n";
        fs::write(&path, future).unwrap();
        let doc = load(&path).unwrap();
        assert_eq!(doc.number("profile", "schema", 0), 99);
        assert_eq!(doc.get("future", "choice", ""), "keep");
        assert!(write_profile(&path, future).is_err());
        assert!(write_profile(&path, "[profile]\nschema=6\n").is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), future);
    }
}
