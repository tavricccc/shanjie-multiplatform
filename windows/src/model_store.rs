//! User model packages are published only after complete source and mapped validation.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub const BUILTIN: &str = "model-v6-classes-v5";
#[derive(Clone, Serialize, Deserialize)]
pub struct Spec {
    pub id: String,
    pub name: String,
    pub model_release: String,
    pub classes_release: String,
    pub model_sha256: String,
    pub classes_sha256: String,
    pub model_size: u64,
    pub classes_size: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Installed {
    #[serde(flatten)]
    pub spec: Spec,
    pub map_sha256: String,
}
pub fn root() -> PathBuf {
    crate::profile::folder().join("Models")
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}
fn digest_ok(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}
fn hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn mapped_source(path: &Path) -> Result<String> {
    let mut bytes = [0u8; 64];
    fs::File::open(path)?.read_exact(&mut bytes)?;
    Ok(bytes[32..64].iter().map(|b| format!("{b:02x}")).collect())
}
fn regular(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("模型附件必須是一般檔案。".into());
    }
    Ok(())
}
fn check_source(path: &Path, size: u64, sha: &str) -> Result<()> {
    regular(path)?;
    if size == 0
        || size > 1024 * 1024 * 1024
        || !digest_ok(sha)
        || fs::metadata(path)?.len() != size
        || hash(path)? != sha
    {
        return Err("模型附件大小或 SHA-256 不符，尚未套用。".into());
    }
    Ok(())
}
fn validate_spec(spec: &Spec) -> Result<()> {
    if !valid_id(&spec.id)
        || spec.name.is_empty()
        || spec.name.len() > 240
        || !digest_ok(&spec.model_sha256)
        || !digest_ok(&spec.classes_sha256)
    {
        return Err("模型資訊格式有誤。".into());
    }
    Ok(())
}
type Stamp = Vec<(u64, SystemTime)>;
static CHECKED: OnceLock<Mutex<Vec<(PathBuf, Stamp, Installed)>>> = OnceLock::new();
pub fn checked(folder: &Path) -> Result<Installed> {
    if ["kn.sjkn", "trigram.sjt3", "tinylm.sjtl"]
        .iter()
        .any(|name| folder.join(name).exists())
    {
        return Err("此配套含本版模型管理尚未支援的附加模型，未列入可用清單。".into());
    }
    if fs::symlink_metadata(folder)?.file_type().is_symlink() {
        return Err("模型資料夾不可為符號連結。".into());
    }
    let files = [
        folder.join("model.json"),
        folder.join("bigram.winmap"),
        folder.join("classes.sjc"),
    ];
    let mut stamp = Vec::new();
    for path in &files {
        regular(path)?;
        let m = fs::metadata(path)?;
        stamp.push((m.len(), m.modified()?));
    }
    let cache = CHECKED.get_or_init(Default::default);
    if let Some((_, _, item)) = cache
        .lock()
        .unwrap()
        .iter()
        .find(|(p, s, _)| p == folder && s == &stamp)
    {
        return Ok(item.clone());
    }
    if stamp[0].0 > 16384 {
        return Err("模型資訊過大。".into());
    }
    let item: Installed = serde_json::from_slice(&fs::read(&files[0])?)?;
    validate_spec(&item.spec)?;
    if folder.file_name().and_then(|p| p.to_str()) != Some(item.spec.id.as_str())
        || !digest_ok(&item.map_sha256)
        || hash(&files[1])? != item.map_sha256
    {
        return Err("模型快取已變動，請重新取得此配套。".into());
    }
    check_source(&files[2], item.spec.classes_size, &item.spec.classes_sha256)?;
    let mapped = shanjie_core::lm::Lm::load(&files[1]).map_err(|_| "模型格式或詞類配對不相容。")?;
    if mapped_source(&files[1])? != item.spec.model_sha256 {
        return Err("模型來源與配套資訊不符。".into());
    }
    drop(mapped);
    let mut entries = cache.lock().unwrap();
    if entries.len() >= 16 {
        entries.remove(0);
    }
    entries.push((folder.to_owned(), stamp, item.clone()));
    Ok(item)
}
pub fn scan() -> Vec<Installed> {
    let Ok(dirs) = fs::read_dir(root()) else {
        return vec![];
    };
    let mut out: Vec<_> = dirs
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name();
            if !id.to_str().is_some_and(valid_id) {
                return None;
            }
            checked(&entry.path()).ok()
        })
        .collect();
    out.sort_by(|a, b| a.spec.id.cmp(&b.spec.id));
    out
}
pub fn resolve(data: &Path, id: &str) -> PathBuf {
    if id == BUILTIN || id.is_empty() {
        return data.join("models/model-v6/bigram.winmap");
    }
    if valid_id(id) {
        let folder = root().join(id);
        if checked(&folder).is_ok() {
            return folder.join("bigram.winmap");
        }
    }
    // Keep the selected id in settings; an unavailable external package must not disable typing.
    data.join("models/model-v6/bigram.winmap")
}
pub fn prepare(staging: &str) -> Result<Installed> {
    if !valid_id(staging) {
        return Err("無效的下載工作識別。".into());
    }
    let folder = root().join(".staging").join(staging);
    if fs::symlink_metadata(&folder)?.file_type().is_symlink() {
        return Err("下載資料夾不可為符號連結。".into());
    }
    let metadata = folder.join("model.json");
    regular(&metadata)?;
    if fs::metadata(&metadata)?.len() > 16384 {
        return Err("模型資訊過大。".into());
    }
    let spec: Spec = serde_json::from_slice(&fs::read(metadata)?)?;
    validate_spec(&spec)?;
    let source = folder.join("bigram.sjlm");
    let classes = folder.join("classes.sjc");
    check_source(&source, spec.model_size, &spec.model_sha256)?;
    check_source(&classes, spec.classes_size, &spec.classes_sha256)?;
    let mapped = folder.join("bigram.winmap");
    {
        let bytes = fs::read(&source)?;
        let table = fs::read(&classes)?;
        let parsed = shanjie_core::lm::Lm::parse_with_classes(&bytes, &table)
            .map_err(|_| "主模型與詞類表不相容。")?;
        parsed
            .write_windows_map(&mapped, &bytes, Some(&table))
            .map_err(|_| "無法建立 Windows 模型快取。")?;
        let verify =
            shanjie_core::lm::Lm::load(&mapped).map_err(|_| "Windows 模型快取驗證失敗。")?;
        if !parsed.windows_map_matches(&verify) {
            return Err("模型轉換結果不等價。".into());
        }
    }
    let item = Installed {
        spec,
        map_sha256: hash(&mapped)?,
    };
    fs::write(folder.join("model.json"), serde_json::to_vec_pretty(&item)?)?;
    // Original download remains in the shared Assets cache; only generated staging copies go away.
    fs::remove_file(&source)?;
    let destination = root().join(&item.spec.id);
    if destination.exists() {
        if let Ok(existing) = checked(&destination) {
            if existing.spec.model_sha256 == item.spec.model_sha256
                && existing.spec.classes_sha256 == item.spec.classes_sha256
            {
                return Ok(existing);
            }
        }
        return Err("已有同名模型資料夾；請先透過模型資料夾保留或重新命名，再重試。".into());
    }
    fs::rename(&folder, &destination)?;
    checked(&destination)
}
/// Preserve an explicitly selected v4 package from an older installed version before it is retired.
pub fn migrate_legacy(data: &Path) -> Result<()> {
    let id = "model-v6-classes-v4";
    if root().join(id).exists() {
        return Ok(());
    }
    let source = data.join("models").join(id);
    if !source.join("bigram.winmap").exists() {
        return Ok(());
    }
    let spec: Spec = serde_json::from_str(include_str!("../../data/model-v6-classes-v4.json"))?;
    check_source(
        &source.join("classes.sjc"),
        spec.classes_size,
        &spec.classes_sha256,
    )?;
    let mapped = shanjie_core::lm::Lm::load(&source.join("bigram.winmap"))
        .map_err(|_| "舊模型不相容，尚未遷移。")?;
    if mapped_source(&source.join("bigram.winmap"))? != spec.model_sha256 {
        return Err("舊模型來源不符。".into());
    }
    drop(mapped);
    let stage = root()
        .join(".staging")
        .join(format!("legacy-{}", std::process::id()));
    fs::create_dir_all(&stage)?;
    fs::copy(source.join("bigram.winmap"), stage.join("bigram.winmap"))?;
    fs::copy(source.join("classes.sjc"), stage.join("classes.sjc"))?;
    let receipt = Installed {
        spec,
        map_sha256: hash(&stage.join("bigram.winmap"))?,
    };
    fs::write(
        stage.join("model.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    fs::rename(stage, root().join(id))?;
    Ok(())
}
pub fn migrate_before_install() -> Result<()> {
    let prefs = crate::profile::load(&crate::profile::path())?;
    if prefs.model_id() == "model-v6-classes-v4" {
        if let Some(previous) = crate::installer::current_binaries()? {
            migrate_legacy(&previous.join("data"))?;
        }
    }
    Ok(())
}
#[no_mangle]
pub extern "C" fn sj_models_scan() -> *mut std::ffi::c_char {
    std::ffi::CString::new(serde_json::to_string(&scan()).unwrap())
        .unwrap()
        .into_raw()
}
#[no_mangle]
pub extern "C" fn sj_models_errors() -> *mut std::ffi::c_char {
    let errors: Vec<String> = fs::read_dir(root())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().to_string();
            if !valid_id(&id) {
                return None;
            }
            checked(&entry.path())
                .err()
                .map(|error| format!("{id}：{error}"))
        })
        .take(8)
        .collect();
    std::ffi::CString::new(serde_json::to_string(&errors).unwrap())
        .unwrap()
        .into_raw()
}
#[no_mangle]
pub unsafe extern "C" fn sj_model_prepare(
    staging: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    let result = (|| -> Result<String> {
        if staging.is_null() {
            return Err("缺少下載工作識別。".into());
        }
        let staging = std::ffi::CStr::from_ptr(staging).to_str()?;
        Ok(serde_json::to_string(&prepare(staging)?)?)
    })();
    match result {
        Ok(text) => std::ffi::CString::new(text).unwrap().into_raw(),
        Err(error) => {
            crate::bridge::failure(error);
            std::ptr::null_mut()
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_are_not_paths() {
        for bad in ["", "../x", "C:\\x", "a/b", ".staging", "A", "a_b"] {
            assert!(!valid_id(bad));
        }
        assert!(valid_id("model-v6-classes-v4"));
    }
    #[test]
    fn missing_selection_falls_back_without_writes() {
        let data = Path::new("test-data");
        assert_eq!(
            resolve(data, "../escape"),
            data.join("models/model-v6/bigram.winmap")
        );
    }
}
