use std::collections::BTreeMap;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub const SCHEMA: u32 = 9;

/// The model bundle the package ships: always upstream's current released set, whatever it is called.
pub const BUILTIN: &str = "builtin";
/// What the built-in bundle was called before schema 8, when its id named the model.
const OLD_BUILTIN: &str = "model-v6-classes-v5";
/// `[engine] rerank_level`, in the order of the core's level codes.
pub const RERANK_LEVELS: [&str; 3] = ["conservative", "standard", "aggressive"];
/// `[engine] fuzzy_*`, in the order of shanjie_engine_set_fuzzy's bits.
pub const FUZZY_KEYS: [&str; 5] = ["fuzzy_en_eng", "fuzzy_zh_z", "fuzzy_ch_c", "fuzzy_sh_s", "fuzzy_adjacent"];
/// Chat programs for `[engine] ranking=auto` (upstream Shell.chatApps, by Windows executable name); `[engine] chat_apps`
/// replaces the list.
pub const CHAT_APPS: &str = "discord.exe;line.exe;slack.exe;telegram.exe;whatsapp.exe;whatsapp.root.exe;messenger.exe";
/// Settings that change how the core is built: a change to any of them opens a new engine.
pub const REBUILD: [(&str, &str); 26] = [
    ("input", "keyboard"),
    ("engine", "ranking"),
    ("engine", "chat_apps"),
    ("engine", "model_id"),
    ("engine", "model_revision"),
    ("engine", "acg_pack"),
    ("engine", "it_pack"),
    ("engine", "quote_pack"),
    ("engine", "rerank"),
    ("engine", "rerank_level"),
    ("engine", FUZZY_KEYS[0]),
    ("engine", FUZZY_KEYS[1]),
    ("engine", FUZZY_KEYS[2]),
    ("engine", FUZZY_KEYS[3]),
    ("engine", FUZZY_KEYS[4]),
    ("input", "english_key_order"),
    ("candidates", "anchor"),
    ("input", "learning"),
    ("input", "dictionary"),
    ("input", "context"),
    ("input", "mixed_input"),
    ("input", "upstream_english"),
    ("input", "english_overwrite"),
    ("input", "english_reversal_guard"),
    ("input", "space_inputs_zhuyin"),
    ("input", "zhuyin_only"),
];

