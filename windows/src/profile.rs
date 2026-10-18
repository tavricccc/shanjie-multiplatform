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
    // AppContainer LOCALAPPDATA is the package folder; the core endpoint uses the user's.
    if crate::broker::app_container() {
        if let Some(home) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(home).join("AppData").join("Local").join("Shanjie");
        }
    }
    PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA")).join("Shanjie")
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
        let previous = load(&path())?;
        for (section, keys) in [
            ("engine", &["ranking", "demote", "class_table", "model_id", "model_revision"][..]),
            (
                "input",
                &[
                    "dictionary",
                    "keyboard",
                    "learning",
                    "context",
                    "chinese",
                    "full_width",
                ][..],
            ),
            ("candidates", &["big5"][..]),
            ("appearance", &["theme", "font_size", "mode_indicator"][..]),
        ] {
            for key in keys {
                if let Some(value) = previous.0.get(section).and_then(|s| s.get(*key)) {
                    preset.set(section, key, value);
                }
            }
        }
    }
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
