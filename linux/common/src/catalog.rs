//! The online model list and downloads (ui/Models/ModelCatalog.cs). Only packages in the
//! compatibility catalog whose release assets match its size and SHA-256 are offered; a download
//! resumes from the shared Assets cache and is published through models::prepare.
use crate::paths;
use serde::{Deserialize, Serialize};
use shanjie_shell::{
    models::{self, Spec, BUILTIN},
    profile::Result,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const AGENT: &str = concat!("Shanjie-Linux/", env!("CARGO_PKG_VERSION"));
const LIST_LIMIT: u64 = 8 * 1024 * 1024;
const RELEASES: &str = "https://api.github.com/repos/Nanako0129/shanjie/releases";
const REMOTE_CATALOG: &str = "https://raw.githubusercontent.com/tavricccc/shanjie-multiplatform/main/MODEL-CATALOG.json";
pub const BUILTIN_NAME: &str = "model-v6＋classes-v5（內建）";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub id: String,
    pub name: String,
    pub revision: String,
}

#[derive(Clone)]
pub struct Row {
    pub name: String,
    pub state: String,
    pub spec: Option<Spec>,
    pub available: bool,
}

#[derive(Deserialize)]
struct Compatibility {
    format: u32,
    models: Vec<Spec>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Asset {
    name: Option<String>,
    size: u64,
    digest: Option<String>,
    browser_download_url: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Release {
    tag_name: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Option<Vec<Asset>>,
}

pub fn builtin() -> Choice {
    Choice { id: BUILTIN.into(), name: BUILTIN_NAME.into(), revision: String::new() }
}

pub fn known() -> Vec<Spec> {
    serde_json::from_str::<Compatibility>(include_str!("../../../MODEL-CATALOG.json")).map(|c| c.models).unwrap_or_default()
}

/// Validated downloaded packages, each with the stamp of its receipt as the revision.
pub fn installed() -> Vec<Choice> {
    let root = paths::models();
    models::scan(&root)
        .into_iter()
        .map(|item| {
            let stamp = fs::metadata(root.join(&item.spec.id).join("model.json"))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            Choice { id: item.spec.id, name: format!("{}（已下載）", item.spec.name), revision: stamp.to_string() }
        })
        .collect()
}

pub fn issues() -> Vec<String> {
    models::errors(&paths::models())
}

fn valid_spec(s: &Spec) -> bool {
    let release = |r: &str, prefix: &str| r.starts_with(prefix) && r.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-');
    let digest = |d: &str| d.len() == 64 && d.bytes().all(|c| c.is_ascii_hexdigit());
    !s.id.is_empty()
        && s.id.len() <= 80
        && !s.name.is_empty()
        && s.name.chars().count() <= 240
        && s.id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && release(&s.model_release, "model-v")
        && release(&s.classes_release, "classes-v")
        && (1..=1 << 30).contains(&s.model_size)
        && (1..=1 << 30).contains(&s.classes_size)
        && digest(&s.model_sha256)
        && digest(&s.classes_sha256)
}

fn has_asset(releases: &[Release], tag: &str, name: &str, size: u64, hash: &str) -> bool {
    releases.iter().any(|r| {
        r.tag_name.as_deref() == Some(tag)
            && !r.draft
            && !r.prerelease
            && r.assets.iter().flatten().any(|a| {
                a.name.as_deref() == Some(name)
                    && a.size == size
                    && a.digest.as_deref() == Some(&format!("sha256:{hash}"))
                    && a.browser_download_url.as_deref() == Some(&format!("https://github.com/Nanako0129/shanjie/releases/download/{tag}/{name}"))
            })
    })
}

pub fn rows(releases: &[Release], known: &[Spec], installed: &[Choice]) -> Vec<Row> {
    let releases: Vec<_> = releases.iter().filter(|r| r.tag_name.is_some() && r.assets.is_some()).cloned().collect();
    let mut out = Vec::new();
    for spec in known.iter().filter(|s| valid_spec(s)) {
        let available = has_asset(&releases, &spec.model_release, "bigram.sjlm", spec.model_size, &spec.model_sha256)
            && has_asset(&releases, &spec.classes_release, "classes.sjc", spec.classes_size, &spec.classes_sha256);
        let state = if spec.id == BUILTIN {
            "內建".to_owned()
        } else if installed.iter().any(|c| c.id == spec.id) {
            "已下載".to_owned()
        } else if available {
            format!("可下載 · {:.1} MiB", (spec.model_size + spec.classes_size) as f64 / 1048576.0)
        } else {
            "線上附件無法核對".to_owned()
        };
        out.push(Row { name: spec.name.clone(), state, spec: Some(spec.clone()), available: available && spec.id != BUILTIN });
    }
    for release in &releases {
        let tag = release.tag_name.as_deref().unwrap_or_default();
        if !release.draft && tag.starts_with("model-v") && known.iter().all(|k| k.model_release != tag) {
            let state = if release.prerelease { "預覽資料，尚未支援" } else { "尚未確認相容配套" };
            out.push(Row { name: tag.to_owned(), state: state.into(), spec: None, available: false });
        }
    }
    out
}

/// Runs curl until it exits, killing it when `cancel` is set.
fn curl(arguments: &[&str], cancel: &AtomicBool) -> Result<std::process::Output> {
    let mut child = Command::new("curl")
        .args(["--silent", "--show-error", "--location", "--fail", "--proto", "=https", "--user-agent", AGENT])
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("無法執行 curl：{e}"))?;
    // Drain stdout on a thread so a large reply cannot block the child.
    let mut stdout = child.stdout.take().ok_or("curl 輸出無法讀取。")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut std::io::Read::take(&mut stdout, LIST_LIMIT + 1), &mut bytes).map(|_| bytes)
    });
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Cancelled.into());
        }
        if let Some(status) = child.try_wait()? {
            let stdout = reader.join().map_err(|_| "curl 輸出無法讀取。")??;
            let mut stderr = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = std::io::Read::read_to_string(&mut e, &mut stderr);
            }
            return Ok(std::process::Output { status, stdout, stderr: stderr.into_bytes() });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[derive(Debug)]
