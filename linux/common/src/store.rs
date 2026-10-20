//! Writing the user folder with the Windows guarantees (windows/src/profile.rs, word_store.rs,
//! dictionary_store.rs). flock replaces the named mutexes and rename(2) replaces MoveFileEx; a file
//! that is replaced or cleared is deleted.
use crate::{lock, notify, paths};
use sha2::{Digest, Sha256};
use shanjie_shell::{
    profile::{dictionary_content, Document, Result, SCHEMA},
    words::{content, current, entries, merge, validate_word},
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn stamp() -> Result<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos())
}

/// installer::remove_tree: deletes for good. A link is removed itself, never followed.
pub fn discard(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(_) => Ok(()),
        Ok(metadata) if metadata.is_dir() => Ok(fs::remove_dir_all(path)?),
        Ok(_) => Ok(fs::remove_file(path)?),
    }
}

fn profile_lock(path: &Path) -> Result<lock::Guard> {
    let folder = path.parent().ok_or("設定路徑有誤")?;
    lock::hold(folder, lock::PROFILE, Duration::from_millis(2000))?.ok_or_else(|| "詞庫或設定正在更新，請稍後重試；草稿已保留。".into())
}

fn write_profile(path: &Path, text: &str) -> Result<()> {
    let mut doc = Document::parse(text)?;
    if doc.number("profile", "schema", 1) > SCHEMA {
        return Err("設定檔由較新版本建立，請更新善解後再儲存。".into());
    }
    if path.exists() {
        let original = fs::read_to_string(path)?;
        let schema = Document::parse(&original).map(|d| d.number("profile", "schema", 1)).unwrap_or(0);
        if schema > SCHEMA {
            return Err("設定檔已由較新版本更新，請更新善解後再儲存。".into());
        }
        // Back up the on-disk legacy settings only when the user saves.
        if schema < SCHEMA {
            let backup = path.with_extension(format!("ini.schema{schema}.bak"));
            if !backup.exists() {
                fs::copy(path, backup)?;
            }
        }
    }
    doc.migrate();
    fs::create_dir_all(path.parent().ok_or("設定路徑有誤")?)?;
    let temporary = path.with_extension("ini.tmp");
    let mut file = fs::File::create(&temporary)?;
    file.write_all(doc.content().as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path)?;
    Ok(())
}

fn save_locked(path: &Path, text: &str) -> Result<()> {
    write_profile(path, text)?;
    if path == paths::path() { notify::send(notify::CHANGED) } else { Ok(()) }
}

/// profile::save.
pub fn save(path: &Path, text: &str) -> Result<()> {
    let _guard = profile_lock(path)?;
    save_locked(path, text)
}

/// profile::restart: the settings app saves first, then rebuilds the core in every context.
pub fn restart() -> Result<()> {
    notify::send(notify::RESTART)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stage {
    Written,
    BeforePublish,
    Published,
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = discard(&self.0);
        }
    }
}

/// dictionary_store::publish: write a complete content-addressed file; never reuse an unchecked one.
pub fn publish(folder: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    publish_with(folder, name, bytes, |_| Ok(()))
}

fn publish_with(folder: &Path, name: &str, bytes: &[u8], mut checkpoint: impl FnMut(Stage) -> Result<()>) -> Result<()> {
    fs::create_dir_all(folder)?;
    if fs::symlink_metadata(folder)?.file_type().is_symlink() {
        return Err("個人詞庫資料夾不可為連結。".into());
    }
    let _lock = lock::hold(folder, lock::DICTIONARY, Duration::from_millis(5000))?
        .ok_or("個人詞庫正在儲存，請稍後重試；設定及草稿尚未變更。")?;
    let target = folder.join(name);
    if let Ok(metadata) = fs::symlink_metadata(&target) {
        if !metadata.is_file() {
            return Err("個人詞庫檔案不可為連結或資料夾。".into());
        }
        if metadata.len() == bytes.len() as u64 && fs::read(&target)? == bytes {
            return Ok(());
        }
    }
    let temporary = Temporary(folder.join(format!(".{name}.{}-{:x}-{}.tmp", std::process::id(), stamp()?, NEXT.fetch_add(1, Ordering::Relaxed))));
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary.0)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    checkpoint(Stage::Written)?;
    checkpoint(Stage::BeforePublish)?;
    // The name is the content hash, so a differing file under it is only a damaged copy.
    discard(&target)?;
    fs::rename(&temporary.0, &target)?;
    checkpoint(Stage::Published)?;
    if fs::read(&target)? != bytes {
        return Err("個人詞庫完整性檢查失敗，請重試。".into());
    }
    Ok(())
}