#[derive(Clone, Default)]
pub struct Document(pub BTreeMap<String, BTreeMap<String, String>>);
impl Document {
    pub fn model_id(&self) -> String { self.get("engine", "model_id", BUILTIN).to_owned() }
    /// shanjie_engine_set_profile: 0 chat, 1 formal. `auto` (upstream profile-choice section 2.2): the chat profile in a
    /// chat program, formal elsewhere and when the program is unknown.
    pub fn ranking(&self, app: Option<&str>) -> u32 {
        match self.get("engine", "ranking", "auto") {
            "chat" => 0,
            "formal" => 1,
            _ => {
                let chat = app.filter(|a| !a.is_empty()).is_some_and(|app| {
                    self.get("engine", "chat_apps", CHAT_APPS).split([';', ',', ' ', '\n']).any(|name| name.eq_ignore_ascii_case(app))
                });
                (!chat) as u32
            }
        }
    }
    /// The raw-key mixing mode (`[input] mixed_input`): takes effect on Zhuyin keyboards only, whatever the file says.
    pub fn raw_key_mixing(&self) -> bool {
        self.number("input", "keyboard", 0) < 3 && self.flag("input", "mixed_input", false)
    }
    /// Pure Zhuyin is a Windows typing mode; Pinyin keeps its usual converter.
    pub fn zhuyin_only(&self) -> bool {
        self.number("input", "keyboard", 0) < 3 && self.flag("input", "zhuyin_only", false)
    }
    /// Whether the reading panel shows. Raw-key mixing keeps keys in the composition and its Zhuyin hint in the panel, so it
    /// forces the panel on at read time; the stored `[candidates] reading_panel` is left alone for when the mode is off.
    pub fn reading_panel(&self) -> bool {
        self.raw_key_mixing() || self.flag("candidates", "reading_panel", false)
    }
    pub fn fuzzy(&self) -> u32 {
        FUZZY_KEYS.iter().enumerate().fold(0, |flags, (bit, key)| flags | (self.flag("engine", key, false) as u32) << bit)
    }
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
        // The candidate window is vertical only on Windows: a line left in an old profile is dropped unread.
        // The Linux frontends still read `layout`.
        if cfg!(windows) {
            if let Some(candidates) = out.0.get_mut("candidates") {
                candidates.remove("layout");
            }
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
        // A future reader must never turn a newer format back into this one.
        if schema > SCHEMA {
            return;
        }
        let windows = self.get("profile", "interaction", "windows") == "windows";
        if self.get("candidates", "right_action", "").is_empty() {
            let action = if self.flag("candidates", "right_pages", false) { "page" } else { "none" };
            self.set("candidates", "right_action", action);
        }
        self.remove("candidates", "right_pages");
        if schema < 4
            && !self
                .0
                .get("appearance")
                .is_some_and(|s| s.contains_key("font_size"))
        {
            self.set("appearance", "font_size", 18);
        }
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
        }
        if schema < 6 {
            // One switch used to cover every mark of a row (e.g. 「」 also turned \ into 、).
            for (old, prefix, names) in [
                ("shift_punctuation", "shift", SHIFT_KEYS.map(|(_, n)| n).to_vec()),
                ("plain_punctuation", "plain", PLAIN_KEYS.map(|(_, n)| n).to_vec()),
            ] {
                if let Some(value) = self.0.get("input").and_then(|s| s.get(old)).cloned() {
                    for n in names {
                        let key = format!("{prefix}_{n}");
                        if !self.0.get("punctuation").is_some_and(|s| s.contains_key(&key)) {
                            self.set("punctuation", &key, &value);
                        }
                    }
                }
                self.remove("input", old);
            }
        }
        // Missing values in older profiles preserve their pre-update behavior.
        if schema<7 && self.get("candidates","enter_to_cursor","").is_empty(){self.set("candidates","enter_to_cursor",0);}
        if schema < 8 {
            // The class table switch chose between two built-in pairs; the old v4 pair lives on as a user package.
            let id = match self.get("engine", "model_id", "") {
                "" if self.get("engine", "class_table", "") == "4" => "model-v6-classes-v4",
                "" | OLD_BUILTIN => BUILTIN,
                id => id,
            }
            .to_owned();
            self.set("engine", "model_id", id);
            // Earlier versions always kept an unfinished reading behind punctuation; new profiles drop it as upstream.
            if self.get("input", "punctuation_keeps_reading", "").is_empty() {
                self.set("input", "punctuation_keeps_reading", 1);
            }
            self.remove("engine", "class_table");
            self.remove("engine", "model");
        }
        // Legacy INI files do not distinguish the old default from a manual opt-out.
        // Align Windows once; an explicit opt-out in schema 9 and later is retained.
        if schema < 9 && windows {
            self.set("shortcuts", "backquote_symbols", 1);
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
        // The two mixing keys are one choice (中英混打方式): off, upstream English or raw-key mixing. Raw-key mixing wins:
        // both on (兩者並用, 0.9.0 to 0.15.x) is no longer offered and raw-key mixing judged first there, and a profile
        // without the upstream key (older than it, or edited by hand) must not take the template's 1 beside it.
        if self.flag("input", "mixed_input", false) {
            self.set("input", "upstream_english", 0);
        }
        self.set("profile", "schema", SCHEMA);
        if cfg!(not(windows)) && !matches!(self.get("candidates", "layout", ""), "horizontal" | "vertical") {
            self.set("candidates", "layout", "horizontal");
        }
        // `reading` is an old switch from before the panel merged with the candidates; `reading_panel` is not it.
        for key in [
            "big5",
            "reading",
            "tab_expands",
            "right_expands",
            "grid_up_collapses",
        ] {
            self.remove("candidates", key);
        }
        self.remove("input", "escape_clears");
        self.remove("input", "smart_zhuyin");
    }
}
/// Same order as the core's SHIFT_PUNCT; one `[punctuation] shift_<name>` switch each.
pub const SHIFT_KEYS: [(char, &str); 13] = [
    (',', "comma"),
    ('.', "period"),
    ('/', "slash"),
    ('1', "1"),
    (';', "semicolon"),
    ('[', "lbracket"),
    (']', "rbracket"),
    ('9', "9"),
    ('0', "0"),
    ('`', "backquote"),
    ('\\', "backslash"),
    ('\'', "quote"),
    ('=', "equal"),
];
/// Same order as the core's PLAIN_PUNCT; one `[punctuation] plain_<name>` switch each.
pub const PLAIN_KEYS: [(char, &str); 6] = [
    ('[', "lbracket"),
    (']', "rbracket"),
    ('\\', "backslash"),
    ('\'', "quote"),
    ('=', "equal"),
    ('`', "backquote"),
];
impl Document {
    pub fn shift_punctuation(&self, ch: char) -> bool {
        SHIFT_KEYS.iter().any(|(c, n)| *c == ch && self.flag("punctuation", &format!("shift_{n}"), false))
    }
    /// Core option bits: standalone tones, then each Shift and plain punctuation switch.
    pub fn input_options(&self) -> u32 {
        let mut flags = self.flag("input", "standalone_tones", false) as u32;
        for (i, (_, n)) in SHIFT_KEYS.iter().enumerate() {
            flags |= (self.flag("punctuation", &format!("shift_{n}"), false) as u32) << (1 + i);
        }
        for (i, (_, n)) in PLAIN_KEYS.iter().enumerate() {
            flags |= (self.flag("punctuation", &format!("plain_{n}"), false) as u32) << (14 + i);
        }
        flags |= (!self.flag("input", "space_inputs_zhuyin", true) as u32) << 20;
        let windows = self.get("profile", "interaction", "windows") != "shanjie";
        flags |= (self.flag("candidates", "enter_to_cursor", windows) as u32) << 21;
        flags |= (self.flag("input", "punctuation_keeps_reading", false) as u32) << 22;
        flags |= (self.zhuyin_only() as u32) << 25;
        flags
    }
}
pub fn dictionary_content(text: &str) -> Result<String> {
    let mut content = String::new();
    // WinUI TextBox returns CR; accept it alongside LF and Windows CRLF.
    for line in text
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|l| !l.is_empty())
    {
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
    Ok(content)
}
/// Personal choices a preset never carries: they stay with the user when loading one.
const PERSONAL: [(&str, &[&str]); 3] = [
    ("engine", &["ranking", "chat_apps", "demote", "acg_pack", "quote_pack", "it_pack", "model_id", "model_revision", "rerank", "rerank_level",
        "fuzzy_en_eng", "fuzzy_zh_z", "fuzzy_ch_c", "fuzzy_sh_s", "fuzzy_adjacent"]),
    (
        "input",
        &["dictionary", "keyboard", "learning", "context", "chinese", "full_width", "cjk_spacing"],
    ),
    ("appearance", &["theme", "font_size", "mode_indicator", "glass_tint", "accent", "accent_color", "pure_black", "animation"]),
];
const CHOICES: [(&str, &str, &[&str]); 8] = [
    ("profile", "interaction", &["windows", "shanjie"]),
    ("engine", "ranking", &["auto", "chat", "formal"]),
    ("engine", "rerank_level", &RERANK_LEVELS),
    ("input", "escape", &["staged", "clear", "ignore"]),
    ("candidates", "layout", &["horizontal", "vertical"]),
    ("candidates", "anchor", &["right", "left"]),
    ("candidates", "open", &["down", "space", "both"]),
    ("candidates", "right_action", &["none", "page", "expand"]),
];
/// A preset over the current profile: the personal keys stay the user's.
pub fn merge_preset(previous: &Document, mut preset: Document) -> String {
    for (section, keys) in PERSONAL {
        for key in keys {
            match previous.0.get(section).and_then(|s| s.get(*key)) {
                Some(value) => preset.set(section, key, value),
                None => preset.remove(section, key),
            }
        }
    }
    preset.migrate();
    preset.content()
}
pub fn preset(content: &str, windows: bool) -> Result<String> {
    let preset = Document::parse(if windows {
        include_str!("../../profiles/windows.ini")
    } else {
        include_str!("../../profiles/shanjie.ini")
    })?;
    Ok(merge_preset(&Document::parse(content)?, preset))
}
/// Validates someone else's preset file: only known operation keys with valid values survive.
fn shared_preset(text: &str) -> Result<Document> {
    let mut source = Document::parse(text)?;
    source.migrate();
    let windows = source.get("profile", "interaction", "windows") == "windows";
    let defaults = Document::parse(if windows {
        include_str!("../../profiles/windows.ini")
    } else {
        include_str!("../../profiles/shanjie.ini")
    })?;
    let mut out = defaults.clone();
    for (section, fields) in &defaults.0 {
        for (key, default) in fields {
            if PERSONAL.iter().any(|(s, k)| s == section && k.contains(&key.as_str())) {
                continue;
            }
            let Some(value) = source.0.get(section).and_then(|s| s.get(key)) else {
                continue;
            };
            let valid = match CHOICES.iter().find(|(s, k, _)| s == section && k == key) {
                Some((_, _, allowed)) => allowed.contains(&value.as_str()),
                None if section == "shortcuts" && key == "add_word_interval" => value.parse::<u32>().is_ok_and(|v| (200..=1000).contains(&v)),
                None if section == "shortcuts" && (key == "shift_side" || key == "add_word_side") => value.parse::<u32>().is_ok_and(|v| v <= 2),
                None if default == "0" || default == "1" => value == "0" || value == "1",
                None => default.parse::<u32>().is_ok() && value.parse::<u8>().is_ok(),
            };
            if !valid {
                return Err(format!("操作預設的 [{section}] {key}={value} 無效").into());
            }
            out.set(section, key, value);
        }
    }
    if let Some(name) = source.0.get("preset").and_then(|s| s.get("name")) {
        out.set("preset", "name", name.chars().filter(|c| !c.is_control()).take(40).collect::<String>());
    }
    Ok(out)
}
/// Loads a shared preset into the current profile while keeping personal choices.
pub fn import_preset(content: &str, preset: &str) -> Result<String> {
    Ok(merge_preset(&Document::parse(content)?, shared_preset(preset)?))
}
/// Writes only the operation keys, so a shared file never carries dictionaries or looks.
pub fn export_preset(content: &str, name: &str) -> Result<String> {
    let mut doc = shared_preset(content)?;
    for (section, keys) in PERSONAL {
        for key in keys {
            doc.remove(section, key);
        }
    }
    doc.0.remove("appearance");
    doc.set("preset", "name", name.chars().filter(|c| !c.is_control()).take(40).collect::<String>());
    Ok(doc.content())
}
#[cfg(test)]
mod tests {
    #[test]
    fn ranking_follows_the_program_when_automatic() {
        let doc = super::Document::parse("[engine]\nranking=auto\n").unwrap();
        assert_eq!((doc.ranking(Some("Discord.exe")), doc.ranking(Some("WINWORD.EXE")), doc.ranking(None)), (0, 1, 1));
        let doc = super::Document::parse("[engine]\nranking=auto\nchat_apps=winword.exe; notepad.exe\n").unwrap();
        assert_eq!((doc.ranking(Some("WINWORD.EXE")), doc.ranking(Some("Notepad.exe")), doc.ranking(Some("discord.exe"))), (0, 0, 1));
        for (fixed, want) in [("chat", 0), ("formal", 1)] {
            let doc = super::Document::parse(&format!("[engine]\nranking={fixed}\n")).unwrap();
            assert_eq!((doc.ranking(Some("discord.exe")), doc.ranking(None)), (want, want));
        }
    }
    #[test]
    fn legacy_right_pages_migrates_and_presets_validate_the_three_actions() {
        for (legacy, action) in [(0, "none"), (1, "page")] {
            let mut doc = super::Document::parse(&format!("[candidates]\nright_pages={legacy}\n")).unwrap();
            doc.migrate();
            assert_eq!(doc.get("candidates", "right_action", ""), action);
            assert!(!doc.0["candidates"].contains_key("right_pages"));
            doc.set("candidates", "right_action", "expand");doc.set("candidates", "right_pages", 0);doc.migrate();
            assert_eq!(doc.get("candidates", "right_action", ""), "expand");
        }
        for action in ["none", "page", "expand"] {
            let imported = super::import_preset("", &format!("[candidates]\nright_action={action}\n")).unwrap();
            assert_eq!(super::Document::parse(&imported).unwrap().get("candidates", "right_action", ""), action);
        }
        assert!(super::import_preset("", "[candidates]\nright_action=table\n").is_err());
    }
    #[test]
    fn presets_keep_the_personal_engine_choices() {
        assert!(super::FUZZY_KEYS.iter().all(|k| super::PERSONAL[0].1.contains(k)));
        let mine = "[profile]\nschema=8\n[engine]\nrerank=0\nrerank_level=aggressive\nfuzzy_zh_z=1\n[input]\ncjk_spacing=1\n";
        let doc = super::Document::parse(&super::preset(mine, false).unwrap()).unwrap();
        assert!(!doc.flag("engine", "rerank", true) && doc.flag("engine", "fuzzy_zh_z", false) && doc.flag("input", "cjk_spacing", false));
        assert_eq!(doc.get("engine", "rerank_level", ""), "aggressive");
    }
    #[test]
    fn model_selection_and_enter_migration_preserve_explicit_choices() {
        let mut old=super::Document::parse("[profile]\nschema=6\ninteraction=windows\n[engine]\nclass_table=4\n").unwrap();
        old.migrate();assert!(!old.flag("candidates","enter_to_cursor",true));assert_eq!(old.model_id(),"model-v6-classes-v4");
        assert!(!old.0["engine"].contains_key("class_table"));
        for builtin in ["", "model_id=model-v6-classes-v5\n"] {
            let mut doc=super::Document::parse(&format!("[profile]\nschema=7\n[engine]\n{builtin}")).unwrap();
            doc.migrate();assert_eq!(doc.model_id(),super::BUILTIN,"the old built-in follows the package");
        }
        old.set("candidates","enter_to_cursor",1);old.set("engine","model_id","model-v5-classes-v3");old.migrate();
        assert!(old.flag("candidates","enter_to_cursor",false));
        let preset=super::preset(&old.content(),true).unwrap();
        assert_eq!(super::Document::parse(&preset).unwrap().model_id(),"model-v5-classes-v3");
        assert!(!super::export_preset(&old.content(),"public").unwrap().contains("model_id="));
    }
    #[test]
    fn backquote_migration_aligns_legacy_windows_once() {
        for (schema, interaction, enabled) in [(8, "windows", true), (9, "windows", false), (8, "shanjie", false)] {
            let mut doc = super::Document::parse(&format!("[profile]\nschema={schema}\ninteraction={interaction}\n[shortcuts]\nbackquote_symbols=0\n")).unwrap();
            doc.migrate();
            assert_eq!(doc.flag("shortcuts", "backquote_symbols", !enabled), enabled);
            doc.set("shortcuts", "backquote_symbols", 0); doc.migrate();
            assert!(!doc.flag("shortcuts", "backquote_symbols", true));
        }
    }
    use super::*;
    fn template(windows: bool) -> Document {
        Document::parse(if windows { include_str!("../../profiles/windows.ini") } else { include_str!("../../profiles/shanjie.ini") }).unwrap()
    }
    #[test]
    fn templates_default_to_upstream_mixing_without_the_reading_panel() {
        for windows in [true, false] {
            let doc = template(windows);
            assert!(doc.flag("input", "upstream_english", false) && !doc.flag("input", "mixed_input", true));
            assert!(!doc.flag("candidates", "reading_panel", true) && !doc.reading_panel());
        }
    }
    #[test]
    fn the_mixing_mode_is_one_choice_and_both_become_raw_key_mixing() {
        let mode = |doc: &Document| (doc.flag("input", "mixed_input", false), doc.flag("input", "upstream_english", false));
        for interaction in ["windows", "shanjie"] {
            // Raw-key mixing from before upstream English existed (schema 6, v0.8), or a hand-edited newer file.
            for schema in [6, 9] {
                let old = format!("[profile]\nschema={schema}\ninteraction={interaction}\n[input]\nmixed_input=1\n");
                let mut doc = Document::parse(&old).unwrap();
                doc.migrate();
                assert_eq!(mode(&doc), (true, false), "{interaction} schema {schema}");
                doc.migrate();
                assert_eq!(mode(&doc), (true, false), "stays after another migration");
                // A shared preset written that way keeps its single mode too.
                assert_eq!(mode(&Document::parse(&import_preset("", &old).unwrap()).unwrap()), (true, false));
            }
            let mut upstream = Document::parse(&format!("[profile]\nschema=9\ninteraction={interaction}\n[input]\nupstream_english=1\n")).unwrap();
            upstream.migrate();
            assert_eq!(mode(&upstream), (false, true));
            // Both on (兩者並用, no longer offered) becomes raw-key mixing, which judged first, whatever the schema; it stays so,
            // and a shared preset or a template merge written with both comes in the same way.
            for schema in [6, 9] {
                let old = format!("[profile]\nschema={schema}\ninteraction={interaction}\n[input]\nmixed_input=1\nupstream_english=1\n");
                let mut both = Document::parse(&old).unwrap();
                both.migrate();
                assert_eq!(mode(&both), (true, false), "{interaction} schema {schema} both");
                both.migrate();
                assert_eq!(mode(&both), (true, false), "both stays raw-key mixing");
                assert_eq!(mode(&Document::parse(&import_preset("", &old).unwrap()).unwrap()), (true, false));
                assert_eq!(mode(&Document::parse(&merge_preset(&Document::default(), Document::parse(&old).unwrap())).unwrap()), (true, false));
            }
            // An off key beside a missing one still takes the template's default.
            let mut off = Document::parse(&format!("[profile]\nschema=9\ninteraction={interaction}\n[input]\nmixed_input=0\n")).unwrap();
            off.migrate();
            assert_eq!(mode(&off), mode(&template(interaction == "windows")));
        }
    }
    #[test]
    fn shanjie_template_follows_upstream_defaults() {
        let doc = template(false);
        for (section, key) in [("candidates", "space_operates"), ("shortcuts", "shift_toggle"), ("input", "learning"),
            ("input", "caps_lock_english"), ("shortcuts", "forget_candidate")] {
            assert!(doc.flag(section, key, false), "{key}");
        }
        assert_eq!(doc.get("candidates", "open", ""), "both");
        assert!(doc.flag("candidates", "space_pages", false));
    }
    #[test]
    fn windows_template_follows_microsoft_for_the_candidate_list_and_keeps_explicit_values() {
        let doc = template(true);
        assert_eq!(doc.get("candidates", "right_action", ""), "expand");
        let shanjie = template(false);
        assert_eq!(shanjie.get("candidates", "right_action", ""), "none");
        let mut mine = Document::parse("[profile]\nschema=9\ninteraction=windows\n[candidates]\nright_action=none\n").unwrap();
        mine.migrate();
        assert_eq!(mine.get("candidates", "right_action", ""), "none");
        // A profile from before right_action keeps its old → (the legacy mapping, not the template).
        let mut legacy = Document::parse("[profile]\nschema=5\ninteraction=windows\n").unwrap();
        legacy.migrate();
        assert_eq!(legacy.get("candidates", "right_action", ""), "none");
    }
    #[test]
    fn stale_big5_values_load_harmlessly() {
        let stale = "[profile]\nschema=9\ninteraction=windows\n[candidates]\nbig5=1\nanchor=left\n";
        let mut doc = Document::parse(stale).unwrap();
        doc.migrate();
        assert!(!doc.0["candidates"].contains_key("big5"));
        assert_eq!(doc.get("candidates", "anchor", ""), "left");
        assert!(!preset(stale, true).unwrap().contains("big5"));
        assert!(!import_preset(stale, stale).unwrap().contains("big5"));
    }
    #[test]
    fn stale_layout_lines_load_harmlessly_on_windows() {
        // Any old value, even one that is not valid, goes without touching the keys around it.
        for value in ["horizontal", "vertical", "sideways"] {
            let stale = format!("[profile]\nschema=9\ninteraction=windows\n[candidates]\nlayout={value}\nanchor=left\n[appearance]\nglass_tint=70\n");
            let mut doc = Document::parse(&stale).unwrap();
            doc.migrate();
            assert_eq!(doc.get("candidates", "anchor", ""), "left");
            assert_eq!(doc.number("appearance", "glass_tint", 0), 70);
            if cfg!(windows) {
                assert!(!doc.0["candidates"].contains_key("layout"));
                assert!(!template(true).0["candidates"].contains_key("layout"));
                assert!(!template(false).0["candidates"].contains_key("layout"));
                assert!(!import_preset(&stale, &stale).unwrap().contains("layout"));
                assert!(!preset(&stale, false).unwrap().contains("layout"));
            }
        }
    }
    #[test]
    fn raw_key_mixing_forces_the_reading_panel_without_rewriting_it() {
        let doc = |keyboard, mixed, upstream, panel| {
            let mut doc = Document::parse(&format!("[profile]\nschema=9\ninteraction=windows\n[input]\nkeyboard={keyboard}\nmixed_input={mixed}\nupstream_english={upstream}\n[candidates]\nreading_panel={panel}\n")).unwrap();
            doc.migrate();
            doc
        };
        // Off, upstream only, then the user's own choice.
        assert!(!doc(0, 0, 0, 0).reading_panel() && !doc(0, 0, 1, 0).reading_panel());
        assert!(doc(0, 0, 1, 1).reading_panel());
        // Raw-key mixing on every Zhuyin layout, also from a profile that had upstream's on beside it; the stored value stays 0.
        for (keyboard, upstream) in [(0, 0), (0, 1), (1, 0), (2, 0)] {
            let forced = doc(keyboard, 1, upstream, 0);
            assert!(forced.reading_panel() && !forced.flag("candidates", "reading_panel", true));
        }
        // Pinyin keyboards never run the mode, so nothing forces the panel.
        assert!(!doc(3, 1, 0, 0).raw_key_mixing() && !doc(3, 1, 0, 0).reading_panel());
    }
    #[test]
    fn migration_keeps_the_new_panel_key_and_explicit_values() {
        // A profile from before the setting: the key is added off, a stale `reading` is still dropped, explicit values stay.
        let mut doc = Document::parse("[profile]\nschema=9\ninteraction=shanjie\n[input]\nlearning=0\nmixed_input=0\nupstream_english=0\ncaps_lock_english=0\n[candidates]\nreading=1\nspace_operates=0\n[shortcuts]\nshift_toggle=0\nforget_candidate=0\n").unwrap();
        doc.migrate();
        assert!(!doc.0["candidates"].contains_key("reading"));
        assert_eq!(doc.0["candidates"].get("reading_panel").map(String::as_str), Some("0"));
        for (section, key) in [("input", "learning"), ("input", "upstream_english"), ("input", "caps_lock_english"),
            ("candidates", "space_operates"), ("shortcuts", "shift_toggle"), ("shortcuts", "forget_candidate")] {
            assert!(!doc.flag(section, key, true), "{key}");
        }
        // Turned on, it survives further migrations and a shared preset.
        doc.set("candidates", "reading_panel", 1);
        doc.migrate();
        assert!(doc.reading_panel());
        let shared = export_preset(&doc.content(), "panel").unwrap();
        assert!(Document::parse(&import_preset("", &shared).unwrap()).unwrap().reading_panel());
        // Missing keys take the new defaults of the profile's own template.
        let mut empty = Document::parse("[profile]\nschema=9\ninteraction=windows\n").unwrap();
        empty.migrate();
        assert!(empty.flag("input", "upstream_english", false) && !empty.reading_panel());
    }
    #[test]
    fn the_panel_without_the_reading_row_closes_on_composition_alone() {
        let state = |candidates: &[&str], reading_hint| crate::protocol::Snapshot {
            preedit: "gj".into(),
            cursor: 2,
            candidates: candidates.iter().map(|c| c.to_string()).collect(),
            reading_hint,
            ..Default::default()
        };
        let closed = crate::input::panel_view(state(&[], false), false);
        assert!(closed.preedit.is_empty() && closed.candidates.is_empty(), "nothing left to draw: the panel closes");
        let listed = crate::input::panel_view(state(&["好"], false), false);
        assert!(listed.preedit.is_empty() && listed.candidates.len() == 1, "candidates without the sentence row");
        assert_eq!(crate::input::panel_view(state(&["ㄕㄨ", "gj"], true), false).preedit, "gj", "a hint always draws");
        let on = crate::input::panel_view(state(&[], false), true);
        assert!(on.preedit == "gj" && on.cursor == 2);
    }
    #[test]
    fn dictionary_newlines_preserve_rows_and_scores() {
        let rows = ["ㄋㄧˇ  你", "ㄏㄠˇ\t好\t-1.25"];
        let expected = "ㄋㄧˇ\t你\t-0.01\nㄏㄠˇ\t好\t-1.25\n";
        for newline in ["\n", "\r\n", "\r"] {
            let input = format!(
                "{newline} {}{newline}{newline}{} {newline}",
                rows[0], rows[1]
            );
            assert_eq!(dictionary_content(&input).unwrap(), expected);
            assert_eq!(
                dictionary_content(&expected.replace('\n', newline)).unwrap(),
                expected
            );
        }
        assert_eq!(
            dictionary_content("ㄋㄧˇ 你\rㄏㄠˇ 好\nㄕㄢˋ-ㄐㄧㄝˇ 善解\r\n")
                .unwrap()
                .lines()
                .count(),
            3
        );
    }
    #[test]
    fn dictionary_newlines_do_not_relax_validation() {
        for invalid in [
            "ㄋㄧˇ 你好",
            "invalid 你",
            "ㄋㄧˇ 你 0.5",
            "ㄋㄧˇ 你 NaN",
            "ㄋㄧˇ 你 -inf",
            "ㄋㄧˇ 你 -0.01 extra",
        ] {
            for newline in ["\n", "\r\n", "\r"] {
                assert!(dictionary_content(&format!("ㄏㄠˇ 好{newline}{invalid}")).is_err());
            }
        }
    }
    #[test]
    fn shared_presets_keep_personal_choices_and_reject_bad_values() {
        let mine = "[profile]\nschema=6\ninteraction=windows\n[input]\ndictionary=mine.tsv\nkeyboard=2\n[appearance]\nfont_size=24\n";
        let source = "[profile]\nschema=6\ninteraction=windows\n[input]\ndictionary=../evil.tsv\nkeyboard=1\n[punctuation]\nplain_lbracket=1\n[candidates]\nanchor=left\n";
        let shared = export_preset(source, "朋友\n").unwrap();
        assert!(!shared.contains("dictionary") && !shared.contains("\nkeyboard=") && !shared.contains("[appearance]"));
        let merged = Document::parse(&import_preset(mine, &shared).unwrap()).unwrap();
        assert_eq!(merged.get("input", "dictionary", ""), "mine.tsv");
        assert_eq!(merged.number("input", "keyboard", 0), 2);
        assert_eq!(merged.number("appearance", "font_size", 0), 24);
        assert!(merged.flag("punctuation", "plain_lbracket", false));
        assert_eq!(merged.get("candidates", "anchor", ""), "left");
        assert_eq!(merged.get("preset", "name", ""), "朋友");
        for bad in [
            // Legacy values (e.g. layout=grid) are migrated first; unknown ones are refused.
            "[candidates]\nanchor=middle\n",
            "[input]\nescape_x=1\nmixed_input=2\n",
            "[shortcuts]\nshift_side=x\n",
        ] {
            assert!(import_preset(mine, bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn schema5_punctuation_rows_split_into_per_key_switches() {
        let mut doc = Document::parse("[profile]\nschema=5\ninteraction=windows\n[input]\nshift_punctuation=1\nplain_punctuation=0\nstandalone_tones=1\n").unwrap();
        doc.migrate();
        assert!(!doc.0["input"].contains_key("shift_punctuation"));
        assert!(!doc.0["input"].contains_key("plain_punctuation"));
        assert!(SHIFT_KEYS.iter().all(|(c, _)| doc.shift_punctuation(*c)));
        // Existing profiles retain whole-line Enter and the kept unfinished reading.
        assert_eq!(doc.input_options(), 1 | 0x1fff << 1 | 1 << 22);
        // Each mark is now independent: 「 on, 、 (backslash) still off.
        doc.set("punctuation", "plain_lbracket", 1);
        assert_eq!(doc.input_options() >> 14 & 0x3f, 1);
        assert!(!doc.shift_punctuation('a'));
    }
    #[test]
    fn zhuyin_only_defaults_and_preset_round_trip() {
        assert!(REBUILD.contains(&("input", "zhuyin_only")));
        for preset in [include_str!("../../profiles/windows.ini"), include_str!("../../profiles/shanjie.ini")] {
            let mut doc = Document::parse(preset).unwrap();
            assert!(!doc.zhuyin_only());
            doc.set("input", "zhuyin_only", 1);
            let exported = export_preset(&doc.content(), "純注音").unwrap();
            let imported = import_preset("[profile]\nschema=9\n", &exported).unwrap();
            let mut doc = Document::parse(&imported).unwrap();
            for keyboard in 0..=4 {
                doc.set("input", "keyboard", keyboard);
                assert_eq!(doc.zhuyin_only(), keyboard < 3);
                assert_eq!(doc.input_options() & (1 << 25) != 0, keyboard < 3);
                assert!(doc.flag("input", "zhuyin_only", false));
            }
            assert!(import_preset(preset, "[input]\nzhuyin_only=2\n").is_err());
        }
        let mut old = Document::parse("[profile]\nschema=8\n").unwrap();
        old.migrate();
        assert!(!old.zhuyin_only());
    }
    #[test]
    fn space_symbol_toggle_defaults_and_preset_round_trip() {
        let mut old = Document::parse("[profile]\nschema=6\ninteraction=windows\n").unwrap();
        old.migrate();
        assert!(old.flag("input", "space_inputs_zhuyin", false));
        assert_eq!(old.input_options() & (1 << 20),0);
        assert_ne!(old.input_options() & (1 << 22),0,"an existing profile keeps its punctuation behavior");
        for preset in [include_str!("../../profiles/windows.ini"), include_str!("../../profiles/shanjie.ini")] {
            assert_eq!(Document::parse(preset).unwrap().input_options() & (1 << 22),0,"new profiles follow upstream");
        }
        old.set("input", "space_inputs_zhuyin",0);
        let shared = export_preset(&old.content(),"注音候選").unwrap();
        let imported = Document::parse(&import_preset("[profile]\nschema=6\n",&shared).unwrap()).unwrap();
        assert!(!imported.flag("input", "space_inputs_zhuyin",true));
        assert_ne!(imported.input_options() & (1 << 20),0);
        assert!(import_preset(&old.content(),"[input]\nspace_inputs_zhuyin=2\n").is_err());
    }
    #[test]
    fn migration_preserves_personal_data_and_removes_retired_choices() {
        let mut doc=Document::parse("[profile]\nschema=2\ninteraction=windows\n[engine]\nmodel=1\n[input]\nlearning=1\ndictionary=custom.tsv\nescape_clears=0\n[candidates]\nlayout=grid\ntab_expands=1\n[appearance]\nfont_size=22\n").unwrap();
        doc.migrate();
        doc.set("input", "smart_zhuyin", 1);
        doc.migrate();
        assert!(!doc.0["input"].contains_key("smart_zhuyin"));
        assert!(!doc.0["engine"].contains_key("model"));
        assert_eq!(doc.get("input", "dictionary", ""), "custom.tsv");
        assert!(doc.flag("input", "learning", false));
        assert_eq!(doc.number("appearance", "font_size", 0), 22);
        assert_eq!(doc.get("input", "escape", ""), "ignore");
        assert!(!doc.0["candidates"].contains_key("tab_expands"));
        assert!(!doc.flag("input", "space_commits", true));
        assert!(!doc.flag("candidates", "space_operates", true));
        doc.set("input", "space_commits", 1);
        doc.set("candidates", "space_operates", 1);
        doc.migrate();
        assert!(doc.flag("input", "space_commits", false));
        assert!(doc.flag("candidates", "space_operates", false));
        let preset = Document::parse(&preset(&doc.content(), false).unwrap()).unwrap();
        assert_eq!(preset.get("input", "dictionary", ""), "custom.tsv");
        assert!(preset.flag("input", "learning", false));
        assert_eq!(preset.number("appearance", "font_size", 0), 22);
    }
}