pub struct Cancelled;
impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("已取消")
    }
}
impl std::error::Error for Cancelled {}

pub fn is_cancelled(error: &(dyn std::error::Error + 'static)) -> bool {
    error.downcast_ref::<Cancelled>().is_some()
}

fn failed(output: &std::process::Output) -> Box<dyn std::error::Error> {
    let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if message.is_empty() { format!("curl 結束代碼 {}", output.status.code().unwrap_or(-1)).into() } else { message.into() }
}

fn fetch(url: &str, cancel: &AtomicBool) -> Result<String> {
    let limit = LIST_LIMIT.to_string();
    let output = curl(&["--max-time", "25", "--max-filesize", &limit, "--header", "Accept: application/vnd.github+json", url], cancel)?;
    if !output.status.success() {
        return Err(failed(&output));
    }
    if output.stdout.len() as u64 > LIST_LIMIT {
        return Err("線上清單過大。".into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// ModelCatalog.Online: the release list (cached for offline use) and the compatibility catalog.
pub fn online(cancel: &AtomicBool) -> Result<(Vec<Row>, String)> {
    let folder = paths::models();
    fs::create_dir_all(&folder)?;
    let cache = folder.join("catalog-cache.json");
    let mut status = "已更新上游公開清單。".to_owned();
    let fetched = (|| -> Result<Vec<Release>> {
        let mut releases = Vec::new();
        for page in 1..=10 {
            let batch: Vec<Release> = serde_json::from_str(&fetch(&format!("{RELEASES}?per_page=100&page={page}"), cancel)?)?;
            let count = batch.len();
            releases.extend(batch);
            if count < 100 {
                break;
            }
        }
        let temporary = cache.with_extension("json.download");
        fs::write(&temporary, serde_json::to_vec(&releases)?)?;
        fs::rename(&temporary, &cache)?;
        Ok(releases)
    })();
    let releases = match fetched {
        Ok(releases) => releases,
        Err(e) if is_cancelled(e.as_ref()) => return Err(e),
        Err(_) if cache.is_file() => {
            status = format!("無法更新線上清單；顯示 {} 的快取。", local_time(fs::metadata(&cache)?.modified()?));
            serde_json::from_slice(&fs::read(&cache)?)?
        }
        Err(_) => return Ok((vec![], "目前無法連線取得清單；內建及已下載模型仍可使用。請稍後重試。".into())),
    };
    let mut known = known();
    // Optional project-maintained compatibility metadata can add data versions without a binary update.
    match fetch(REMOTE_CATALOG, cancel).and_then(|text| Ok(serde_json::from_str::<Compatibility>(&text)?)) {
        Ok(remote) if remote.format == 1 => {
            for spec in remote.models.into_iter().filter(valid_spec) {
                if known.iter().all(|k| k.id != spec.id) {
                    known.push(spec);
                }
            }
        }
        Err(e) if is_cancelled(e.as_ref()) => return Err(e),
        _ => {}
    }
    Ok((rows(&releases, &known, &installed()), status))
}

#[repr(C)]
struct Tm {
    sec: i32,
    min: i32,
    hour: i32,
    mday: i32,
    mon: i32,
    year: i32,
    wday: i32,
    yday: i32,
    isdst: i32,
    gmtoff: i64,
    zone: *const std::ffi::c_char,
}
extern "C" {
    fn localtime_r(time: *const i64, out: *mut Tm) -> *mut Tm;
}

/// yyyy/MM/dd HH:mm in the local time zone.
fn local_time(time: SystemTime) -> String {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let mut tm = Tm { sec: 0, min: 0, hour: 0, mday: 0, mon: 0, year: 0, wday: 0, yday: 0, isdst: 0, gmtoff: 0, zone: std::ptr::null() };
    if unsafe { localtime_r(&seconds, &mut tm) }.is_null() {
        return String::new();
    }
    format!("{:04}/{:02}/{:02} {:02}:{:02}", tm.year + 1900, tm.mon + 1, tm.mday, tm.hour, tm.min)
}

fn matches(path: &Path, size: u64, sha: &str) -> bool {
    fs::metadata(path).is_ok_and(|m| m.len() == size) && models::hash(path).is_ok_and(|h| h.eq_ignore_ascii_case(sha))
}

fn asset_file(tag: &str, name: &str, size: u64, sha: &str, progress: &(dyn Fn(String) + Sync), cancel: &AtomicBool) -> Result<PathBuf> {
    let assets = paths::models().join("Assets");
    fs::create_dir_all(&assets)?;
    let complete = assets.join(sha);
    let partial = assets.join(format!("{sha}.download"));
    if complete.is_file() && matches(&complete, size, sha) {
        return Ok(complete);
    }
    let mut offset = fs::metadata(&partial).map_or(0, |m| m.len());
    if offset > size {
        fs::remove_file(&partial)?;
        offset = 0;
    }
    if offset < size {
        let url = format!("https://github.com/Nanako0129/shanjie/releases/download/{tag}/{name}");
        let target = partial.to_string_lossy().into_owned();
        let limit = size.to_string();
        let done = AtomicBool::new(false);
        let watch = std::thread::scope(|s| {
            let reporter = s.spawn(|| {
                while !done.load(Ordering::Relaxed) {
                    let have = fs::metadata(&partial).map_or(0, |m| m.len()).min(size);
                    progress(format!("下載 {tag}：{:.1} / {:.1} MiB（{:.0}%）", have as f64 / 1048576.0, size as f64 / 1048576.0, 100.0 * have as f64 / size as f64));
                    std::thread::sleep(Duration::from_millis(250));
                }
            });
            let resume = if offset > 0 { "-" } else { "0" };
            let mut output = curl(&["--max-time", "900", "--max-filesize", &limit, "--continue-at", resume, "--output", &target, &url], cancel);
            // 33: the server ignored the range; start the fragment again.
            if output.as_ref().is_ok_and(|o| o.status.code() == Some(33)) {
                let _ = fs::remove_file(&partial);
                output = curl(&["--max-time", "900", "--max-filesize", &limit, "--output", &target, &url], cancel);
            }
            done.store(true, Ordering::Relaxed);
            let _ = reporter.join();
            output
        })?;
        if !watch.status.success() {
            return Err(format!("尚未下載完成（{}）；下載片段會保留供下次續傳。", failed(&watch)).into());
        }
        if fs::metadata(&partial)?.len() > size {
            fs::remove_file(&partial)?;
            return Err("下載檔案超過預期大小，尚未套用。".into());
        }
    }
    progress(format!("核對 {tag} 的完整性…"));
    if !matches(&partial, size, sha) {
        fs::remove_file(&partial)?;
        return Err("下載檔案的大小或 SHA-256 不符，請重試。".into());
    }
    fs::rename(&partial, &complete)?;
    Ok(complete)
}

/// ModelCatalog.Download: fetch both assets, stage them with the receipt and publish the package.
pub fn download(spec: &Spec, progress: &(dyn Fn(String) + Sync), cancel: &AtomicBool) -> Result<Choice> {
    if !valid_spec(spec) {
        return Err("此模型配套資訊無效。".into());
    }
    if spec.id == BUILTIN {
        return Ok(builtin());
    }
    if let Some(existing) = installed().into_iter().find(|c| c.id == spec.id) {
        return Ok(existing);
    }
    let model = asset_file(&spec.model_release, "bigram.sjlm", spec.model_size, &spec.model_sha256, progress, cancel)?;
    let classes = asset_file(&spec.classes_release, "classes.sjc", spec.classes_size, &spec.classes_sha256, progress, cancel)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(Cancelled.into());
    }
    let work = format!("{:x}{:x}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos());
    let root = paths::models();
    let stage = root.join(".staging").join(&work);
    fs::create_dir_all(&stage)?;
    progress("下載完成，正在建立及驗證模型快取…".into());
    fs::copy(model, stage.join("bigram.sjlm"))?;
    fs::copy(classes, stage.join("classes.sjc"))?;
    fs::write(stage.join("model.json"), serde_json::to_vec(spec)?)?;
    models::prepare(&root, &work)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(Cancelled.into());
    }
    Ok(Choice { id: spec.id.clone(), name: format!("{}（已下載）", spec.name), revision: String::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str, model_release: &str) -> Spec {
        Spec {
            id: id.into(),
            name: id.into(),
            model_release: model_release.into(),
            classes_release: "classes-v9".into(),
            model_sha256: "a".repeat(64),
            classes_sha256: "b".repeat(64),
            model_size: 1048576,
            classes_size: 1048576,
            catalog: None,
        }
    }

    fn release(tag: &str, prerelease: bool, assets: &[(&str, &str)]) -> Release {
        Release {
            tag_name: Some(tag.into()),
            draft: false,
            prerelease,
            assets: Some(
                assets
                    .iter()
                    .map(|(name, hash)| Asset {
                        name: Some((*name).into()),
                        size: 1048576,
                        digest: Some(format!("sha256:{hash}")),
                        browser_download_url: Some(format!("https://github.com/Nanako0129/shanjie/releases/download/{tag}/{name}")),
                    })
                    .collect(),
            ),
        }
    }

    #[test]
    fn only_matching_release_assets_are_offered() {
        let (a, b) = ("a".repeat(64), "b".repeat(64));
        let releases = [release("model-v9", false, &[("bigram.sjlm", &a)]), release("classes-v9", false, &[("classes.sjc", &b)]), release("model-v10", true, &[])];
        let known = [spec("model-v9-classes-v9", "model-v9"), spec("model-v8-classes-v9", "model-v8"), spec("Bad", "model-v9")];
        let rows = rows(&releases, &known, &[]);
        let summary: Vec<_> = rows.iter().map(|r| (r.name.as_str(), r.state.as_str(), r.available)).collect();
        assert_eq!(
            summary,
            [
                ("model-v9-classes-v9", "可下載 · 2.0 MiB", true),
                ("model-v8-classes-v9", "線上附件無法核對", false),
                ("model-v10", "預覽資料，尚未支援", false),
            ]
        );
        // A different digest in the release is not the package the catalog vouches for.
        let tampered = [release("model-v9", false, &[("bigram.sjlm", &b)]), release("classes-v9", false, &[("classes.sjc", &b)])];
        assert!(!rows_available(&tampered));
    }

    fn rows_available(releases: &[Release]) -> bool {
        rows(releases, &[spec("model-v9-classes-v9", "model-v9")], &[])[0].available
    }

    #[test]
    fn bundled_catalog_lists_the_builtin_package() {
        let known = known();
        assert!(known.iter().any(|s| s.id == BUILTIN && valid_spec(s)));
    }
}

#[cfg(test)]
mod live {
    /// SHANJIE_LIVE=1 cargo test live -- --ignored --nocapture (uses the network and XDG_DATA_HOME).
    #[test]
    #[ignore]
    fn online_list() {
        let (rows, status) = super::online(&Default::default()).unwrap();
        println!("{status}");
        for row in rows {
            println!("{} — {} {}", row.name, row.state, row.available);
        }
    }

    /// SHANJIE_LIVE_MODEL=<id>: download, convert and publish that catalog package.
    #[test]
    #[ignore]
    fn download_one() {
        let id = std::env::var("SHANJIE_LIVE_MODEL").unwrap();
        let spec = super::known().into_iter().find(|s| s.id == id).unwrap();
        let last = std::sync::Mutex::new(String::new());
        let choice = super::download(&spec, &|s| *last.lock().unwrap() = s, &Default::default()).unwrap();
        println!("{} {} / {}", choice.id, choice.name, last.lock().unwrap());
        assert!(super::installed().iter().any(|c| c.id == id));
    }
}
