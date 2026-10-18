//! Personal dictionary rows, independent of how a platform locks and publishes them: parsing, the
//! three-way merge the settings app saves with, word validation and the suggested reading.
use crate::profile::{dictionary_content, Document, Result};
use std::{collections::BTreeMap, fs, path::{Path, PathBuf}};

pub type Entries = BTreeMap<(String, String), String>;

pub fn dictionary_path(folder: &Path, doc: &Document) -> Result<PathBuf> {
    let name = doc.get("input", "dictionary", "dictionary.tsv");
    if Path::new(name).components().count() != 1 || !matches!(Path::new(name).components().next(), Some(std::path::Component::Normal(_))) {
        return Err("個人詞庫路徑必須是資料夾內的檔名。".into());
    }
    Ok(folder.join(name))
}
pub fn current(folder: &Path, doc: &Document) -> Result<String> {
    let path = dictionary_path(folder, doc)?;
    if path.try_exists()? { Ok(fs::read_to_string(path)?) } else { Ok(String::new()) }
}
pub fn entries(text: &str) -> Result<Entries> {
    let content = dictionary_content(text)?;
    let mut out = Entries::new();
    for row in content.lines() {
        let f: Vec<_> = row.split('\t').collect();
        out.insert((f[0].into(), f[1].into()), f[2].into());
    }
    Ok(out)
}
pub fn content(rows: &Entries) -> String {
    rows.iter().map(|((reading, word), score)| format!("{reading}\t{word}\t{score}\n")).collect()
}
pub fn merge(base: &Entries, draft: &Entries, latest: &Entries) -> Result<Entries> {
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
pub fn validate_word(word: &str) -> Result<()> {
    if !(1..=32).contains(&word.chars().count()) || !word.chars().all(|c| ('\u{3400}'..='\u{9fff}').contains(&c) || ('\u{f900}'..='\u{faff}').contains(&c) || ('\u{20000}'..='\u{323af}').contains(&c)) {
        return Err("請反白 1–32 個漢字；此版加詞浮層只接受漢字詞條。".into());
    }
    Ok(())
}
pub fn suggest(data: &Path, word: &str) -> Result<String> {
    validate_word(word)?;
    let lex = shanjie_core::engine::windows_base_lexicon(data).map_err(|_| "無法讀取內建詞庫，請手動填寫讀音。")?;
    if let Some((reading, _)) = lex.word_info(word) { return Ok(reading.join("-")); }
    let readings: Option<Vec<_>> = word.chars().map(|ch| lex.word_info(&ch.to_string()).map(|(r,_)| r.join("-"))).collect();
    Ok(readings.unwrap_or_default().join("-"))
}
/// (built-in word, built-in exact reading, personal word, personal exact reading).
pub fn matches(lex:&shanjie_core::Lexicon, rows:&Entries, word:&str, reading:&str)->(bool,bool,bool,bool) {
    let key:Vec<_>=reading.split('-').map(str::to_owned).collect();
    (lex.word_info(word).is_some(),lex.entries(&key).iter().any(|(w,_)|*w==word),
        rows.keys().any(|(_,w)|w==word),rows.contains_key(&(reading.into(),word.into())))
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
    #[test] fn inspection_distinguishes_words_from_exact_pronunciations() {
        let lex=shanjie_core::Lexicon::parse("ㄊㄡˊ-ㄈㄚˇ 頭髮 -0.1\nㄈㄚˇ 法 -0.2\n").unwrap();
        let rows=entries("ㄊㄡˊ-ㄈㄚˇ 頭髮\nㄗˋ-ㄉㄧㄥˋ 自訂").unwrap();
        assert_eq!(matches(&lex,&rows,"頭髮","ㄊㄡˊ-ㄈㄚˇ"),(true,true,true,true));
        assert_eq!(matches(&lex,&rows,"頭髮","ㄊㄡˊ-ㄈㄚˊ"),(true,false,true,false));
        assert_eq!(matches(&lex,&rows,"自訂","ㄗˋ-ㄉㄧㄥˋ"),(false,false,true,true));
        assert_eq!(matches(&lex,&rows,"新詞","ㄒㄧㄣ-ㄘˊ"),(false,false,false,false));
    }
}