/// profile::save_dictionary.
pub fn save_dictionary(text: &str, folder: &Path) -> Result<String> {
    let content = dictionary_content(text)?;
    let name = format!("dictionary-{:x}.tsv", Sha256::digest(content.as_bytes()));
    publish(folder, &name, content.as_bytes())?;
    Ok(name)
}

/// word_store::save_settings: merge the dictionary draft into what is on disk now, publish it and
/// point the saved settings at it, all under the profile lock. Returns the dictionary file name.
pub fn save_settings(path: &Path, text: &str, draft: &str, base: &str) -> Result<String> {
    let folder = path.parent().ok_or("設定路徑有誤")?;
    let _guard = profile_lock(path)?;
    let latest = paths::load(path)?;
    let rows = merge(&entries(base)?, &entries(draft)?, &entries(&current(folder, &latest)?)?)?;
    let mut doc = Document::parse(text)?;
    // Every keyboard layout is available on Linux; installation::keyboard has nothing to refuse.
    let name = save_dictionary(&content(&rows), folder)?;
    doc.set("input", "dictionary", &name);
    save_locked(path, &doc.content())?;
    Ok(name)
}

/// word_store::add.
pub fn add(path: &Path, word: &str, reading: &str) -> Result<()> {
    validate_word(word)?;
    let extra = entries(&format!("{reading}\t{word}\t-0.01"))?;
    let folder = path.parent().ok_or("設定路徑有誤")?;
    let _guard = profile_lock(path)?;
    let mut doc = paths::load(path)?;
    let mut rows = entries(&current(folder, &doc)?)?;
    // A retry is idempotent; preserve a custom score.
    for (key, score) in extra {
        rows.entry(key).or_insert(score);
    }
    let name = save_dictionary(&content(&rows), folder)?;
    doc.set("input", "dictionary", name);
    save_locked(path, &doc.content())
}

