//! One lock spans dictionary merge, immutable publication and profile pointer update.
use crate::profile::{self, Document, Result};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::{Path, PathBuf}};
use windows::{core::PCWSTR, Win32::{Foundation::*, System::Threading::*}};
pub(crate) struct Guard(HANDLE);
impl Drop for Guard { fn drop(&mut self) { unsafe { let _ = ReleaseMutex(self.0); let _ = CloseHandle(self.0); } } }
pub(crate) fn lock(path: &Path) -> Result<Guard> {
    let identity = path.to_string_lossy().to_uppercase();
    let name = crate::wide(&format!("Local\\ShanjieProfileWrite-{:x}", Sha256::digest(identity.as_bytes())));
    let handle = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr()))? };
    let status = unsafe { WaitForSingleObject(handle, 2000) };
    if status != WAIT_OBJECT_0 && status != WAIT_ABANDONED {
        unsafe { let _ = CloseHandle(handle); }
        return Err("詞庫或設定正在更新，請稍後重試；草稿已保留。".into());
    }
    Ok(Guard(handle))
}
fn dictionary_path(folder: &Path, doc: &Document) -> Result<PathBuf> {
    let name = doc.get("input", "dictionary", "dictionary.tsv");
    if Path::new(name).components().count() != 1 || !matches!(Path::new(name).components().next(), Some(std::path::Component::Normal(_))) {
        return Err("個人詞庫路徑必須是資料夾內的檔名。".into());
    }
    Ok(folder.join(name))
}
fn current(folder: &Path, doc: &Document) -> Result<String> {
    let path = dictionary_path(folder, doc)?;
    if path.try_exists()? { Ok(fs::read_to_string(path)?) } else { Ok(String::new()) }
}
type Entries = BTreeMap<(String, String), String>;
fn entries(text: &str) -> Result<Entries> {
    let content = profile::dictionary_content(text)?;
    let mut out = Entries::new();
    for row in content.lines() {
        let f: Vec<_> = row.split('\t').collect();
        out.insert((f[0].into(), f[1].into()), f[2].into());
    }
    Ok(out)
}
fn content(rows: &Entries) -> String {
    rows.iter().map(|((reading, word), score)| format!("{reading}\t{word}\t{score}\n")).collect()
}
fn merge(base: &Entries, draft: &Entries, latest: &Entries) -> Result<Entries> {
    let mut out = latest.clone();
    for key in base.keys().chain(draft.keys()) {
        if draft.get(key) == base.get(key) { continue; }
        if latest.get(key) != base.get(key) && latest.get(key) != draft.get(key) {
            return Err("同一詞條已在其他地方修改，請保留草稿並重新載入詞庫後合併。".into());
        }
        if let Some(score) = draft.get(key) { out.insert(key.clone(), score.clone()); }
        else { out.remove(key); }
    }
    Ok(out)
}
pub fn save_settings(text: &str, draft: &str, base: &str) -> Result<String> {
    save_settings_at(&profile::path(),text,draft,base)
}
fn save_settings_at(path:&Path, text:&str, draft:&str, base:&str) -> Result<String> {
    let folder = path.parent().ok_or("設定路徑有誤")?;
    let _guard = lock(&path)?;
    let latest = profile::load(&path)?;
    let rows = merge(&entries(base)?, &entries(draft)?, &entries(&current(folder, &latest)?)?)?;
    let mut doc = Document::parse(text)?;
    let keyboard=doc.number("input","keyboard",0);
    if crate::installation::keyboard(keyboard,crate::installation::available())!=keyboard { return Err("此鍵盤配置尚未安裝，請選擇可用配置。".into()); }
    let name = profile::save_dictionary(&content(&rows), folder)?;
    doc.set("input", "dictionary", &name);
    profile::save(&path, &doc.content())?;
    Ok(name)
}
pub fn add(word: &str, reading: &str) -> Result<()> {
    add_at(&profile::path(),word,reading)
}
fn add_at(path:&Path, word:&str, reading:&str) -> Result<()> {
    validate_word(word)?;
    let row = format!("{reading}\t{word}\t-0.01");
    let extra = entries(&row)?;
    let folder = path.parent().ok_or("設定路徑有誤")?;
    let _guard = lock(&path)?;
    let mut doc = profile::load(&path)?;
    let mut rows = entries(&current(folder, &doc)?)?;
    // Retry after an ambiguous pipe reply is idempotent; preserve a custom score.
    for (key, score) in extra { rows.entry(key).or_insert(score); }
    let name = profile::save_dictionary(&content(&rows), folder)?;
    doc.set("input", "dictionary", name);
    profile::save(&path, &doc.content())
}
pub fn validate_word(word: &str) -> Result<()> {
    if !(1..=32).contains(&word.chars().count()) || !word.chars().all(|c| ('\u{3400}'..='\u{9fff}').contains(&c) || ('\u{f900}'..='\u{faff}').contains(&c) || ('\u{20000}'..='\u{323af}').contains(&c)) {
        return Err("請反白 1–32 個漢字；此版加詞浮層只接受漢字詞條。".into());
    }
    Ok(())
}
pub fn suggest(data: &Path, word: &str) -> Result<String> {
    validate_word(word)?;
    let lex = shanjie_core::engine::load_lexicon(data).map_err(|_| "無法讀取內建詞庫，請手動填寫讀音。")?;
    if let Some((reading, _)) = lex.word_info(word) { return Ok(reading.join("-")); }
    let readings: Option<Vec<_>> = word.chars().map(|ch| lex.word_info(&ch.to_string()).map(|(r,_)| r.join("-"))).collect();
    Ok(readings.unwrap_or_default().join("-"))
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn draft_preserves_concurrent_additions_and_explicit_deletions() {
        let base = entries("ㄋㄧˇ 你").unwrap();
        let draft = entries("ㄏㄠˇ 好").unwrap();
        let latest = entries("ㄋㄧˇ 你\nㄐㄧㄚ 家").unwrap();
        let merged = merge(&base,&draft,&latest).unwrap();
        assert_eq!(merged.len(),2); assert!(merged.contains_key(&("ㄐㄧㄚ".into(),"家".into())));
        assert!(!merged.contains_key(&("ㄋㄧˇ".into(),"你".into())));
    }
    #[test] fn conflicting_score_edits_are_rejected() {
        assert!(merge(&entries("ㄋㄧˇ 你 -1").unwrap(),&entries("ㄋㄧˇ 你 -2").unwrap(),&entries("ㄋㄧˇ 你 -3").unwrap()).is_err());
    }
    #[test] fn concurrent_additions_and_stale_settings_are_merged_atomically() {
        let folder=std::env::var_os("SHANJIE_TEST_ROOT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
            .join(format!("add-word-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&folder).unwrap(); let path=folder.join("profile.ini");
        let original=profile::load(&path).unwrap().content();
        std::thread::scope(|s| {
            for (word,reading) in [("回家","ㄏㄨㄟˊ-ㄐㄧㄚ"),("你好","ㄋㄧˇ-ㄏㄠˇ")] {
                let path=&path; s.spawn(move||add_at(path,word,reading).unwrap());
            }
        });
        let name=save_settings_at(&path,&original,"ㄨㄛˇ 我","").unwrap();
        let saved=fs::read_to_string(folder.join(&name)).unwrap();
        assert_eq!(entries(&saved).unwrap().len(),3);
        add_at(&path,"回家","ㄏㄨㄟˊ-ㄐㄧㄚ").unwrap();
        let doc=profile::load(&path).unwrap(); assert_eq!(doc.get("input","dictionary",""),name);
        assert!(add_at(&path,"回家","ㄏㄨㄟˊ").is_err());
        assert_eq!(fs::read_to_string(folder.join(&name)).unwrap(),saved);
    }
}
