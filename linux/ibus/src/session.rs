//! One input context, with the decisions of windows/src/tsf/keys.rs (`wants`, `key_down`) and
//! windows/src/tsf/service.rs (`apply`, `cancel`, `toggle`, `pick_at`) in the same order. Where TSF writes a
//! composition, this records IBus effects; where Win32 asks the keyboard, this reads the IBus event.
use crate::{
    bus::{Effect, Effects, Session, CAP_SURROUNDING_TEXT},
    engine::{Config, Engine},
    keys, profile, serial,
};
use shanjie_core::ffi::ShanjieKey;
use shanjie_shell::{
    input::{self, Associated, Pinyin, Special},
    profile::{Document, REBUILD},
    protocol::{Failure, Op, Snapshot},
};
use std::{collections::BTreeSet, path::PathBuf};

type R<T> = Result<T, ()>;

fn output(s: Option<Snapshot>) -> R<Snapshot> {
    s.ok_or(())
}

// IBusInputPurpose (ibustypes.h): password and PIN fields keep their keys, like a read-only TSF context.
const PURPOSE_PASSWORD: u32 = 8;
const PURPOSE_PIN: u32 = 9;
const MODE_PROPERTY: &str = "InputMode";
const SETUP_PROPERTY: &str = "Setup";

/// installation::layout with every Windows keyboard profile available and the standard one active.
pub fn layout(keyboard: u32) -> u32 {
    (keyboard == 2) as u32
}

pub fn config(data: &std::path::Path, p: &Document) -> (Config, u32) {
    let keyboard = p.number("input", "keyboard", 0);
    (
        Config {
            data: data.to_owned(),
            folder: profile::folder(),
            model: profile::model(data, &p.model_id()),
            engine: shanjie_shell::protocol::Config::from_profile(p, layout(keyboard)),
        },
        keyboard,
    )
}

/// windows/src/tsf/service.rs `option_operations`.
pub fn option_operations(p: &Document) -> [Op; 4] {
    let keyboard = p.number("input", "keyboard", 0);
    let zhuyin = keyboard < 3;
    let upstream = zhuyin && keyboard != 2 && p.flag("input", "upstream_english", false);
    let inline = p.flag("input", "english_inline", true);
    [
        Op::Options(p.input_options()),
        Op::English(upstream as u32 | (inline as u32) << 1),
        Op::Mixed(zhuyin && p.flag("input", "mixed_input", false)),
        Op::Tolerance(zhuyin && p.flag("input", "mixed_tolerance", false)),
    ]
}

pub struct Shanjie {
    data: PathBuf,
    prefs: Document,
    engine: Option<Engine>,
    pinyin: Pinyin,
    special: Special,
    associated: Associated,
    snapshot: Snapshot,
    raw_reading: String,
    chinese: bool,
    full_width: bool,
    shift_tap: bool,
    shift_used: bool,
    shift_physical: u32,
    /// Keycodes pressed and not yet released, standing in for GetKeyState.
    held: BTreeSet<u32>,
    capabilities: u32,
    purpose: u32,
    /// The client's text before the cursor, from SetSurroundingText.
    surrounding: Option<String>,
}

impl Shanjie {
    pub fn new(data: PathBuf, prefs: Document) -> Self {
        let chinese = prefs.flag("input", "chinese", true);
        let full_width = prefs.flag("input", "full_width", false);
        Self {
            data,
            prefs,
            engine: None,
            pinyin: Pinyin::default(),
            special: Special::default(),
            associated: Associated::default(),
            snapshot: Snapshot::default(),
            raw_reading: String::new(),
            chinese,
            full_width,
            shift_tap: false,
            shift_used: false,
            shift_physical: 0,
            held: BTreeSet::new(),
            capabilities: 0,
            purpose: 0,
            surrounding: None,
        }
    }

    fn engine(&mut self) -> R<&mut Engine> {
        self.engine.as_mut().ok_or(())
    }

