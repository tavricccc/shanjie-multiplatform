//! The settings draft (ui/ViewModels/SettingsViewModel*.cs). Loading reads every key with the same
//! defaults as ApplyDocument and writes it back explicitly, as WriteDocument does before a save, so
//! the widgets bind straight to `[section] key` and a save writes the same file Windows would.
use shanjie_linux::{catalog, paths, store};
use shanjie_shell::profile::{self, Document, Result};
use std::path::{Path, PathBuf};

/// Flags read with a default that depends on the operation set (`w` is "windows").
const FLAGS: &[(&str, &str, fn(bool) -> bool)] = &[
    ("engine", "prediction", |w| !w),
    ("engine", "demote", |_| true),
    ("shortcuts", "shift_toggle", |w| w),
    ("shortcuts", "ctrl_space_toggle", |_| false),
    ("shortcuts", "shift_space_toggle", |w| w),
    ("shortcuts", "ctrl_punctuation", |w| w),
    ("input", "chinese", |_| true),
    ("input", "full_width", |_| false),
    ("input", "caps_lock_english", |w| w),
    ("input", "space_after_commit", |_| true),
    ("input", "pinyin_space_converts", |w| w),
    ("input", "space_commits", |_| false),
    ("candidates", "space_operates", |_| false),
    ("candidates", "space_pages", |_| true),
    ("candidates", "wrap_pages", |_| true),
    ("shortcuts", "backquote_symbols", |w| w),
    ("shortcuts", "code_input", |w| w),
    ("shortcuts", "symbol_keyboard", |w| w),
    ("shortcuts", "ctrl_backslash", |w| !w),
    ("shortcuts", "forget_candidate", |_| false),
    ("appearance", "mode_indicator", |_| true),
    ("input", "learning", |_| false),
    ("input", "associations", |_| false),
    ("input", "alphanumeric", |w| w),
    ("input", "mixed_tolerance", |_| false),
    ("shortcuts", "add_word", |_| true),
    ("input", "mixed_input", |_| false),
    ("input", "upstream_english", |_| false),
    ("input", "english_inline", |_| true),
    ("input", "context", |_| true),
    ("candidates", "big5", |_| false),
    ("input", "standalone_tones", |w| !w),
    ("input", "space_inputs_zhuyin", |_| true),
    ("candidates", "enter_commits", |w| w),
    ("candidates", "enter_to_cursor", |_| false),
    ("candidates", "up_cycles", |w| w),
    ("candidates", "backspace_deletes", |_| false),
];

const NUMBERS: &[(&str, &str, u32)] = &[
    ("shortcuts", "shift_side", 0),
    ("appearance", "theme", 0),
    ("appearance", "font_size", 16),
    ("input", "keyboard", 0),
    ("shortcuts", "add_word_side", 0),
    ("shortcuts", "add_word_shortcut", 0),
];

/// PunctuationKey.ShiftRow / PlainRow: (ini key, label).
pub const SHIFT_KEYS: &[(&str, &str)] = &[
    ("shift_comma", "，  Shift＋,"),
    ("shift_period", "。  Shift＋."),
    ("shift_slash", "？  Shift＋/"),
    ("shift_1", "！  Shift＋1"),
    ("shift_semicolon", "：  Shift＋;"),
    ("shift_lbracket", "『  Shift＋["),
    ("shift_rbracket", "』  Shift＋]"),
    ("shift_9", "（  Shift＋9"),
    ("shift_0", "）  Shift＋0"),
    ("shift_backquote", "～  Shift＋`"),
    ("shift_backslash", "｜  Shift＋\\"),
    ("shift_quote", "“  Shift＋'"),
    ("shift_equal", "＋  Shift＋="),
];
pub const PLAIN_KEYS: &[(&str, &str)] = &[
    ("plain_lbracket", "「  ["),
    ("plain_rbracket", "」  ]"),
    ("plain_backslash", "、  \\"),
    ("plain_quote", "‘  '"),
    ("plain_equal", "＝  ="),
    ("plain_backquote", "·  `"),
];

pub const ESCAPE: &[&str] = &["staged", "clear", "ignore"];
pub const OPEN: &[&str] = &["down", "space", "both"];

