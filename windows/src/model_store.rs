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
        bundle_map_sha256:None,
    };
    fs::write(
        stage.join("model.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    fs::rename(stage, root().join(id))?;
    Ok(())
}
/// After an update, a chosen package this version no longer validates is replaced by the built-in set,
/// and `model-retired.txt` beside the profile names it for the settings to show once. A package that
/// still validates stays chosen; the settings point out the new built-in set instead.
pub fn retire_unsupported(profile: &Path) -> Result<()> {
    retire_unless(profile, |id| scan().iter().any(|m| m.spec.id == id))
}
fn retire_unless(profile: &Path, usable: impl Fn(&str) -> bool) -> Result<()> {
    if !profile.exists() {
        return Ok(());
    }
    let id = crate::profile::load(profile)?.model_id();
    if id == BUILTIN || usable(&id) {
        return Ok(());
    }
    crate::profile::set_model(profile, BUILTIN)?;
    fs::write(profile.with_file_name("model-retired.txt"), id)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_a_package_this_version_cannot_read_is_replaced() {
        let root = std::env::temp_dir().join(format!("shanjie-retire-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let profile = root.join("profile.ini");
        let saved = "[profile]\nschema=8\n[engine]\nmodel_id=model-v5-classes-v3\n[appearance]\nfont_size=24\n";
        fs::write(&profile, saved).unwrap();
        retire_unless(&profile, |_| true).unwrap();
        assert_eq!(fs::read_to_string(&profile).unwrap(), saved);
        assert!(!root.join("model-retired.txt").exists());
        retire_unless(&profile, |_| false).unwrap();
        let doc = crate::profile::load(&profile).unwrap();
        assert_eq!(doc.model_id(), BUILTIN);
        assert_eq!(doc.number("appearance", "font_size", 0), 24);
        assert_eq!(fs::read_to_string(root.join("model-retired.txt")).unwrap(), "model-v5-classes-v3");
        fs::remove_dir_all(root).unwrap();
    }
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
#[no_mangle]
pub unsafe extern "C" fn sj_catalog_validate(text:*const std::ffi::c_char)->*mut std::ffi::c_char{
    let result=(||->Result<String>{
        if text.is_null(){return Err("缺少清單".into());}
        let c=shanjie_shell::catalog::parse(std::ffi::CStr::from_ptr(text).to_bytes())?;
        Ok(serde_json::to_string(&c)?)
    })();
    match result{Ok(s)=>std::ffi::CString::new(s).unwrap().into_raw(),Err(e)=>{crate::bridge::failure(e);std::ptr::null_mut()}}
}