    /// service.rs `ensure_engine`: open the core with the current settings, then apply the options.
    fn ensure_engine(&mut self) -> R<()> {
        if self.engine.is_some() {
            return Ok(());
        }
        let (config, keyboard) = config(&self.data, &self.prefs);
        let mut engine = Engine::open(&config).ok_or(())?;
        self.pinyin = if keyboard >= 3 { Pinyin::load(&self.data, keyboard == 4).map_err(|_| ())? } else { Pinyin::default() };
        for op in option_operations(&self.prefs) {
            output(engine.perform(op))?;
        }
        output(engine.perform(Op::Demote(self.prefs.flag("engine", "demote", true))))?;
        self.engine = Some(engine);
        Ok(())
    }

    fn mode_property(&self) -> serial::Property {
        let (label, symbol) = match (self.chinese, self.full_width) {
            (true, false) => ("中文（半形）", "中"),
            (true, true) => ("中文（全形）", "中"),
            (false, false) => ("英數（半形）", "英"),
            (false, true) => ("英數（全形）", "全"),
        };
        serial::Property::button(MODE_PROPERTY, label, "按一下切換中英", symbol)
    }

    fn register_properties(&self, out: &mut Effects) {
        out.push(Effect::Properties(vec![
            self.mode_property(),
            serial::Property::button(SETUP_PROPERTY, "善解設定", "開啟善解設定", ""),
        ]));
    }

    /// service.rs `set_mode`: the mode lives in this context; the top bar shows it.
    fn set_mode(&mut self, chinese: bool, out: &mut Effects) {
        self.chinese = chinese;
        out.push(Effect::UpdateProperty(self.mode_property()));
    }

    /// keys.rs `wants`: whether this key belongs to the input method at all.
    fn wants(&self, keyval: u32, keycode: u32, state: u32) -> bool {
        if keys::system_shortcut(keyval, state) || [PURPOSE_PASSWORD, PURPOSE_PIN].contains(&self.purpose) {
            return false;
        }
        let p = &self.prefs;
        let control = state & keys::CONTROL_MASK != 0;
        let alt = state & keys::MOD1_MASK != 0;
        let shift = state & keys::SHIFT_MASK != 0;
        let side = p.number("shortcuts", "shift_side", 0);
        if p.flag("shortcuts", "shift_toggle", true)
            && keys::is_shift(keyval)
            && (side == 0 || keycode == if side == 1 { keys::KEY_LEFTSHIFT } else { keys::KEY_RIGHTSHIFT })
            && !control
            && !alt
        {
            return true;
        }
        let space = keyval == 0x20;
        if (p.flag("shortcuts", "ctrl_space_toggle", false) && space && control && !shift && !alt)
            || (p.flag("shortcuts", "shift_space_toggle", true) && space && shift && !control && !alt)
        {
            return true;
        }
        let Some(mut key) = keys::translate(keyval, keycode, state) else { return false };
        if self.full_width && !self.chinese && !control && !alt {
            return [1, 2].contains(&key.kind);
        }
        if !self.chinese {
            return false;
        }
        if !p.flag("input", "caps_lock_english", false) {
            key.modifiers &= !16;
        }
        if Special::starts(key, p) {
            return true;
        }
        if key.modifiers & 12 != 0 {
            return false;
        }
        if key.modifiers & 2 != 0 {
            if self.special.mode != 0 {
                return false;
            }
            return (p.flag("shortcuts", "ctrl_punctuation", true) && input::is_punctuation(key))
                || (p.flag("shortcuts", "ctrl_backslash", false) && key.kind == 1 && key.ch == 92 && key.modifiers == 2)
                || (p.flag("shortcuts", "forget_candidate", false)
                    && p.flag("input", "learning", true)
                    && key.kind == 4
                    && key.modifiers == 3
                    && self.snapshot.selected.is_some());
        }
        if self.associated.handles(key) || self.pinyin.handles(key) || self.special.mode != 0 || !self.snapshot.preedit.is_empty() {
            return true;
        }
        key.kind == 1
            && (key.modifiers & 30 == 0
                || (p.flag("input", "caps_lock_english", false) && key.modifiers & 16 != 0 && key.modifiers & 14 == 0)
                || (p.flag("shortcuts", "ctrl_punctuation", true) && input::is_punctuation(key)))
    }

