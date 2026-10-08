use crate::{
    engine::{Engine, Snapshot},
    profile::Document,
};
use shanjie_core::ffi::ShanjieKey;
use std::{collections::HashMap, fs, path::Path};
pub fn punctuation(key: ShanjieKey) -> &'static str {
    let shift = key.modifiers & 1 != 0;
    match key.ch as u8 as char {
        ',' => {
            if shift {
                "〈"
            } else {
                "，"
            }
        }
        '.' => {
            if shift {
                "〉"
            } else {
                "。"
            }
        }
        '/' => {
            if shift {
                "？"
            } else {
                "／"
            }
        }
        '1' => "！",
        ';' => {
            if shift {
                "："
            } else {
                "；"
            }
        }
        '[' => {
            if shift {
                "『"
            } else {
                "「"
            }
        }
        ']' => {
            if shift {
                "』"
            } else {
                "」"
            }
        }
        '9' => "（",
        '0' => "）",
        '\\' => "、",
        '\'' => {
            if shift {
                "＂"
            } else {
                "＇"
            }
        }
        '=' => {
            if shift {
                "＋"
            } else {
                "＝"
            }
        }
        '`' => "｀",
        _ => "",
    }
}
pub fn is_punctuation(k: ShanjieKey) -> bool {
    k.kind == 1
        && [2, 3].contains(&k.modifiers)
        && ",./1;[]90`\\'=".contains(k.ch as u8 as char)
        && (!"190".contains(k.ch as u8 as char) || k.modifiers == 3)
}
pub fn pending(s: &Snapshot) -> bool {
    s.preedit
        .chars()
        .any(|c| ('\u{3105}'..='\u{312f}').contains(&c))
}
pub fn candidate_key(mut k: ShanjieKey, s: &Snapshot, windows: bool) -> ShanjieKey {
    if s.candidates.is_empty() || k.modifiers != 0 || (s.prediction && s.selected.is_none()) {
        return k;
    }
    // No table expansion reaches the upstream engine. Both profiles stay on a nine-item horizontal page.
    k.kind = match k.kind {
        10 => 8,
        9 => 7,
        13 => 8,
        kind => kind,
    };
    let _ = windows;
    k
}
const SYMBOLS: &str =
    "，。？！；：、（）「」『』【】《》〈〉—…～·「」＋－×÷＝≠≤≥±∞√％‰℃℉°′″→←↑↓★☆○●□■△▲◎※〒＄€£¥";
