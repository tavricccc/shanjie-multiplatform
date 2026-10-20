//! Publish complete content-addressed dictionaries; never reuse unchecked files.
use crate::profile::Result;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
        System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
    },
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Lock(HANDLE);
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = crate::installer::remove_tree(&self.0);
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Written,
    BeforePublish,
    Published,
}
pub fn publish(folder: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    publish_with(folder, name, bytes, |_| Ok(()))
}
fn publish_with(
    folder: &Path,
    name: &str,
    bytes: &[u8],
    mut checkpoint: impl FnMut(Stage) -> Result<()>,
) -> Result<()> {
    fs::create_dir_all(folder)?;
    if fs::symlink_metadata(folder)?.file_attributes() & 0x400 != 0 {
        return Err("個人詞庫資料夾不可為連結。".into());
    }
    let identity = format!("{}\\{name}", folder.canonicalize()?.to_string_lossy()).to_uppercase();
    let mutex_name = crate::wide(&format!(
        "Local\\ShanjieDictionary-{:x}",
        Sha256::digest(identity.as_bytes())
    ));
    let handle = unsafe { CreateMutexW(None, false, PCWSTR(mutex_name.as_ptr()))? };
    let status = unsafe { WaitForSingleObject(handle, 5000) };
    if status != WAIT_OBJECT_0 && status != WAIT_ABANDONED {
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Err("個人詞庫正在儲存，請稍後重試；設定及草稿尚未變更。".into());
    }
    let _lock = Lock(handle);
    let target = folder.join(name);
    if target.try_exists()? {
        let metadata = fs::symlink_metadata(&target)?;
        if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
            return Err("個人詞庫檔案不可為連結或資料夾。".into());
        }
        if metadata.len() == bytes.len() as u64 && fs::read(&target)? == bytes {
            return Ok(());
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let temporary = Temporary(folder.join(format!(
        ".{name}.{}-{stamp:x}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary.0)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    checkpoint(Stage::Written)?;
    checkpoint(Stage::BeforePublish)?;
    if target.try_exists()? {
        // The name is the content hash, so a differing file under it is only a damaged copy.
        if crate::installer::remove_tree(&target)? != 0 {
            return Err("舊詞庫使用中，未完成修復；請稍後重試。".into());
        }
    }
    fs::rename(&temporary.0, &target)?;
    checkpoint(Stage::Published)?;
    if fs::read(&target)? != bytes {
        return Err("個人詞庫完整性檢查失敗，請重試。".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn folder() -> PathBuf {
        let root = std::env::var_os("SHANJIE_TEST_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        root.join(format!(
            "dictionary-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
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
        let stamp = fs::metadata(folder.join("dictionary-test.tsv"))
            .unwrap()
            .modified()
            .unwrap();
        publish(&folder, "dictionary-test.tsv", bytes).unwrap();
        assert_eq!(
            fs::metadata(folder.join("dictionary-test.tsv"))
                .unwrap()
                .modified()
                .unwrap(),
            stamp
        );
    }
    #[test]
    fn concurrent_publish_returns_only_complete_files() {
        let folder = folder();
        let bytes = vec![b'x'; 262144];
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let mut threads = Vec::new();
        for _ in 0..2 {
            let folder = folder.clone();
            let bytes = bytes.clone();
            let barrier = barrier.clone();
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                publish_with(&folder, "dictionary-test.tsv", &bytes, |stage| {
                    if stage == Stage::Written {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Ok(())
                })
                .unwrap();
                assert_eq!(fs::read(folder.join("dictionary-test.tsv")).unwrap(), bytes);
            }));
        }
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
            assert!(
                publish_with(&folder, "dictionary-test.tsv", b"complete", |stage| {
                    if stage == fail {
                        Err("injected interruption".into())
                    } else {
                        Ok(())
                    }
                })
                .is_err()
            );
            assert_eq!(
                fs::read(folder.join("previous.tsv")).unwrap(),
                b"previous usable dictionary"
            );
            publish(&folder, "dictionary-test.tsv", b"complete").unwrap();
            assert_eq!(
                fs::read(folder.join("dictionary-test.tsv")).unwrap(),
                b"complete"
            );
        }
    }
}