    /// keys.rs `key_down`, after `wants` said yes.
    fn key_down(&mut self, keyval: u32, keycode: u32, state: u32, repeat: bool, out: &mut Effects) -> R<bool> {
        let p = self.prefs.clone();
        let control = state & keys::CONTROL_MASK != 0;
        let alt = state & keys::MOD1_MASK != 0;
        let shift = state & keys::SHIFT_MASK != 0;
        if p.flag("shortcuts", "shift_toggle", true) && keys::is_shift(keyval) {
            if self.shift_tap && self.shift_physical != keycode {
                self.shift_used = true;
            }
            if !repeat && !self.shift_tap {
                self.shift_tap = true;
                self.shift_used = self.held.iter().any(|&k| k != keycode);
                self.shift_physical = keycode;
            }
            // The application still sees Shift; only the tap on release belongs to the input method.
            return Ok(false);
        }
        if p.flag("shortcuts", "ctrl_space_toggle", false) && keyval == 0x20 && control && !shift && !alt {
            if !repeat {
                self.toggle(out)?;
            }
            return Ok(true);
        }
        if p.flag("shortcuts", "shift_space_toggle", true) && keyval == 0x20 && shift && !control && !alt {
            if !repeat {
                self.full_width = !self.full_width;
                let chinese = self.chinese;
                self.set_mode(chinese, out);
            }
            return Ok(true);
        }
        self.ensure_engine()?;
        let mut key = keys::translate(keyval, keycode, state).ok_or(())?;
        if !p.flag("input", "caps_lock_english", false) {
            key.modifiers &= !16;
        }
        let state = self.snapshot.clone();
        let closed = state.candidates.is_empty() || (state.prediction && state.selected.is_none());
        if key.kind == 6 && closed && !self.associated.active() && self.special.mode == 0 {
            let escape = p.get("input", "escape", "staged");
            if escape == "ignore" {
                return Ok(true);
            }
            if escape == "clear" {
                self.pinyin.clear();
                self.raw_reading.clear();
                let mut o = output(self.engine()?.reset(true))?;
                o.handled = true;
                self.apply(o, false, out)?;
                return Ok(true);
            }
        }
        if p.flag("shortcuts", "ctrl_punctuation", true) && input::is_punctuation(key) {
            let mark = input::punctuation(key).to_owned();
            let mut converted = Snapshot::default();
            if !self.pinyin.pending.is_empty() {
                let state = self.snapshot.clone();
                let engine = self.engine.as_mut().ok_or(())?;
                converted = output(self.pinyin.key(
                    ShanjieKey { kind: 2, ch: 0, modifiers: 0 },
                    engine,
                    state,
                    p.flag("input", "alphanumeric", true),
                    true,
                ))?;
                if !self.pinyin.pending.is_empty() {
                    self.apply(converted, false, out)?;
                    return Ok(true);
                }
            }
            self.raw_reading.clear();
            self.associated.clear();
            let mut o = output(self.engine()?.perform(Op::Punctuation(mark)))?;
            o.commit = converted.commit + &o.commit;
            self.apply(o, false, out)?;
            return Ok(true);
        }
        let space_candidates = p.flag("candidates", "space_operates", false);
        let trigger = if space_candidates { p.get("candidates", "open", "down").to_owned() } else { "down".to_owned() };
        if key.kind == 2
            && key.modifiers == 0
            && self.special.mode == 0
            && self.pinyin.pending.is_empty()
            && !input::pending(&state)
            && !self.engine()?.mixed_pending()
            && !(space_candidates && (!closed || trigger != "down"))
        {
            self.associated.clear();
            self.raw_reading.clear();
            let o = if p.flag("input", "space_commits", false) {
                let mut o = output(self.engine()?.reset(false))?;
                o.handled = true;
                if p.flag("input", "space_after_commit", false) {
                    o.commit.push(' ');
                }
                o
            } else {
                output(self.engine()?.perform(Op::Space))?
            };
            self.apply(o, false, out)?;
            return Ok(true);
        }
        if self.associated.handles(key) {
            if key.kind == 2
                && (!p.flag("candidates", "space_pages", true) || (!p.flag("candidates", "wrap_pages", true) && state.first + 9 >= state.total))
            {
                return Ok(true);
            }
            let o = if key.kind == 1 { self.pick_associated(key.ch - 49)? } else { self.associated.key(key) };
            self.apply(o, false, out)?;
            return Ok(true);
        }
        if self.associated.active() {
            self.associated.clear();
            self.snapshot.associated = false;
            self.snapshot.candidates.clear();
            out.push(Effect::HideLookup);
        }
        if self.pinyin.handles(key) {
            let state = self.snapshot.clone();
            let engine = self.engine.as_mut().ok_or(())?;
            let o = output(self.pinyin.key(key, engine, state, p.flag("input", "alphanumeric", true), p.flag("input", "pinyin_space_converts", false)))?;
            self.apply(o, false, out)?;
            return Ok(true);
        }
        if self.special.mode != 0 || Special::starts(key, &p) {
            let pending = if self.special.mode == 0 { output(self.engine()?.reset(false))? } else { Snapshot::default() };
            let mut o = self.special.key(key, &p);
            o.commit = pending.commit + &o.commit;
            self.apply(o, false, out)?;
            return Ok(true);
        }
        let selected = !state.candidates.is_empty()
            && state.selected.is_some()
            && key.modifiers == 0
            && (key.kind == 3 || (key.kind == 1 && (49..=57).contains(&key.ch)));
        key = input::candidate_key(key, &state, p.get("profile", "interaction", "windows") == "windows");
        if p.flag("shortcuts", "forget_candidate", false)
            && p.flag("input", "learning", true)
            && key.kind == 4
            && key.modifiers == 3
            && state.selected.is_some()
        {
            key.modifiers = 8;
        }
        let english = p.flag("input", "caps_lock_english", false) && key.modifiers & 16 != 0 && key.modifiers & 14 == 0 && [1, 2].contains(&key.kind);
        let keyboard = p.number("input", "keyboard", 0);
        let mixed_letter = (p.flag("input", "mixed_input", false) || (p.flag("input", "upstream_english", false) && layout(keyboard) == 0))
            && keyboard < 3
            && key.kind == 1
            && (key.ch as u8).is_ascii_alphabetic()
            && key.modifiers & 14 == 0
            && !english;
        let latin = if mixed_letter { keys::typed(keyval).filter(|c| c.is_ascii_alphabetic()) } else { None };
        let ascii = !mixed_letter && key.kind == 1 && key.modifiers == 1 && !p.shift_punctuation(key.ch as u8 as char);
        if (self.full_width && !self.chinese) || english || ascii {
            let Some(ch) = keys::typed(keyval) else { return Ok(false) };
            let mut o = if ascii && !english && self.chinese {
                output(self.engine()?.perform(Op::Punctuation(ch.to_string())))?
            } else {
                let mut o = output(self.engine()?.reset(false))?;
                o.commit.push(ch);
                o
            };
            o.handled = true;
            self.apply(o, false, out)?;
            return Ok(true);
        }
        if !state.prediction
            && key.kind == 2
            && !state.candidates.is_empty()
            && (!p.flag("candidates", "space_pages", true)
                || (!p.flag("candidates", "wrap_pages", true) && state.first + (state.selected.unwrap_or(0) / 9 + 1) as u32 * 9 >= state.total))
        {
            return Ok(true);
        }
        let delete = p.flag("candidates", "backspace_deletes", false) && !state.prediction && !state.candidates.is_empty() && key.kind == 4 && key.modifiers == 0;
        let mixed_pending = self.engine()?.mixed_pending();
        let bypass = key.modifiers == 0 && closed && !state.preedit.is_empty() && !input::pending(&state) && !mixed_pending && key.kind == 10 && trigger == "space";
        let cycle = p.flag("candidates", "up_cycles", true)
            && key.kind == 9
            && key.modifiers == 0
            && closed
            && !state.preedit.is_empty()
            && !input::pending(&state)
            && !mixed_pending;
        if self.snapshot.preedit.is_empty() && p.flag("input", "context", true) {
            let left = self.left_context();
            output(self.engine()?.perform(Op::Context(left)))?;
        }
        let mut o = if bypass {
            let mut o = output(self.engine()?.reset(false))?;
            o.handled = key.kind == 2 && !p.flag("input", "space_after_commit", false);
            o
        } else if cycle {
            output(self.engine()?.perform(Op::Cycle))?
        } else {
            let mut raw = self.raw_reading.clone();
            if key.kind == 4 {
                raw.pop();
            }
            if key.kind == 1 && key.modifiers == 0 {
                raw.push(key.ch as u8 as char);
            } else if key.kind == 2 && key.modifiers == 0 {
                raw.push(' ');
            }
            let o = if let Some(o) = self.engine()?.key_with_latin(key, latin) {
                o
            } else {
                // Raw-reading fallback is only for invalid phonetic input; a busy learning writer keeps the key.
                if self.engine()?.failure() != Some(Failure::Failed) {
                    return Err(());
                }
                if !p.flag("input", "alphanumeric", true) || raw.is_empty() || !input::pending(&self.snapshot) {
                    return Err(());
                }
                let mut units: Vec<_> = self.snapshot.preedit.encode_utf16().collect();
                let end = (self.snapshot.cursor as usize).min(units.len());
                let mut start = end;
                while start > 0 && ((0x3105..=0x312f).contains(&units[start - 1]) || [0x2ca, 0x2c7, 0x2cb, 0x2d9].contains(&units[start - 1])) {
                    start -= 1;
                }
                units.splice(start..end, raw.encode_utf16());
                Snapshot { handled: true, commit: String::from_utf16(&units).unwrap(), ..Default::default() }
            };
            self.raw_reading = if input::pending(&o) { raw } else { String::new() };
            o
        };
        if delete {
            o = output(self.engine()?.key(key))?;
        }
        if key.kind == 2
            && key.modifiers == 0
            && !p.flag("input", "space_inputs_zhuyin", true)
            && p.flag("input", "space_commits", false)
            && !input::pending(&o)
            && o.preedit.ends_with(' ')
        {
            let committed = o.commit;
            o = output(self.engine()?.reset(false))?;
            o.commit = committed + &o.commit;
        }
        if p.flag("candidates", "enter_commits", true)
            && selected
            && key.kind == 3
            && key.modifiers == 0
            && o.candidates.is_empty()
            && !input::pending(&o)
            && !o.preedit.is_empty()
        {
            let committed = o.commit;
            o = output(self.engine()?.key(key))?;
            o.commit = committed + &o.commit;
        }
        let handled = o.handled;
        self.apply(o, selected && !state.prediction && key.kind != 3, out)?;
        Ok(handled)
    }

