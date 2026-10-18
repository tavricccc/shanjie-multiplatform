//! One lock spans dictionary merge, immutable publication and profile pointer update.
use crate::profile::{self, Document, Result};
use sha2::{Digest, Sha256};
pub use shanjie_shell::words::{suggest, validate_word};
use shanjie_shell::words::{content, current, entries, matches, merge};
use std::path::Path;
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
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Inspection {
    pub reading: String,
    pub builtin_word: bool,
    pub builtin_exact: bool,
    pub personal_word: bool,
    pub personal_exact: bool,
    pub model_word: Option<bool>,
    pub model_id: String,
    pub model_fallback: bool,
}
/// Read-only inspection. LM vocabulary membership and lexicon pronunciation are distinct.
pub(crate) fn inspect(data:&Path,word:&str,reading:Option<&str>)->Result<Inspection> {
    validate_word(word)?;
    if reading.is_some_and(|r|r.chars().count()>255){return Err("讀音過長。".into());}
    let lex=shanjie_core::engine::windows_base_lexicon(data).map_err(|_|"無法讀取內建詞庫。")?;
    let folder=profile::folder();let doc=profile::load(&profile::path())?;
    let rows=entries(&current(&folder,&doc)?)?;
    let suggested=suggest(data,word)?;
    let reading=reading.unwrap_or(&suggested).to_owned();
    let (builtin_word,builtin_exact,personal_word,personal_exact)=matches(&lex,&rows,word,&reading);
    let selected=doc.model_id();let path=crate::model_store::resolve(data,&selected);
    let model_fallback=selected!=crate::model_store::BUILTIN&&path==data.join("models/model-v6/bigram.winmap");
    let model_id=if model_fallback{crate::model_store::BUILTIN.into()}else{selected};
    let model_word=shanjie_core::engine::windows_model_contains(data,&path,word).ok();
    Ok(Inspection{reading,builtin_word,builtin_exact,personal_word,personal_exact,model_word,model_id,model_fallback})
}
#[cfg(test)] mod tests {
    use super::*;
    use std::{fs, path::PathBuf};
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
