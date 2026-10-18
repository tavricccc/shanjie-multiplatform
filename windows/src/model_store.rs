//! User model packages are published only after complete source and mapped validation.
//! The validation lives in shanjie_shell::models; this keeps the Windows folder, migration and C ABI.
use shanjie_shell::models::{check_source, hash, mapped_source, Installed, Spec};
pub use shanjie_shell::models::{valid_id, BUILTIN};
use std::{fs, path::{Path, PathBuf}};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub fn root() -> PathBuf {
    crate::profile::folder().join("Models")
}
pub fn checked(folder: &Path) -> Result<Installed> {
    shanjie_shell::models::checked(folder)
}
pub fn scan() -> Vec<Installed> {
    shanjie_shell::models::scan(&root())
}
pub fn resolve(data: &Path, id: &str) -> PathBuf {
    shanjie_shell::models::resolve(&root(), data, id)
}
pub fn prepare(staging: &str) -> Result<Installed> {
    shanjie_shell::models::prepare(&root(), staging)
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
    std::ffi::CString::new(serde_json::to_string(&shanjie_shell::models::errors(&root())).unwrap())
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