    /// The eight UTF-16 units before the cursor, without a split surrogate (keys.rs left context).
    fn left_context(&self) -> String {
        let Some(text) = self.surrounding.as_ref() else { return String::new() };
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut units = &units[units.len().saturating_sub(8)..];
        if units.first().is_some_and(|u| (0xdc00..=0xdfff).contains(u)) {
            units = &units[1..];
        }
        if units.last().is_some_and(|u| (0xd800..=0xdbff).contains(u)) {
            units = &units[..units.len() - 1];
        }
        String::from_utf16(units).unwrap_or_default()
    }

    /// service.rs `pick_associated`.
    fn pick_associated(&mut self, index: u32) -> R<Snapshot> {
        let text = self.associated.pick(index as usize);
        if text.associated {
            return Ok(text);
        }
        let mut o = output(self.engine()?.reset(false))?;
        let mut units: Vec<_> = o.commit.encode_utf16().collect();
        let at = (self.snapshot.cursor as usize).min(units.len());
        units.splice(at..at, text.commit.encode_utf16());
        o.commit = String::from_utf16(&units).unwrap();
        o.handled = true;
        Ok(o)
    }

    /// service.rs `apply`: commit the final prefix, show the rest as the preedit, then the candidates.
    fn apply(&mut self, o: Snapshot, selected: bool, out: &mut Effects) -> R<()> {
        if !o.handled && o.commit.is_empty() && o.preedit.is_empty() {
            return Ok(());
        }
        if !o.commit.is_empty() {
            let committed = if self.full_width {
                o.commit
                    .chars()
                    .map(|c| if c == ' ' { '\u{3000}' } else if ('!'..='~').contains(&c) { char::from_u32(c as u32 + 0xfee0).unwrap() } else { c })
                    .collect()
            } else {
                o.commit.clone()
            };
            out.push(Effect::Commit(committed));
            if let Some(text) = self.surrounding.as_mut() {
                // Until the client reports again, the committed text is what precedes the cursor.
                text.push_str(&o.commit);
            }
        }
        self.snapshot = o.clone();
        if self.prefs.flag("input", "associations", false) && selected && o.candidates.is_empty() && o.cursor != 0 && !o.preedit.is_empty() {
            let before: Vec<_> = o.preedit.encode_utf16().take(o.cursor as usize).collect();
            let text = String::from_utf16(&before).unwrap();
            let mut chars: Vec<_> = text.chars().rev().take(2).collect();
            chars.reverse();
            let mut suggestions = output(self.engine()?.perform(Op::Associate(chars.iter().collect())))?;
            if suggestions.candidates.is_empty() && chars.len() == 2 {
                suggestions = output(self.engine()?.perform(Op::Associate(chars[1].to_string())))?;
            }
            self.associated.open(suggestions.candidates, o);
            self.snapshot = self.associated.view();
        }
        self.render(out);
        Ok(())
    }

