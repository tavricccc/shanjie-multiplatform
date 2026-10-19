use std::collections::BTreeMap;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub const SCHEMA: u32 = 7;

pub const BUILTIN: &str = "model-v6-classes-v5";

#[derive(Clone, Default)]
pub struct Document(pub BTreeMap<String, BTreeMap<String, String>>);
impl Document {
    pub fn model_id(&self)->String {self.get("engine","model_id",if self.class_table()==4 {"model-v6-classes-v4"}else{BUILTIN}).to_owned()}
    pub fn class_table(&self) -> u32 { if self.number("engine","class_table",5)==4 {4} else {5} }
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
        // A future reader must never turn a newer format back into this one.
        if schema > SCHEMA {
            return;
        }
        let windows = self.get("profile", "interaction", "windows") == "windows";
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
        if self.get("engine","model_id","").is_empty(){let id=self.model_id();self.set("engine","model_id",id);}
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
        self.set("profile", "schema", SCHEMA);
        self.set("engine", "model", 5);
        if !matches!(self.get("candidates", "layout", ""), "horizontal" | "vertical") {
            self.set("candidates", "layout", "horizontal");
        }
        for key in [
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
const PERSONAL: [(&str, &[&str]); 4] = [
    ("engine", &["ranking", "demote", "acg_pack", "class_table", "model_id", "model_revision"]),
    (
        "input",
        &["dictionary", "keyboard", "learning", "context", "chinese", "full_width"],
    ),
    ("candidates", &["big5"]),
    ("appearance", &["theme", "font_size", "mode_indicator"]),
];
const CHOICES: [(&str, &str, &[&str]); 6] = [
    ("profile", "interaction", &["windows", "shanjie"]),
    ("engine", "ranking", &["chat", "formal"]),
    ("input", "escape", &["staged", "clear", "ignore"]),
    ("candidates", "layout", &["horizontal", "vertical"]),
    ("candidates", "anchor", &["right", "left"]),
    ("candidates", "open", &["down", "space", "both"]),
];
fn merge_preset(previous: &Document, mut preset: Document) -> String {
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
    fn model_selection_and_enter_migration_preserve_explicit_choices() {
        let mut old=super::Document::parse("[profile]\nschema=6\ninteraction=windows\n[engine]\nclass_table=4\n").unwrap();
        old.migrate();assert!(!old.flag("candidates","enter_to_cursor",true));assert_eq!(old.model_id(),"model-v6-classes-v4");
        old.set("candidates","enter_to_cursor",1);old.set("engine","model_id","model-v5-classes-v3");old.migrate();
        assert!(old.flag("candidates","enter_to_cursor",false));
        let preset=super::preset(&old.content(),true).unwrap();
        assert_eq!(super::Document::parse(&preset).unwrap().model_id(),"model-v5-classes-v3");
        assert!(!super::export_preset(&old.content(),"public").unwrap().contains("model_id="));
    }
    use super::*;
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
        let source = "[profile]\nschema=6\ninteraction=windows\n[input]\ndictionary=../evil.tsv\nkeyboard=1\n[punctuation]\nplain_lbracket=1\n[candidates]\nlayout=vertical\n";
        let shared = export_preset(source, "朋友\n").unwrap();
        assert!(!shared.contains("dictionary") && !shared.contains("\nkeyboard=") && !shared.contains("[appearance]"));
        let merged = Document::parse(&import_preset(mine, &shared).unwrap()).unwrap();
        assert_eq!(merged.get("input", "dictionary", ""), "mine.tsv");
        assert_eq!(merged.number("input", "keyboard", 0), 2);
        assert_eq!(merged.number("appearance", "font_size", 0), 24);
        assert!(merged.flag("punctuation", "plain_lbracket", false));
        assert_eq!(merged.get("candidates", "layout", ""), "vertical");
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
        assert_eq!(doc.input_options(), 1 | 0x1fff << 1); // Existing profiles retain whole-line Enter.
        // Each mark is now independent: 「 on, 、 (backslash) still off.
        doc.set("punctuation", "plain_lbracket", 1);
        assert_eq!(doc.input_options() >> 14 & 0x3f, 1);
        assert!(!doc.shift_punctuation('a'));
    }
    #[test]
    fn space_symbol_toggle_defaults_and_preset_round_trip() {
        let mut old = Document::parse("[profile]\nschema=6\ninteraction=windows\n").unwrap();
        old.migrate();
        assert!(old.flag("input", "space_inputs_zhuyin", false));
        assert_eq!(old.input_options() & (1 << 20),0);
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
        assert_eq!(doc.number("engine", "model", 0), 5);
        assert_eq!(doc.get("candidates", "layout", ""), "horizontal");
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