#[derive(Default)]
pub struct Special {
    pub mode: u32,
    digits: String,
    selected: usize,
}
impl Special {
    pub fn starts(k: ShanjieKey, p: &Document) -> bool {
        k.kind == 1
            && ((k.ch == 96
                && k.modifiers == 0
                && (p.flag("shortcuts", "backquote_symbols", false)
                    || p.flag("shortcuts", "code_input", true)))
                || (p.flag("shortcuts", "symbol_keyboard", true) && k.ch == 44 && k.modifiers == 6))
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    fn view(&self) -> Snapshot {
        let mut s = Snapshot {
            handled: true,
            ..Default::default()
        };
        if self.mode == 4 {
            s.preedit = "符號".into();
            s.cursor = 2;
            s.first = (self.selected / 9 * 9) as u32;
            s.total = SYMBOLS.chars().count() as u32;
            s.candidates = SYMBOLS
                .chars()
                .skip(s.first as usize)
                .take(9)
                .map(|c| c.to_string())
                .collect();
            s.selected = Some(self.selected - s.first as usize);
        } else if self.mode != 0 {
            s.preedit = format!(
                "`{}{}",
                match self.mode {
                    2 => "U",
                    3 => "B",
                    _ => "",
                },
                self.digits
            );
            s.cursor = s.preedit.encode_utf16().count() as u32;
        }
        s
    }
    pub fn pick(&mut self, index: usize) -> Snapshot {
        let s = self.view();
        if self.mode == 4 {
            if let Some(word) = s.candidates.get(index) {
                let out = Snapshot {
                    handled: true,
                    commit: word.clone(),
                    ..Default::default()
                };
                self.clear();
                return out;
            }
        }
        s
    }
    pub fn key(&mut self, k: ShanjieKey, p: &Document) -> Snapshot {
        if self.mode == 0 {
            self.mode = if k.ch == 44 { 4 } else { 1 };
            return self.view();
        }
        if k.kind == 6 {
            self.clear();
            return self.view();
        }
        if self.mode == 4 {
            if k.modifiers == 0 {
                match k.kind {
                    1 if (49..=57).contains(&k.ch) => return self.pick((k.ch - 49) as usize),
                    3 => return self.pick(self.selected % 9),
                    10 | 8 | 13 => {
                        self.selected = (self.selected + 1).min(SYMBOLS.chars().count() - 1)
                    }
                    9 | 7 => self.selected = self.selected.saturating_sub(1),
                    2 if p.flag("candidates", "space_pages", true) => {
                        let next = self.selected / 9 * 9 + 9;
                        if next < SYMBOLS.chars().count() {
                            self.selected = next;
                        } else if p.flag("candidates", "wrap_pages", true) {
                            self.selected = 0;
                        }
                    }
                    4 => self.clear(),
                    _ => {}
                }
            }
            return self.view();
        }
        if k.kind == 4 {
            if !self.digits.is_empty() {
                self.digits.pop();
            } else if self.mode != 1 {
                self.mode = 1;
            } else {
                self.clear();
            }
            return self.view();
        }
        let ch = char::from_u32(k.ch).unwrap_or('\0');
        if self.mode == 1 && k.kind == 1 && k.modifiers & 30 == 0 {
            let lower = ch.to_ascii_lowercase();
            if p.flag("shortcuts", "code_input", true) && (lower == 'u' || lower == 'b') {
                self.mode = if lower == 'u' { 2 } else { 3 };
                return self.view();
            }
            if p.flag("shortcuts", "backquote_symbols", false) {
                let text = punctuation(k);
                if !text.is_empty() {
                    self.clear();
                    return Snapshot {
                        handled: true,
                        commit: text.into(),
                        ..Default::default()
                    };
                }
            }
        }
        if [2, 3].contains(&self.mode)
            && k.kind == 1
            && k.modifiers & 30 == 0
            && ch.is_ascii_hexdigit()
        {
            if self.digits.len() < if self.mode == 2 { 6 } else { 4 } {
                self.digits.push(ch);
            }
            return self.view();
        }
        if [2, 3].contains(&k.kind) && !self.digits.is_empty() {
            let code = u32::from_str_radix(&self.digits, 16).unwrap();
            let text = if self.mode == 2 {
                char::from_u32(code)
                    .filter(|c| *c != '\0')
                    .map(|c| c.to_string())
                    .unwrap_or_default()
            } else if self.digits.len() == 4 {
                use windows::Win32::Globalization::{MultiByteToWideChar, MB_ERR_INVALID_CHARS};
                let bytes = [(code >> 8) as u8, code as u8];
                let mut wide = [0u16; 2];
                let len = unsafe {
                    MultiByteToWideChar(950, MB_ERR_INVALID_CHARS, &bytes, Some(&mut wide))
                };
                String::from_utf16(&wide[..len as usize]).unwrap()
            } else {
                String::new()
            };
            if !text.is_empty() {
                self.clear();
                return Snapshot {
                    handled: true,
                    commit: text,
                    ..Default::default()
                };
            }
        }
        self.view()
    }
}
#[derive(Default)]
pub struct Associated {
    words: Vec<String>,
    selected: usize,
    base: Snapshot,
}
impl Associated {
    pub fn active(&self) -> bool {
        !self.words.is_empty()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn open(&mut self, words: Vec<String>, mut base: Snapshot) {
        base.commit.clear();
        base.candidates.clear();
        base.prediction = false;
        *self = Self {
            words,
            base,
            selected: 0,
        };
    }
    pub fn view(&self) -> Snapshot {
        let mut s = self.base.clone();
        s.handled = true;
        s.associated = true;
        if self.active() {
            s.first = (self.selected / 9 * 9) as u32;
            s.total = self.words.len() as u32;
            s.candidates = self
                .words
                .iter()
                .skip(s.first as usize)
                .take(9)
                .cloned()
                .collect();
            s.selected = Some(self.selected - s.first as usize);
            s.columns = 0;
        }
        s
    }
    pub fn handles(&self, k: ShanjieKey) -> bool {
        self.active()
            && if k.kind == 1 {
                k.modifiers == 1 && (49..=57).contains(&k.ch)
            } else {
                k.modifiers == 0 && [2, 6].contains(&k.kind)
            }
    }
    pub fn pick(&mut self, index: usize) -> Snapshot {
        let frame = self.view();
        if let Some(word) = frame.candidates.get(index) {
            let out = Snapshot {
                handled: true,
                commit: word.clone(),
                ..Default::default()
            };
            self.clear();
            out
        } else {
            frame
        }
    }
    pub fn key(&mut self, k: ShanjieKey) -> Snapshot {
        match k.kind {
            1 => return self.pick((k.ch - 49) as usize),
            3 => return self.pick(self.selected % 9),
            6 => {
                let mut s = self.base.clone();
                s.handled = true;
                self.clear();
                return s;
            }
            2 => {
                self.selected = self.selected / 9 * 9 + 9;
                if self.selected >= self.words.len() {
                    self.selected = 0;
                }
            }
            10 | 8 | 13 => self.selected = (self.selected + 1).min(self.words.len() - 1),
            9 | 7 => self.selected = self.selected.saturating_sub(1),
            _ => {}
        }
        self.view()
    }
}
#[derive(Clone)]
struct Entry {
    keys: String,
    score: f64,
}
#[derive(Default)]
pub struct Pinyin {
    index: HashMap<String, Entry>,
    pub pending: String,
    base: Snapshot,
}
impl Pinyin {
    pub fn load(data: &Path, taiwan: bool) -> std::io::Result<Self> {
        let mut out = Self::default();
        let mut spellings = HashMap::new();
        for line in fs::read_to_string(data.join("pinyin-map.tsv"))?.lines() {
            let f: Vec<_> = line.split_whitespace().collect();
            if f.len() != 3 {
                continue;
            }
            let mut roman = f[if taiwan { 2 } else { 1 }].to_owned();
            if !taiwan && (f[0].starts_with("ㄋㄩ") || f[0].starts_with("ㄌㄩ")) {
                roman = format!("{}v{}", &roman[..1], &roman[2..]);
            }
            spellings.insert(f[0].to_owned(), roman);
        }
        let symbols: Vec<_> =
            "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ"
                .chars()
                .collect();
        let keys = b"1qaz2wsxedcrfv5tgbyhnujm8ik,9ol.0p;/-";
        for file in ["mcbpmf-data.txt", "overlay-add.tsv", "sandhi-add.tsv"] {
            for line in fs::read_to_string(data.join(file))?.lines() {
                if line.starts_with(['#', '_']) {
                    continue;
                }
                let f: Vec<_> = line.split_whitespace().collect();
                if f.len() < 3 {
                    continue;
                }
                let Ok(score) = f[2].parse::<f64>() else {
                    continue;
                };
                let mut plain = String::new();
                let mut toned = String::new();
                let mut sequence = String::new();
                let mut valid = true;
                for part in f[0].split('-') {
                    let mut tone = 1;
                    let sound: String = part
                        .chars()
                        .filter(|c| {
                            if let Some(i) = "ˊˇˋ˙".chars().position(|m| m == *c) {
                                tone = i + 2;
                                false
                            } else {
                                true
                            }
                        })
                        .collect();
                    let Some(spelling) = spellings.get(&sound) else {
                        valid = false;
                        break;
                    };
                    plain += spelling;
                    toned += &format!("{spelling}{tone}");
                    for symbol in sound.chars() {
                        let Some(i) = symbols.iter().position(|c| *c == symbol) else {
                            valid = false;
                            break;
                        };
                        sequence.push(keys[i] as char);
                    }
                    sequence.push([' ', '6', '3', '4', '7'][tone - 1]);
                }
                if valid {
                    for roman in [plain.clone(), toned.clone()] {
                        out.add(roman, &sequence, score);
                    }
                    if !taiwan {
                        out.add(
                            plain.replace("nve", "nue").replace("lve", "lue"),
                            &sequence,
                            score,
                        );
                        out.add(
                            toned.replace("nve", "nue").replace("lve", "lue"),
                            &sequence,
                            score,
                        );
                    }
                }
            }
        }
        Ok(out)
    }
    fn add(&mut self, roman: String, keys: &str, score: f64) {
        if !self.index.get(&roman).is_some_and(|e| e.score >= score) {
            self.index.insert(
                roman,
                Entry {
                    keys: keys.into(),
                    score,
                },
            );
        }
    }
    pub fn enabled(&self) -> bool {
        !self.index.is_empty()
    }
    pub fn clear(&mut self) {
        self.pending.clear();
        self.base = Snapshot::default();
    }
    pub fn handles(&self, k: ShanjieKey) -> bool {
        self.enabled()
            && k.modifiers & 30 == 0
            && (!self.pending.is_empty()
                || (k.kind == 1 && (97..=122).contains(&k.ch) && k.modifiers == 0))
    }
    fn preview(&self) -> Snapshot {
        let mut s = self.base.clone();
        s.handled = true;
        s.commit.clear();
        s.candidates.clear();
        s.selected = None;
        let units: Vec<u16> = s.preedit.encode_utf16().collect();
        let (left, right) = units.split_at(s.cursor as usize);
        s.preedit = format!(
            "{}{}{}",
            String::from_utf16(left).unwrap(),
            self.pending,
            String::from_utf16(right).unwrap()
        );
        s.cursor += self.pending.len() as u32;
        s
    }
    fn convert(&mut self, e: &mut Engine, ascii: bool) -> Option<Snapshot> {
        let roman = self.pending.replace(' ', "");
        if roman.is_empty() {
            self.pending.clear();
            return Some(self.base.clone());
        }
        let mut plans = vec![(f64::NEG_INFINITY, String::new()); roman.len() + 1];
        plans[0].0 = 0.0;
        for end in 1..=roman.len() {
            for start in 0..end {
                if let Some(entry) = self.index.get(&roman[start..end]) {
                    let score = plans[start].0 + entry.score - 0.5;
                    if score > plans[end].0 {
                        plans[end] = (score, format!("{}{}", plans[start].1, entry.keys));
                    }
                }
            }
        }
        if plans.last()?.1.is_empty() {
            if !ascii {
                return Some(self.preview());
            }
            let mut s = e.reset(false)?;
            s.commit += &self.pending;
            s.handled = true;
            self.clear();
            return Some(s);
        }
        let mut committed = String::new();
        let mut s = Snapshot::default();
        for ch in plans.last()?.1.bytes() {
            s = e.key(ShanjieKey {
                kind: if ch == b' ' { 2 } else { 1 },
                ch: ch as u32,
                modifiers: 0,
            })?;
            committed += &s.commit;
        }
        s.commit = committed;
        self.pending.clear();
        self.base = s.clone();
        Some(s)
    }
    pub fn key(
        &mut self,
        k: ShanjieKey,
        e: &mut Engine,
        current: Snapshot,
        ascii: bool,
        space: bool,
    ) -> Option<Snapshot> {
        if self.pending.is_empty() {
            self.base = current;
            self.base.commit.clear();
            if !self.base.candidates.is_empty() {
                self.base = e.key(ShanjieKey {
                    kind: 6,
                    ch: 0,
                    modifiers: 0,
                })?;
            }
        }
        if k.kind == 1 && (97..=122).contains(&k.ch) && k.modifiers == 0 {
            if self.pending.len() < 64 {
                self.pending.push(k.ch as u8 as char);
            }
            return Some(self.preview());
        }
        if k.kind == 1 && (49..=53).contains(&k.ch) && k.modifiers == 0 {
            self.pending.push(k.ch as u8 as char);
            return self.convert(e, ascii);
        }
        if k.kind == 4 {
            self.pending.pop();
            return Some(self.preview());
        }
        if k.kind == 6 {
            self.pending.clear();
            let mut s = self.base.clone();
            s.handled = true;
            return Some(s);
        }
        if k.kind == 2 && !space && !self.pending.ends_with(' ') {
            self.pending.push(' ');
            return Some(self.preview());
        }
        let s = self.convert(e, ascii)?;
        if !self.pending.is_empty() || k.kind == 2 {
            return Some(s);
        }
        let committed = s.commit;
        let mut s = e.key(k)?;
        s.commit = committed + &s.commit;
        Some(s)
    }
}