    /// The preedit and the candidate page of the current snapshot.
    fn render(&self, out: &mut Effects) {
        let s = &self.snapshot;
        if s.preedit.is_empty() {
            out.push(Effect::HidePreedit);
        } else {
            let units: Vec<u16> = s.preedit.encode_utf16().collect();
            let cursor = String::from_utf16_lossy(&units[..(s.cursor as usize).min(units.len())]).chars().count() as u32;
            let len = s.preedit.chars().count() as u32;
            out.push(Effect::Preedit(s.preedit.clone(), cursor, vec![(serial::ATTR_UNDERLINE, serial::UNDERLINE_SINGLE, 0, len)]));
        }
        if s.reading_hint {
            // Mixed input: the raw keys stay in the preedit; the zhuyin they spell is a small hint.
            out.push(Effect::HideLookup);
            match s.candidates.first() {
                Some(hint) if !hint.is_empty() => out.push(Effect::Auxiliary(hint.clone())),
                _ => out.push(Effect::HideAuxiliary),
            }
            return;
        }
        out.push(Effect::HideAuxiliary);
        if s.candidates.is_empty() {
            out.push(Effect::HideLookup);
            return;
        }
        let labels = (1..=s.candidates.len())
            .map(|i| match s.selected {
                None => String::new(),
                Some(_) if s.associated => format!("⇧{i}"),
                Some(_) => i.to_string(),
            })
            .collect();
        let vertical = self.prefs.get("candidates", "layout", "horizontal") == "vertical";
        out.push(Effect::Lookup(s.candidates.clone(), labels, s.selected.map(|i| i as u32), vertical));
    }