pub struct Draft {
    pub doc: Document,
    path: PathBuf,
    pub dictionary: String,
    baseline: String,
    pub changed: bool,
    pub models: Vec<catalog::Choice>,
    model_id: String,
    model_revision: String,
}

impl Draft {
    pub fn open(path: PathBuf) -> Result<Self> {
        let mut draft = Self {
            doc: Document::parse("")?,
            path,
            dictionary: String::new(),
            baseline: String::new(),
            changed: false,
            models: vec![catalog::builtin()],
            model_id: profile::BUILTIN.into(),
            model_revision: String::new(),
        };
        draft.load()?;
        Ok(draft)
    }

    pub fn folder(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new("."))
    }

    /// SettingsViewModel.Load.
    pub fn load(&mut self) -> Result<()> {
        let doc = paths::load(&self.path)?;
        self.apply(doc);
        let file = self.folder().join(self.doc.get("input", "dictionary", "dictionary.tsv"));
        self.dictionary = std::fs::read_to_string(file).unwrap_or_default();
        self.baseline = self.dictionary.clone();
        self.changed = false;
        Ok(())
    }

    /// ApplyDocument followed by WriteDocument: every key the page edits is present afterwards.
    fn apply(&mut self, source: Document) {
        let w = source.get("profile", "interaction", "windows") == "windows";
        let mut doc = source.clone();
        doc.set("profile", "schema", profile::SCHEMA);
        doc.set("profile", "interaction", if w { "windows" } else { "shanjie" });
        let ranking = if source.get("engine", "ranking", "chat") == "chat" { "chat" } else { "formal" };
        doc.set("engine", "ranking", ranking);
        doc.set("engine", "model", 5);
        for (section, key, default) in FLAGS {
            doc.set(section, key, source.flag(section, key, default(w)) as u32);
        }
        for (key, _) in SHIFT_KEYS.iter().chain(PLAIN_KEYS) {
            doc.set("punctuation", key, source.flag("punctuation", key, !w) as u32);
        }
        for (section, key, default) in NUMBERS {
            doc.set(section, key, source.number(section, key, *default));
        }
        doc.set("shortcuts", "add_word_interval", source.number("shortcuts", "add_word_interval", 450).clamp(200, 1000));
        let anchor = if source.get("candidates", "anchor", "right") == "right" { "right" } else { "left" };
        doc.set("candidates", "anchor", anchor);
        let layout = if source.get("candidates", "layout", "horizontal") == "vertical" { "vertical" } else { "horizontal" };
        doc.set("candidates", "layout", layout);
        // An unknown escape or open choice stays as it is and is refused on save, as on Windows.
        for (key, values) in [("escape", ESCAPE), ("open", OPEN)] {
            let section = if key == "escape" { "input" } else { "candidates" };
            let value = source.get(section, key, values[0]).to_owned();
            doc.set(section, key, value);
        }
        self.model_id = source.model_id();
        self.model_revision = source.get("engine", "model_revision", "").to_owned();
        self.doc = doc;
        self.write_model();
    }

    fn write_model(&mut self) {
        let table = if self.model_id == "model-v6-classes-v4" { 4 } else { 5 };
        self.doc.set("engine", "class_table", table);
        self.doc.set("engine", "model_id", &self.model_id);
        self.doc.set("engine", "model_revision", &self.model_revision);
    }

    pub fn flag(&self, section: &str, key: &str) -> bool {
        self.doc.flag(section, key, false)
    }

    pub fn number(&self, section: &str, key: &str) -> u32 {
        self.doc.number(section, key, 0)
    }

    pub fn text(&self, section: &str, key: &str) -> String {
        self.doc.get(section, key, "").to_owned()
    }

    pub fn set(&mut self, section: &str, key: &str, value: impl ToString) {
        let value = value.to_string();
        if self.doc.get(section, key, "\u{0}") != value {
            self.doc.set(section, key, value);
            self.changed = true;
        }
    }

    pub fn set_dictionary(&mut self, text: String) {
        if self.dictionary != text {
            self.dictionary = text;
            self.changed = true;
        }
    }

    /// The combined 中英混打方式 choice: bit 0 mixed_input, bit 1 upstream_english.
    pub fn mixed_mode(&self) -> u32 {
        self.flag("input", "mixed_input") as u32 | (self.flag("input", "upstream_english") as u32) << 1
    }

    pub fn set_mixed_mode(&mut self, mode: u32) {
        self.set("input", "mixed_input", mode & 1);
        self.set("input", "upstream_english", (mode >> 1) & 1);
    }

    pub fn keyboard(&self) -> u32 {
        self.number("input", "keyboard")
    }

    pub fn is_zhuyin(&self) -> bool {
        self.keyboard() < 3
    }

    pub fn set_all_punctuation(&mut self, shift: bool, on: bool) {
        for (key, _) in if shift { SHIFT_KEYS } else { PLAIN_KEYS } {
            self.set("punctuation", key, on as u32);
        }
    }

    // Models (SettingsViewModel.Models.cs).

    /// The index shown in the model list; a missing package shows the built-in one but stays selected.
    pub fn model_index(&self) -> usize {
        self.models.iter().position(|m| m.id == self.model_id).unwrap_or(0)
    }

    pub fn model_available(&self) -> bool {
        self.models.iter().any(|m| m.id == self.model_id)
    }

    pub fn refresh_models(&mut self, installed: Vec<catalog::Choice>) {
        self.models = vec![catalog::builtin()];
        for model in installed {
            if model.id != profile::BUILTIN && self.models.iter().all(|m| m.id != model.id) {
                self.models.push(model);
            }
        }
        if let Some(selected) = self.models.iter().find(|m| m.id == self.model_id) {
            self.model_revision = selected.revision.clone();
            let changed = self.changed;
            self.write_model();
            self.changed = changed;
        }
    }

    /// The user picked a row of the list.
    pub fn choose_model(&mut self, index: usize) {
        if let Some(model) = self.models.get(index) {
            if model.id != self.model_id {
                self.model_id = model.id.clone();
                self.model_revision = model.revision.clone();
                self.write_model();
                self.changed = true;
            }
        }
    }

    /// A package was just downloaded: select it with a fresh revision so engines reload it.
    pub fn select_model(&mut self, id: &str) -> Result<()> {
        let model = self.models.iter().find(|m| m.id == id).ok_or("模型尚未通過驗證，無法選取。")?;
        self.model_revision = format!("{}-{:x}", model.revision, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos());
        self.model_id = id.into();
        self.write_model();
        self.changed = true;
        Ok(())
    }

    pub fn model_help(&self) -> &'static str {
        if self.model_available() {
            "儲存後先提交目前組字，再套用所選模型；個人詞庫及學習資料保留。內建配套供離線及失效備援使用。"
        } else {
            "所選配套目前不存在或未通過驗證，輸入時使用內建模型。原選擇已保留，可重新下載；選取其他模型並儲存可更換。"
        }
    }

    // Presets (SettingsViewModel.Profile.cs).

    fn load_draft(&mut self, content: &str) -> Result<()> {
        let doc = Document::parse(content)?;
        self.apply(doc);
        self.changed = true;
        Ok(())
    }

    pub fn preset(&mut self, windows: bool) -> Result<&'static str> {
        let content = profile::preset(&self.doc.content(), windows)?;
        self.load_draft(&content)?;
        Ok("已載入操作預設，按儲存後套用。詞庫、學習與外觀已保留。")
    }

    pub fn preset_folder(&self) -> PathBuf {
        self.folder().join("presets")
    }

    /// Saved presets as (display name, file).
    pub fn saved_presets(&self) -> Vec<(String, PathBuf)> {
        let Ok(entries) = std::fs::read_dir(self.preset_folder()) else { return vec![] };
        let mut files: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ini"))).collect();
        files.sort_by_key(|p| p.to_string_lossy().to_lowercase());
        files
            .into_iter()
            .filter_map(|file| {
                // A damaged file is skipped, not fatal for the settings page.
                let doc = Document::parse(&std::fs::read_to_string(&file).ok()?).ok()?;
                let stem = file.file_stem()?.to_string_lossy().into_owned();
                Some((doc.get("preset", "name", &stem).to_owned(), file))
            })
            .collect()
    }

    /// Validates someone else's preset, keeps a copy under presets/ and loads it as an unsaved draft.
    pub fn import_preset(&mut self, text: &str, fallback: &str) -> Result<String> {
        let merged = profile::import_preset(&self.doc.content(), text)?;
        let name = Document::parse(&merged)?.get("preset", "name", fallback).to_owned();
        let clean = profile::export_preset(&merged, &name)?;
        std::fs::create_dir_all(self.preset_folder())?;
        let safe: String = name.chars().map(|c| if c == '/' || c == '\0' { '_' } else { c }).collect::<String>().trim().to_owned();
        let safe = if safe.is_empty() || safe.starts_with('.') { format!("preset{safe}") } else { safe };
        let file = self.preset_folder().join(format!("{safe}.ini"));
        // A newer import of the same preset replaces the earlier copy.
        store::discard(&file)?;
        std::fs::write(file, clean)?;
        self.load_draft(&merged)?;
        Ok(format!("已匯入「{name}」，按儲存後套用。詞庫、學習、鍵盤配置與外觀已保留。"))
    }

    pub fn apply_saved_preset(&mut self, file: &Path, name: &str) -> Result<String> {
        let merged = profile::import_preset(&self.doc.content(), &std::fs::read_to_string(file)?)?;
        self.load_draft(&merged)?;
        Ok(format!("已載入「{name}」，按儲存後套用。詞庫、學習與外觀已保留。"))
    }

    pub fn export_preset(&self, name: &str) -> Result<String> {
        profile::export_preset(&self.doc.content(), name)
    }

    /// SettingsViewModel.Save.
    pub fn save(&mut self) -> Result<&'static str> {
        let font = self.number("appearance", "font_size");
        if self.keyboard() > 4
            || !ESCAPE.contains(&self.text("input", "escape").as_str())
            || !OPEN.contains(&self.text("candidates", "open").as_str())
            || !(14..=32).contains(&font)
        {
            return Err("請選擇有效的操作選項與 14–32 的候選文字大小。".into());
        }
        let interval = self.number("shortcuts", "add_word_interval");
        if self.number("shortcuts", "add_word_side") > 2 || self.number("shortcuts", "add_word_shortcut") > 1 || !(200..=1000).contains(&interval) {
            return Err("請選擇有效的加詞快捷鍵與 200–1000 毫秒間隔。".into());
        }
        let name = store::save_settings(&self.path, &self.doc.content(), &self.dictionary, &self.baseline)?;
        self.doc.set("input", "dictionary", &name);
        self.dictionary = std::fs::read_to_string(self.folder().join(&name))?;
        self.baseline = self.dictionary.clone();
        self.changed = false;
        Ok("已儲存並通知善解立即套用。切換鍵盤或詞庫時會先提交目前組字。")
    }

    pub fn draft_help(&self) -> &'static str {
        if self.changed { "尚未儲存" } else { "已載入設定" }
    }

    pub fn candidate_help(&self) -> String {
        let windows = self.text("profile", "interaction") == "windows";
        let mut help = String::from(if windows { "↑／↓ 或 ←／→ 選字。" } else { "←／→ 或 ↑／↓ 選字。" });
        help += "Tab 移到下一候選。";
        help += if self.flag("candidates", "space_operates") { "空白依下方設定操作候選。" } else { "空白不操作候選。" };
        help += if self.flag("candidates", "enter_commits") { "Enter 選字並提交整句。" } else { "Enter 選字後保留組字。" };
        help
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> PathBuf {
        std::env::set_var("SHANJIE_TEST_PROFILE", "1");
        let root = std::env::var_os("SHANJIE_TEST_ROOT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let folder = root.join(format!("shanjie-draft-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn defaults_follow_the_operation_set_and_load_is_not_a_change() {
        let folder = folder();
        std::fs::write(folder.join("profile.ini"), "[profile]\nschema=7\ninteraction=shanjie\n[input]\nkeyboard=2\n").unwrap();
        let draft = Draft::open(folder.join("profile.ini")).unwrap();
        assert!(!draft.changed);
        assert!(draft.flag("engine", "prediction"));
        assert!(!draft.flag("shortcuts", "shift_toggle"));
        assert!(draft.flag("punctuation", "shift_comma"));
        assert_eq!(draft.keyboard(), 2);
        assert_eq!(draft.number("appearance", "font_size"), 16);
        // Loading migrates, which fills missing keys from the operation set's template.
        assert_eq!(draft.text("candidates", "open"), "both");
        assert_eq!(draft.number("engine", "class_table"), 5);
    }

    #[test]
    fn save_writes_every_key_and_the_dictionary_draft() {
        let folder = folder();
        let path = folder.join("profile.ini");
        let mut draft = Draft::open(path.clone()).unwrap();
        draft.set_mixed_mode(3);
        draft.set_dictionary("ㄕㄢˋ-ㄐㄧㄝˇ 善解".into());
        assert!(draft.changed);
        draft.save().unwrap();
        assert!(!draft.changed);
        let saved = paths::load(&path).unwrap();
        assert!(saved.flag("input", "mixed_input", false) && saved.flag("input", "upstream_english", false));
        let name = saved.get("input", "dictionary", "");
        assert!(name.starts_with("dictionary-"));
        assert_eq!(std::fs::read_to_string(folder.join(name)).unwrap(), "ㄕㄢˋ-ㄐㄧㄝˇ\t善解\t-0.01\n");
        assert_eq!(draft.dictionary, "ㄕㄢˋ-ㄐㄧㄝˇ\t善解\t-0.01\n");
        // Everything the page edits is explicit in the file, as WriteDocument leaves it.
        for (section, key, _) in FLAGS {
            assert!(saved.0.get(*section).is_some_and(|s| s.contains_key(*key)), "{section}.{key}");
        }
    }

    #[test]
    fn invalid_choices_are_refused_without_writing() {
        let folder = folder();
        let path = folder.join("profile.ini");
        let mut draft = Draft::open(path.clone()).unwrap();
        draft.set("appearance", "font_size", 40);
        assert!(draft.save().is_err());
        draft.set("appearance", "font_size", 16);
        draft.set("input", "escape", "bogus");
        assert!(draft.save().is_err());
        assert!(!path.exists());
    }

    #[test]
    fn a_missing_model_stays_selected_and_a_choice_replaces_it() {
        let folder = folder();
        std::fs::write(folder.join("profile.ini"), "[profile]\nschema=7\n[engine]\nmodel_id=model-v5-classes-v3\n").unwrap();
        let mut draft = Draft::open(folder.join("profile.ini")).unwrap();
        assert_eq!((draft.model_index(), draft.model_available()), (0, false));
        assert_eq!(draft.text("engine", "model_id"), "model-v5-classes-v3");
        draft.refresh_models(vec![catalog::Choice { id: "model-v5-classes-v3".into(), name: "v5".into(), revision: "7".into() }]);
        assert_eq!((draft.model_index(), draft.model_available(), draft.changed), (1, true, false));
        assert_eq!(draft.text("engine", "model_revision"), "7");
        draft.choose_model(0);
        assert_eq!(draft.text("engine", "model_id"), profile::BUILTIN);
        assert!(draft.changed);
    }

    #[test]
    fn presets_keep_personal_choices_and_imports_are_kept() {
        let folder = folder();
        std::fs::write(folder.join("profile.ini"), "[profile]\nschema=7\n[input]\nkeyboard=2\n[appearance]\nfont_size=24\n").unwrap();
        let mut draft = Draft::open(folder.join("profile.ini")).unwrap();
        draft.preset(false).unwrap();
        assert_eq!(draft.text("profile", "interaction"), "shanjie");
        assert_eq!((draft.keyboard(), draft.number("appearance", "font_size")), (2, 24));
        let exported = draft.export_preset("我的").unwrap();
        assert!(!exported.contains("\nkeyboard=") && !exported.contains("[appearance]"));
        draft.preset(true).unwrap();
        let status = draft.import_preset(&exported, "fallback").unwrap();
        assert!(status.contains("我的"));
        assert_eq!(draft.text("profile", "interaction"), "shanjie");
        let saved = draft.saved_presets();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].0, "我的");
    }
}
