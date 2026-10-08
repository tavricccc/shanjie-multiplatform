use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{RegisterWindowMessageW, SendNotifyMessageW},
    },
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Clone, Default)]
pub struct Document(pub BTreeMap<String, BTreeMap<String, String>>);
impl Document {
    pub fn parse(text: &str) -> Result<Self> {
        let mut out = Self::default();
        let mut section = String::new();
        for line in text.trim_start_matches('\u{feff}').lines().map(str::trim) {
            if line.is_empty() || line.starts_with([';', '#']) {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].to_lowercase();
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or("設定檔格式有誤，請確認 [分類] 與 key=value。")?;
            if section.is_empty() || key.trim().is_empty() {
                return Err("設定必須放在分類之下。".into());
            }
            out.set(&section, key.trim(), value.trim());
        }
        Ok(out)
    }
    pub fn get<'a>(&'a self, section: &str, key: &str, default: &'a str) -> &'a str {
        self.0
            .get(section)
            .and_then(|s| s.get(key))
            .map(String::as_str)
            .unwrap_or(default)
    }
    pub fn flag(&self, s: &str, k: &str, default: bool) -> bool {
        self.get(s, k, if default { "1" } else { "0" }) == "1"
    }
    pub fn number(&self, s: &str, k: &str, default: u32) -> u32 {
        self.0
            .get(s)
            .and_then(|s| s.get(k))
            .map(|v| v.parse().unwrap_or(default))
            .unwrap_or(default)
    }
    pub fn set(&mut self, s: &str, k: &str, value: impl ToString) {
        self.0
            .entry(s.to_lowercase())
            .or_default()
            .insert(k.to_lowercase(), value.to_string());
    }
    pub fn remove(&mut self, s: &str, k: &str) {
        if let Some(section) = self.0.get_mut(s) {
            section.remove(k);
        }
    }
    pub fn content(&self) -> String {
        let mut out = String::new();
        for (s, fields) in &self.0 {
            out += &format!("[{s}]\r\n");
            for (k, v) in fields {
                out += &format!("{k}={v}\r\n");
            }
            out += "\r\n";
        }
        out
    }
    pub fn migrate(&mut self) {
        let schema = self.number("profile", "schema", 1);
        let windows = self.get("profile", "interaction", "windows") == "windows";
        if schema < 3 {
            for (s, k, v) in [
                ("engine", "prediction", false),
                ("input", "associations", windows),
                ("input", "learning", false),
                ("input", "caps_lock_english", windows),
                ("input", "space_after_commit", true),
                ("shortcuts", "backquote_symbols", windows),
                ("input", "shift_punctuation", true),
                ("input", "plain_punctuation", true),
                ("input", "standalone_tones", true),
                ("shortcuts", "ctrl_space_toggle", true),
            ] {
                if !self.0.get(s).is_some_and(|s| s.contains_key(k)) {
                    self.set(s, k, u32::from(v));
                }
            }
            let escape = if self.flag("input", "escape_clears", true) {
                "staged"
            } else {
                "ignore"
            };
            if !self
                .0
                .get("input")
                .is_some_and(|s| s.contains_key("escape"))
            {
                self.set("input", "escape", escape);
            }
            for key in [
                "shift_toggle",
                "ctrl_space_toggle",
                "shift_space_toggle",
                "ctrl_punctuation",
            ] {
                if !self.0.get("shortcuts").is_some_and(|s| s.contains_key(key)) {
                    self.set("shortcuts", key, 1);
                }
            }
            let defaults = Self::parse(if windows {
                include_str!("../../profiles/windows.ini")
            } else {
                include_str!("../../profiles/shanjie.ini")
            })
            .unwrap();
            for (s, fields) in defaults.0 {
                for (k, v) in fields {
                    self.0.entry(s.clone()).or_default().entry(k).or_insert(v);
                }
            }
        }
        self.set("profile", "schema", 4);
        self.set("engine", "model", 4);
        self.set("candidates", "layout", "horizontal");
        for key in [
            "reading",
            "tab_expands",
            "right_expands",
            "grid_up_collapses",
        ] {
            self.remove("candidates", key);
        }
        self.remove("input", "escape_clears");
    }
}
pub fn folder() -> PathBuf {
    PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA")).join("Shanjie")
}
pub fn path() -> PathBuf {
    folder().join("profile.ini")
}
pub fn load(path: &Path) -> Result<Document> {
    if !path.exists() {
        return Document::parse(include_str!("../../profiles/windows.ini"));
    }
    let original = fs::read_to_string(path)?;
    let mut doc = Document::parse(&original)?;
    if doc.number("profile", "schema", 1) < 4 {
        let backup = path.with_extension(format!(
            "ini.schema{}.bak",
            doc.number("profile", "schema", 1)
        ));
        if !backup.exists() {
            fs::copy(path, &backup)?;
        }
        doc.migrate();
        save(path, &doc.content())?;
    }
    Ok(doc)
}
pub fn save(path: &Path, text: &str) -> Result<()> {
    let mut doc = Document::parse(text)?;
    doc.migrate();
    fs::create_dir_all(path.parent().unwrap())?;
    let temporary = path.with_extension("ini.tmp");
    fs::write(&temporary, doc.content())?;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    unsafe {
        MoveFileExW(
            PCWSTR(crate::wide(&temporary.to_string_lossy()).as_ptr()),
            PCWSTR(crate::wide(&path.to_string_lossy()).as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )?;
        let message =
            RegisterWindowMessageW(PCWSTR(crate::wide("Shanjie.ProfileChanged.v1").as_ptr()));
        SendNotifyMessageW(
            HWND(0xffffusize as _),
            message,
            Default::default(),
            Default::default(),
        )?;
    }
    Ok(())
}
pub fn save_dictionary(text: &str, folder: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut content = String::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let f: Vec<_> = line.split_whitespace().collect();
        if !(2..=3).contains(&f.len())
            || f[0].split('-').count() != f[1].chars().count()
            || !f[0].split('-').all(|s| {
                !s.is_empty()
                    && s.chars()
                        .all(|c| ('ㄅ'..='ㄩ').contains(&c) || "ˊˇˋ˙".contains(c))
            })
        {
            return Err("個人詞庫格式有誤：每行填「注音 字詞」，每字讀音以 - 分隔。".into());
        }
        let score = f.get(2).copied().unwrap_or("-0.01");
        let value: f64 = score.parse()?;
        if !value.is_finite() || value > 0.0 {
            return Err("詞庫分數必須是有限的非正數。".into());
        }
        content += &format!("{}\t{}\t{score}\n", f[0], f[1]);
    }
    let name = format!("dictionary-{:x}.tsv", Sha256::digest(content.as_bytes()));
    fs::create_dir_all(folder)?;
    let target = folder.join(&name);
    if !target.exists() {
        fs::write(&target, content)?;
    }
    Ok(name)
}
pub fn clear_learning() -> Result<()> {
    use windows::Win32::{
        Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0},
        System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
    };
    let folder = folder();
    let name = format!(
        "Local\\ShanjieLearning-{}",
        folder.to_str().unwrap().to_uppercase().replace('\\', "_")
    );
    unsafe {
        let mutex = CreateMutexW(None, false, PCWSTR(crate::wide(&name).as_ptr()))?;
        let acquired = WaitForSingleObject(mutex, 2000);
        if acquired != WAIT_OBJECT_0 && acquired != WAIT_ABANDONED {
            CloseHandle(mutex)?;
            return Err("其他程式正在更新選字學習，請稍後再試。".into());
        }
        let result = (|| -> Result<()> {
            fs::create_dir_all(&folder)?;
            for name in ["learning.tsv", "learning.tsv.tmp", "learning.tsv.corrupt"] {
                let p = folder.join(name);
                if p.exists() {
                    fs::remove_file(p)?;
                }
            }
            fs::write(
                folder.join("learning.epoch"),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos()
                    .to_string(),
            )?;
            Ok(())
        })();
        ReleaseMutex(mutex)?;
        CloseHandle(mutex)?;
        result
    }
}