    /// service.rs `toggle`.
    fn toggle(&mut self, out: &mut Effects) -> R<()> {
        let chinese = !self.chinese;
        self.associated.clear();
        if self.snapshot.preedit.is_empty() {
            out.push(Effect::HideLookup);
            self.set_mode(chinese, out);
            return Ok(());
        }
        let pending = (!self.pinyin.pending.is_empty() || [1, 2, 3].contains(&self.special.mode)).then(|| self.snapshot.preedit.clone());
        let mut o = output(self.engine()?.reset(false))?;
        if let Some(pending) = pending {
            o.commit = pending;
        }
        self.special.clear();
        self.pinyin.clear();
        self.raw_reading.clear();
        self.apply(o, false, out)?;
        self.set_mode(chinese, out);
        Ok(())
    }

    /// service.rs `cancel`: what is shown stays in the document; the composition ends. On focus out and
    /// reset IBus commits the shown preedit itself (PREEDIT_COMMIT), so `commit` is false there.
    fn cancel(&mut self, commit: bool, out: &mut Effects) {
        let final_text = if self.snapshot.preedit.is_empty() {
            String::new()
        } else if !self.pinyin.pending.is_empty() || [1, 2, 3].contains(&self.special.mode) {
            let text = self.snapshot.preedit.clone();
            if let Some(engine) = self.engine.as_mut() {
                let _ = engine.reset(true);
            }
            text
        } else {
            self.engine.as_mut().and_then(|e| e.reset(false)).map(|o| o.commit).unwrap_or_else(|| self.snapshot.preedit.clone())
        };
        if commit && !final_text.is_empty() {
            out.push(Effect::Commit(final_text));
        }
        self.shift_tap = false;
        self.shift_used = false;
        self.special.clear();
        self.pinyin.clear();
        self.associated.clear();
        self.raw_reading.clear();
        self.snapshot = Snapshot::default();
        out.push(Effect::HidePreedit);
        out.push(Effect::HideLookup);
        out.push(Effect::HideAuxiliary);
    }

