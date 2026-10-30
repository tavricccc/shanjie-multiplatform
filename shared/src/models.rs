//! Downloaded model packages: a package is listed and used only after its sources and the mapped
//! cache have been validated. `root` is the user folder's Models directory on every platform.
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
pub use crate::profile::BUILTIN;
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
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub catalog:Option<crate::catalog::Model>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Installed {
    #[serde(flatten)]
    pub spec: Spec,
    pub map_sha256: String,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub bundle_map_sha256:Option<String>,
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}
pub fn digest_ok(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}
pub fn hash(path: &Path) -> Result<String> {
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
pub fn mapped_source(path: &Path) -> Result<String> {
    let mut bytes = [0u8; 64];
    fs::File::open(path)?.read_exact(&mut bytes)?;
    Ok(bytes[32..64].iter().map(|b| format!("{b:02x}")).collect())
}
pub fn regular(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("模型附件必須是一般檔案。".into());
    }
    Ok(())
}
pub fn check_source(path: &Path, size: u64, sha: &str) -> Result<()> {
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
pub fn validate_spec(spec: &Spec) -> Result<()> {
    if let Some(m)=&spec.catalog {
        crate::catalog::validate(m)?;
        let main=m.assets.iter().find(|a|a.role=="bigram").ok_or("缺少主模型")?;
        let classes=m.assets.iter().find(|a|a.role=="classes");
        if spec.id!=m.id||spec.name!=m.name||main.sha256!=spec.model_sha256||main.size!=spec.model_size
            ||main.release!=spec.model_release||classes.map_or("",|a|a.sha256.as_str())!=spec.classes_sha256
            ||classes.map_or(0,|a|a.size)!=spec.classes_size
            ||classes.map_or("",|a|a.release.as_str())!=spec.classes_release {return Err("模型資訊與完整配套不符。".into());}
        return Ok(());
    }
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
    let metadata=folder.join("model.json");
    regular(&metadata)?;
    if fs::metadata(&metadata)?.len()>262144{return Err("模型資訊過大。".into());}
    let receipt:Installed=serde_json::from_slice(&fs::read(&metadata)?)?;
    if receipt.spec.catalog.is_some(){return checked_bundle(folder,receipt);}
    if ["kn.sjkn", "trigram.sjt3", "trigram-chat.sjt3", "tinylm.sjtl", "colloquial.sjlm"]
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
    if entries.len() >= 64 {
        entries.remove(0);
    }
    entries.push((folder.to_owned(), stamp, item.clone()));
    Ok(item)
}
pub fn scan(root: &Path) -> Vec<Installed> {
    let Ok(dirs) = fs::read_dir(root) else {
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
/// The package's own bundle (scripts/prepare-models.ps1).
pub fn builtin(data: &Path) -> PathBuf {
    data.join("models/builtin/bigram.winmap")
}
pub fn resolve(root: &Path, data: &Path, id: &str) -> PathBuf {
    if id == BUILTIN || id.is_empty() {
        return builtin(data);
    }
    if valid_id(id) {
        let folder = root.join(id);
        if checked(&folder).is_ok() {
            return folder.join("bigram.winmap");
        }
    }
    // Keep the selected id in settings; an unavailable external package must not disable typing.
    builtin(data)
}
pub fn prepare(root: &Path, staging: &str) -> Result<Installed> {
    if !valid_id(staging) {
        return Err("無效的下載工作識別。".into());
    }
    let folder = root.join(".staging").join(staging);
    if fs::symlink_metadata(&folder)?.file_type().is_symlink() {
        return Err("下載資料夾不可為符號連結。".into());
    }
    let metadata = folder.join("model.json");
    regular(&metadata)?;
    if fs::metadata(&metadata)?.len() > 16384 {
        return Err("模型資訊過大。".into());
    }
    let spec: Spec = serde_json::from_slice(&fs::read(metadata)?)?;
    if spec.catalog.is_some(){return prepare_bundle(root,folder,spec);}
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
        bundle_map_sha256:None,
    };
    fs::write(folder.join("model.json"), serde_json::to_vec_pretty(&item)?)?;
    // Original download remains in the shared Assets cache; only generated staging copies go away.
    fs::remove_file(&source)?;
    let destination = root.join(&item.spec.id);
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
fn checked_bundle(folder:&Path,item:Installed)->Result<Installed>{
    validate_spec(&item.spec)?;
    let m=item.spec.catalog.as_ref().unwrap();
    if !crate::catalog::usable(m)||fs::symlink_metadata(folder)?.file_type().is_symlink()
        ||folder.file_name().and_then(|p|p.to_str())!=Some(item.spec.id.as_str()){return Err("此模型配套不支援目前載入能力。".into());}
    let map=folder.join("bigram.winmap");regular(&map)?;
    let mut paths=vec![folder.join("model.json"),map.clone()];
    paths.extend(m.assets.iter().map(|a|folder.join(&a.filename)));
    let stamp:Stamp=paths.iter().map(|p|{regular(p)?;let s=fs::metadata(p)?;Ok((s.len(),s.modified()?))}).collect::<Result<_>>()?;
    let cache=CHECKED.get_or_init(Default::default);
    if let Some((_,_,cached))=cache.lock().unwrap().iter().find(|(p,s,_)|p==folder&&s==&stamp){return Ok(cached.clone());}
    let wanted=item.bundle_map_sha256.as_deref().ok_or("缺少 v2 快取識別。")?;
    // Old readers require a hex map_sha256 and therefore cannot silently load a
    // new bundle while ignoring its advisories. Legacy receipts keep their hash.
    if item.map_sha256!="catalog-v2"||!digest_ok(wanted)||hash(&map)?!=wanted||mapped_source(&map)?!=item.spec.model_sha256{return Err("模型快取已變動。".into());}
    check_bundle_assets(folder,m,true)?;
    let lm=shanjie_core::lm::Lm::load(&map).map_err(|_|"模型及附加檔載入失敗。")?;
    if m.assets.iter().any(|a|a.role=="semantic"){
        shanjie_core::tinylm::TinyLm::load_paired(&folder.join("tinylm.sjtl"),&lm).map_err(|_|"語意模型配對錯誤。")?;
    }
    let mut entries=cache.lock().unwrap();if entries.len()>=64{entries.remove(0);}entries.push((folder.to_owned(),stamp,item.clone()));
    Ok(item)
}
fn check_bundle_assets(folder:&Path,m:&crate::catalog::Model,main:bool)->Result<()> {
    let declared:std::collections::BTreeSet<_>=m.assets.iter().map(|a|a.filename.as_str()).collect();
    for name in ["kn.sjkn","trigram.sjt3","trigram-chat.sjt3","tinylm.sjtl","classes.sjc","colloquial.sjlm","colloquial-attribution.tsv"]{
        if folder.join(name).exists()&&!declared.contains(name){return Err("發現未列入配套的附加檔。".into());}
    }
    for a in &m.assets {
        if a.role=="bigram"&&!main{continue;}
        let path=folder.join(&a.filename);check_source(&path,a.size,&a.sha256)?;
        if !["attribution","license"].contains(&a.role.as_str()){
            let mut magic=[0u8;8];fs::File::open(path)?.read_exact(&mut magic)?;
            if magic!=a.format_magic.as_bytes(){return Err("附件實際格式與清單不符。".into());}
        }
    }
    Ok(())
}
fn prepare_bundle(root:&Path,folder:PathBuf,spec:Spec)->Result<Installed>{
    validate_spec(&spec)?;
    let m=spec.catalog.as_ref().unwrap();
    if !crate::catalog::usable(m){return Err("本版核心不支援此配套。".into());}
    check_bundle_assets(&folder,m,true)?;
    let main=m.assets.iter().find(|a|a.role=="bigram").unwrap();
    let bytes=fs::read(folder.join(&main.filename))?;
    let table=m.assets.iter().find(|a|a.role=="classes").map(|a|fs::read(folder.join(&a.filename))).transpose()?;
    let parsed=match &table {Some(t)=>shanjie_core::lm::Lm::parse_with_classes(&bytes,t),None=>shanjie_core::lm::Lm::parse(&bytes)}
        .map_err(|_|"模型與詞類格式／配對不相容。")?;
    let mapped=folder.join("bigram.winmap");
    parsed.write_windows_map(&mapped,&bytes,table.as_deref()).map_err(|_|"模型映射失敗。")?;
    let loaded=shanjie_core::lm::Lm::load(&mapped).map_err(|_|"附加模型配對或載入失敗。")?;
    if !parsed.windows_map_matches(&loaded){return Err("模型映射不等價。".into());}
    if m.assets.iter().any(|a|a.role=="semantic"){
        shanjie_core::tinylm::TinyLm::load_paired(&folder.join("tinylm.sjtl"),&loaded).map_err(|_|"語意模型配對錯誤。")?;
    }
    drop(loaded);drop(parsed);
    let item=Installed{spec,map_sha256:"catalog-v2".into(),bundle_map_sha256:Some(hash(&mapped)?)};
    fs::write(folder.join("model.json"),serde_json::to_vec_pretty(&item)?)?;
    let destination=root.join(&item.spec.id);
    if destination.exists(){return checked(&destination);}
    fs::rename(&folder,&destination)?;
    checked(&destination)
}
/// Why each package folder under `root` is not listed (at most eight).
pub fn errors(root: &Path) -> Vec<String> {
    fs::read_dir(root)
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
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_are_not_paths() {
        for bad in ["", "../x", "C:\\x", "a/b", ".staging", "A", "a_b"] {
            assert!(!valid_id(bad));
        }
        assert!(valid_id("model-v6-classes-v4") && valid_id(BUILTIN));
    }
    #[test]
    fn missing_selection_falls_back_without_writes() {
        let data = Path::new("test-data");
        assert_eq!(
            resolve(Path::new("test-models"), data, "../escape"),
            builtin(data)
        );
    }
}