/// profile::clear_learning: under the learning lock the engine takes, move the learning files aside
/// and bump `learning.epoch` so every engine drops what it has in memory.
pub fn clear_learning(folder: &Path) -> Result<()> {
    let _guard = lock::hold(folder, lock::LEARNING, Duration::from_millis(2000))?.ok_or("其他程式正在更新選字學習，請稍後再試。")?;
    for name in ["learning.tsv", "learning.tsv.tmp", "learning.tsv.corrupt"] {
        let p = folder.join(name);
        if let Ok(metadata) = fs::symlink_metadata(&p) {
            if !metadata.is_file() {
                return Err("學習資料的檔案類型異常，清除操作未完成。".into());
            }
            discard(&p)?;
        }
    }
    fs::write(folder.join("learning.epoch"), stamp()?.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> PathBuf {
        let root = std::env::var_os("SHANJIE_TEST_ROOT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        root.join(format!("shanjie-store-{}-{}-{}", std::process::id(), stamp().unwrap(), NEXT.fetch_add(1, Ordering::Relaxed)))
    }

    fn quiet() {
        std::env::set_var("SHANJIE_TEST_PROFILE", "1");
    }

    #[test]
    fn corrupt_content_is_replaced_without_archives() {
        let folder = folder();
        let bytes = b"complete dictionary\n";
        publish(&folder, "dictionary-test.tsv", bytes).unwrap();
        fs::write(folder.join("dictionary-test.tsv"), b"partial").unwrap();
        publish(&folder, "dictionary-test.tsv", bytes).unwrap();
        assert_eq!(fs::read(folder.join("dictionary-test.tsv")).unwrap(), bytes);
        assert!(!folder.join(".deleted_files").exists());
        let modified = fs::metadata(folder.join("dictionary-test.tsv")).unwrap().modified().unwrap();
        publish(&folder, "dictionary-test.tsv", bytes).unwrap();
        assert_eq!(fs::metadata(folder.join("dictionary-test.tsv")).unwrap().modified().unwrap(), modified);
    }

    #[test]
    fn concurrent_publish_returns_only_complete_files() {
        let folder = folder();
        let bytes = vec![b'x'; 262144];
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads: Vec<_> = (0..2)
            .map(|_| {
                let (folder, bytes, barrier) = (folder.clone(), bytes.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    publish_with(&folder, "dictionary-test.tsv", &bytes, |stage| {
                        if stage == Stage::Written {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        Ok(())
                    })
                    .unwrap();
                    assert_eq!(fs::read(folder.join("dictionary-test.tsv")).unwrap(), bytes);
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
    }

    #[test]
    fn interrupted_publish_can_be_retried() {
        for fail in [Stage::Written, Stage::BeforePublish, Stage::Published] {
            let folder = folder();
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("previous.tsv"), b"previous usable dictionary").unwrap();
            let result = publish_with(&folder, "dictionary-test.tsv", b"complete", |stage| if stage == fail { Err("injected interruption".into()) } else { Ok(()) });
            assert!(result.is_err());
            assert_eq!(fs::read(folder.join("previous.tsv")).unwrap(), b"previous usable dictionary");
            publish(&folder, "dictionary-test.tsv", b"complete").unwrap();
            assert_eq!(fs::read(folder.join("dictionary-test.tsv")).unwrap(), b"complete");
            // An interrupted temporary file is kept aside, never left beside the dictionaries.
            assert!(fs::read_dir(&folder).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
        }
    }

    #[test]
    fn symlinked_dictionary_is_refused() {
        let folder = folder();
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("elsewhere"), b"x").unwrap();
        std::os::unix::fs::symlink(folder.join("elsewhere"), folder.join("dictionary-test.tsv")).unwrap();
        assert!(publish(&folder, "dictionary-test.tsv", b"complete").is_err());
        assert_eq!(fs::read(folder.join("elsewhere")).unwrap(), b"x");
    }

    #[test]
    fn concurrent_additions_and_stale_settings_are_merged_atomically() {
        quiet();
        let folder = folder();
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("profile.ini");
        let original = paths::load(&path).unwrap().content();
        std::thread::scope(|s| {
            for (word, reading) in [("回家", "ㄏㄨㄟˊ-ㄐㄧㄚ"), ("你好", "ㄋㄧˇ-ㄏㄠˇ")] {
                let path = &path;
                s.spawn(move || add(path, word, reading).unwrap());
            }
        });
        let name = save_settings(&path, &original, "ㄨㄛˇ 我", "").unwrap();
        let saved = fs::read_to_string(folder.join(&name)).unwrap();
        assert_eq!(entries(&saved).unwrap().len(), 3);
        add(&path, "回家", "ㄏㄨㄟˊ-ㄐㄧㄚ").unwrap();
        assert_eq!(paths::load(&path).unwrap().get("input", "dictionary", ""), name);
        assert!(add(&path, "回家", "ㄏㄨㄟˊ").is_err());
        assert_eq!(fs::read_to_string(folder.join(&name)).unwrap(), saved);
    }

    #[test]
    fn legacy_profile_is_backed_up_once_and_newer_is_refused() {
        quiet();
        let folder = folder();
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("profile.ini");
        fs::write(&path, "[profile]\nschema=5\n").unwrap();
        save(&path, &paths::load(&path).unwrap().content()).unwrap();
        assert_eq!(fs::read_to_string(folder.join("profile.ini.schema5.bak")).unwrap(), "[profile]\nschema=5\n");
        assert_eq!(Document::parse(&fs::read_to_string(&path).unwrap()).unwrap().number("profile", "schema", 0), SCHEMA);
        let newer = format!("[profile]\nschema={}\n", SCHEMA + 1);
        assert!(save(&path, &newer).is_err());
        fs::write(&path, &newer).unwrap();
        assert!(save(&path, paths::TEMPLATE).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), newer);
    }

    #[test]
    fn clearing_learning_deletes_the_files_and_bumps_the_epoch() {
        let folder = folder();
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("learning.tsv"), b"rows").unwrap();
        clear_learning(&folder).unwrap();
        assert!(!folder.join("learning.tsv").exists());
        assert!(!folder.join(".deleted_files").exists());
        let epoch = fs::read_to_string(folder.join("learning.epoch")).unwrap();
        // Held by an engine operation, the clear waits and then reports busy instead of racing it.
        let held = lock::hold(&folder, lock::LEARNING, Duration::ZERO).unwrap().unwrap();
        assert!(clear_learning(&folder).is_err());
        drop(held);
        clear_learning(&folder).unwrap();
        assert_ne!(fs::read_to_string(folder.join("learning.epoch")).unwrap(), epoch);
    }
}