    /// service.rs `pick_at`.
    fn pick_at(&mut self, index: u32, out: &mut Effects) -> R<()> {
        if self.snapshot.reading_hint {
            return Ok(());
        }
        let o = if self.associated.active() {
            self.pick_associated(index)?
        } else if self.special.mode != 0 {
            self.special.pick(index as usize)
        } else {
            output(self.engine()?.perform(Op::Pick(index)))?
        };
        let conversion = !self.snapshot.prediction && !o.associated && !o.preedit.is_empty();
        self.apply(o, conversion, out)
    }

    /// Replaces the settings; an engine built from different values is rebuilt on the next key.
    fn reload(&mut self, current: Document, out: &mut Effects) {
        let old = std::mem::replace(&mut self.prefs, current.clone());
        let rebuild = REBUILD.iter().any(|(s, k)| current.get(s, k, "") != old.get(s, k, ""));
        let mode_changed = current.get("input", "chinese", "") != old.get("input", "chinese", "");
        let width_changed = current.get("input", "full_width", "") != old.get("input", "full_width", "");
        if rebuild || mode_changed {
            self.cancel(true, out);
        }
        if rebuild {
            self.engine = None;
            self.pinyin = Pinyin::default();
        } else if let Some(engine) = self.engine.as_mut() {
            for op in option_operations(&current) {
                let _ = engine.perform(op);
            }
            if current.get("engine", "demote", "") != old.get("engine", "demote", "") {
                let _ = engine.perform(Op::Demote(current.flag("engine", "demote", true)));
            }
            if current.get("engine", "prediction", "") != old.get("engine", "prediction", "") {
                let _ = engine.perform(Op::Prediction(current.flag("engine", "prediction", false)));
            }
        }
        if !current.flag("input", "associations", false) && self.associated.active() {
            self.associated.clear();
            self.snapshot.associated = false;
            self.snapshot.candidates.clear();
            self.snapshot.selected = None;
            out.push(Effect::HideLookup);
        }
        if width_changed {
            self.full_width = current.flag("input", "full_width", false);
        }
        if mode_changed || width_changed {
            let chinese = if mode_changed { current.flag("input", "chinese", true) } else { self.chinese };
            self.set_mode(chinese, out);
        }
    }

    /// Restart from settings: drop the engine so the next key opens a fresh one.
    fn restart(&mut self, out: &mut Effects) {
        self.cancel(true, out);
        self.engine = None;
        self.pinyin = Pinyin::default();
    }
}


impl Session for Shanjie {
    fn key(&mut self, keyval: u32, keycode: u32, state: u32, out: &mut Effects) -> bool {
        if state & keys::RELEASE_MASK != 0 {
            self.held.remove(&keycode);
            if !keys::is_shift(keyval) {
                self.shift_used = true;
            } else if self.shift_tap {
                // keys.rs OnKeyUp: a Shift released alone toggles the mode.
                self.shift_tap = false;
                let toggle = !self.shift_used && self.shift_physical == keycode && self.held.is_empty();
                self.shift_used = false;
                if toggle && self.toggle(out).is_err() {
                    self.cancel(true, out);
                }
            }
            return false;
        }
        let repeat = !self.held.insert(keycode);
        if !keys::is_shift(keyval) && self.shift_tap {
            self.shift_used = true;
        }
        if keys::system_shortcut(keyval, state) || !self.wants(keyval, keycode, state) {
            return false;
        }
        match self.key_down(keyval, keycode, state, repeat, out) {
            Ok(handled) => handled,
            Err(()) => {
                eprintln!("ibus-engine-shanjie: key not completed");
                true
            }
        }
    }
    fn focus_in(&mut self, out: &mut Effects) {
        self.register_properties(out);
        if self.capabilities & CAP_SURROUNDING_TEXT != 0 {
            out.push(Effect::RequireSurrounding);
        }
    }
    fn focus_out(&mut self, out: &mut Effects) {
        self.cancel(false, out);
        self.held.clear();
        self.surrounding = None;
    }
    fn reset(&mut self, out: &mut Effects) {
        self.cancel(false, out);
    }
    fn enable(&mut self, out: &mut Effects) {
        self.register_properties(out);
        if self.capabilities & CAP_SURROUNDING_TEXT != 0 {
            out.push(Effect::RequireSurrounding);
        }
    }
    fn disable(&mut self, out: &mut Effects) {
        self.cancel(false, out);
    }
    fn capabilities(&mut self, caps: u32) {
        self.capabilities = caps;
    }
    fn content_type(&mut self, purpose: u32, _hints: u32) {
        self.purpose = purpose;
    }
    fn surrounding(&mut self, text: &str, cursor: u32, _anchor: u32, _out: &mut Effects) {
        self.surrounding = Some(text.chars().take(cursor as usize).collect());
    }
    fn candidate_clicked(&mut self, index: u32, out: &mut Effects) {
        if self.pick_at(index, out).is_err() {
            eprintln!("ibus-engine-shanjie: pick not completed");
        }
    }
    fn page(&mut self, down: bool, out: &mut Effects) {
        // The panel's page buttons act like the keys that page on Windows: Space forward, ← back.
        if self.snapshot.candidates.is_empty() || self.engine.is_none() {
            return;
        }
        let vertical = self.snapshot.columns == 0 && self.prefs.get("candidates", "layout", "horizontal") == "vertical";
        let key = match (vertical, down) {
            (true, true) => ShanjieKey { kind: 15, ch: 0, modifiers: 0 },
            (true, false) => ShanjieKey { kind: 14, ch: 0, modifiers: 0 },
            (false, true) => ShanjieKey { kind: 2, ch: 0, modifiers: 0 },
            (false, false) => return,
        };
        if let Some(o) = self.engine.as_mut().and_then(|e| e.key(key)) {
            let _ = self.apply(o, false, out);
        }
    }
    fn cursor(&mut self, _down: bool, _out: &mut Effects) {}
    fn notify(&mut self, kind: u32, out: &mut Effects) {
        if kind == 1 {
            self.restart(out);
        }
        match profile::load(&profile::path()) {
            Ok(current) => self.reload(current, out),
            Err(error) => eprintln!("ibus-engine-shanjie: profile.ini unreadable ({error}); settings kept"),
        }
    }
    fn property(&mut self, name: &str, _state: u32, out: &mut Effects) {
        match name {
            MODE_PROPERTY => {
                if self.toggle(out).is_err() {
                    self.cancel(true, out);
                }
            }
            SETUP_PROPERTY => {
                let _ = std::process::Command::new("shanjie-settings").spawn();
            }
            _ => {}
        }
    }
}
