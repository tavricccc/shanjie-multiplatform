use super::service::{compartment, output, pick_associated, range, selection, Data, Service_Impl};
use crate::{
    engine::Snapshot,
    input::{self, Special},
};
use shanjie_core::ffi::ShanjieKey;
use std::{cell::Cell, rc::Rc};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        UI::{Input::KeyboardAndMouse::*, TextServices::*},
    },
};
pub fn shift(vk: WPARAM) -> bool {
    [16, 160, 161].contains(&vk.0)
}
fn translate(vk: WPARAM, flags: LPARAM) -> Option<ShanjieKey> {
    let mut key = ShanjieKey {
        kind: 0,
        ch: 0,
        modifiers: 0,
    };
    if unsafe { crate::bridge::sj_translate(vk.0, flags.0, &mut key) } {
        Some(key)
    } else {
        None
    }
}
fn held(key: VIRTUAL_KEY) -> bool {
    unsafe { GetKeyState(key.0 as i32) as u16 & 0x8000 != 0 }
}
fn system_shortcut(key: WPARAM) -> bool {
    key.0 == VK_LWIN.0 as usize || key.0 == VK_RWIN.0 as usize || held(VK_LWIN) || held(VK_RWIN)
}
fn plain_shortcut(vk: WPARAM, modifier: VIRTUAL_KEY, other: VIRTUAL_KEY) -> bool {
    vk.0 == 32 && held(modifier) && !held(other) && !held(VK_MENU)
}
fn physical_shift(vk: WPARAM, flags: LPARAM) -> u32 {
    if vk.0 == 16 { unsafe { MapVirtualKeyW(((flags.0 >> 16) & 255) as u32, MAPVK_VSC_TO_VK_EX) } }
    else { vk.0 as u32 }
}
fn other_held(physical: u32) -> bool {
    (1..=254).any(|vk| vk != physical && vk != 16 && held(VIRTUAL_KEY(vk as u16)))
}
impl Data {
    fn ctrl_event(self: &Rc<Self>, context: Option<&ITfContext>, key:WPARAM, flags:LPARAM, down:bool) -> Result<()> {
        let p = self.state.borrow().prefs.clone();
        if !p.flag("shortcuts","add_word",true) || p.number("shortcuts","add_word_shortcut",0)!=0 { return Ok(()); }
        let physical = if key.0==17 { if flags.0 & (1<<24)!=0 { 0xa3 } else { 0xa2 } } else { key.0 };
        let side = p.number("shortcuts","add_word_side",0);
        let pure = (side==0 || physical==if side==1 { 0xa2 } else { 0xa3 })
            && !(1..=254).any(|vk| ![17,physical].contains(&vk) && held(VIRTUAL_KEY(vk as u16)));
        let fired = self.ctrl_gesture.borrow_mut().observe(physical,down,flags.0 & (1<<30)!=0,
            super::modifiers::control_epoch(),pure,p.number("shortcuts","add_word_interval",450).clamp(200,1000));
        #[cfg(feature="input-diagnostics")]
        if crate::input_diagnostics::enabled() && [17,0xa2,0xa3].contains(&key.0) {
            crate::input_diagnostics::emit("ctrl.gesture",serde_json::json!({
                "physical_side":if physical==0xa2 {"LEFT"} else {"RIGHT"},
                "configured_side":side,"down":down,"pure":pure,"fired":fired,
                "interval":p.number("shortcuts","add_word_interval",450).clamp(200,1000),
                "observer_epoch":super::modifiers::control_epoch(),"has_context":context.is_some(),
                "message":crate::input_diagnostics::key_metadata(key.0,flags.0)
            }));
        }
        if fired { if let Some(context)=context { self.show_word(context)?; } }
        Ok(())
    }
    fn add_word_shortcut(&self, vk:WPARAM) -> bool {
        let s=self.state.borrow();
        s.prefs.flag("shortcuts","add_word",true) && s.prefs.number("shortcuts","add_word_shortcut",0)==1
            && vk.0==0x7b && held(VK_CONTROL) && held(VK_SHIFT) && !held(VK_MENU)
    }
    fn popup_accepts(&self, vk:WPARAM) -> bool {
        self.word_popup.borrow().as_ref().is_some_and(|p|p.active())
            && crate::word_popup::Popup::accepts(vk.0,held(VK_CONTROL),held(VK_MENU),held(VK_LWIN)||held(VK_RWIN))
    }
    fn observe_modifier(&self, key: WPARAM, down: bool) {
        if !shift(key) && self.state.borrow().shift_tap {
            self.state.borrow_mut().shift_used = true;
        }
        if !down && !shift(key) { self.state.borrow_mut().shift_used = true; }
    }
    pub(super) fn wants(
        self: &Rc<Self>,
        context: &ITfContext,
        vk: WPARAM,
        flags: LPARAM,
    ) -> Result<bool> {
        if system_shortcut(vk) {
            self.diagnostic("wants.system_shortcut",Some(context),Some(vk),Some(0));
            return Ok(false);
        }
        unsafe {
            let status = context.GetStatus()?;
            if status.dwStaticFlags & TF_SD_READONLY != 0 {
                self.diagnostic("wants.readonly",Some(context),Some(vk),Some(0));
                return Ok(false);
            }
            if compartment(context, &GUID_COMPARTMENT_KEYBOARD_DISABLED) {
                self.diagnostic("wants.disabled",Some(context),Some(vk),Some(0));
                return Ok(false);
            }
            if compartment(context, &GUID_COMPARTMENT_EMPTYCONTEXT) {
                self.diagnostic("wants.empty_context",Some(context),Some(vk),Some(0));
                return Ok(false);
            }
            if self.add_word_shortcut(vk) || self.popup_accepts(vk) { return Ok(true); }
            let s = self.state.borrow();
            let p = &s.prefs;
            let physical = if vk.0 == 16 {
                MapVirtualKeyW(((flags.0 >> 16) & 255) as u32, MAPVK_VSC_TO_VK_EX)
            } else {
                vk.0 as u32
            };
            let side = p.number("shortcuts", "shift_side", 0);
            #[cfg(feature = "input-diagnostics")]
            if shift(vk) {
                self.diagnostic_shift_probe(vk,flags,physical,side,p.flag("shortcuts","shift_toggle",true));
            }
            if p.flag("shortcuts", "shift_toggle", true)
                && shift(vk)
                && (side == 0 || physical == if side == 1 { 160 } else { 161 })
                && !held(VK_CONTROL)
                && !held(VK_MENU)
            {
                return Ok(true);
            }
            if (p.flag("shortcuts", "ctrl_space_toggle", false)
                && plain_shortcut(vk, VK_CONTROL, VK_SHIFT))
                || (p.flag("shortcuts", "shift_space_toggle", true)
                    && plain_shortcut(vk, VK_SHIFT, VK_CONTROL))
            {
                return Ok(true);
            }
            let Some(mut key) = translate(vk, flags) else {
                return Ok(false);
            };
            let chinese = s
                .thread
                .as_ref()
                .is_some_and(|t| compartment(t, &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE));
            if s.full_width && !chinese && !held(VK_CONTROL) && !held(VK_MENU) {
                return Ok([1, 2].contains(&key.kind));
            }
            if !chinese {
                return Ok(false);
            }
            if !p.flag("input", "caps_lock_english", false) {
                key.modifiers &= !16;
            }
            if Special::starts(key, p) {
                return Ok(true);
            }
            if key.modifiers & 12 != 0 {
                return Ok(false);
            }
            if key.modifiers & 2 != 0 {
                if s.special.mode != 0 {
                    return Ok(false);
                }
                return Ok((p.flag("shortcuts", "ctrl_punctuation", true)
                    && input::is_punctuation(key))
                    || (p.flag("shortcuts", "ctrl_backslash", false)
                        && key.kind == 1
                        && key.ch == 92
                        && key.modifiers == 2)
                    || (p.flag("shortcuts", "forget_candidate", false)
                        && p.flag("input", "learning", true)
                        && key.kind == 4
                        && key.modifiers == 3
                        && s.snapshot.selected.is_some()));
            }
            if s.associated.handles(key)
                || s.pinyin.handles(key)
                || s.special.mode != 0
                || !s.snapshot.preedit.is_empty()
            {
                return Ok(true);
            }
            Ok(key.kind == 1
                && (key.modifiers & 30 == 0
                    || (p.flag("input", "caps_lock_english", false)
                        && key.modifiers & 16 != 0
                        && key.modifiers & 14 == 0)
                    || (p.flag("shortcuts", "ctrl_punctuation", true)
                        && input::is_punctuation(key))))
        }
    }
    pub(super) fn key_down(
        self: &Rc<Self>,
        context: &ITfContext,
        vk: WPARAM,
        flags: LPARAM,
    ) -> Result<bool> {
        if system_shortcut(vk) {
            return Ok(false);
        }
        if !shift(vk) {
            self.mode_revision.set(self.mode_revision.get() + 1);
            unsafe {
                self.indicator.borrow_mut().hide();
            }
            if self.state.borrow().shift_tap {
                self.state.borrow_mut().shift_used = true;
            }
        }
        if !self.wants(context, vk, flags)? {
            return Ok(false);
        }
        let p = self.state.borrow().prefs.clone();
        if p.flag("shortcuts", "shift_toggle", true) && shift(vk) {
            if self.state.borrow().shift_tap && self.state.borrow().shift_physical != physical_shift(vk,flags) {
                self.state.borrow_mut().shift_used = true;
            }
            if flags.0 & (1 << 30) == 0 && !self.state.borrow().shift_tap {
                let physical = physical_shift(vk, flags);
                let mut s = self.state.borrow_mut();
                s.shift_tap = true;
                s.shift_used = other_held(physical);
                s.shift_physical = physical;
                s.shift_epoch = super::modifiers::epoch();
            }
            return Ok(true);
        }
        if p.flag("shortcuts", "ctrl_space_toggle", false)
            && plain_shortcut(vk, VK_CONTROL, VK_SHIFT)
        {
            if flags.0 & (1 << 30) == 0 {
                self.toggle(Some(context.clone()))?;
            }
            return Ok(true);
        }
        if p.flag("shortcuts", "shift_space_toggle", true)
            && plain_shortcut(vk, VK_SHIFT, VK_CONTROL)
        {
            if flags.0 & (1 << 30) == 0 {
                let width = self.state.borrow().full_width;
                self.state.borrow_mut().full_width = !width;
                self.set_mode(self.chinese())?;
            }
            return Ok(true);
        }
        self.ensure_engine()?;
        self.watch(context)?;
        let mut key = translate(vk, flags).ok_or(E_FAIL)?;
        if !p.flag("input", "caps_lock_english", false) {
            key.modifiers &= !16;
        }
        let state = self.state.borrow().snapshot.clone();
        let closed = state.candidates.is_empty() || (state.prediction && state.selected.is_none());
        if key.kind == 6
            && closed
            && !self.state.borrow().associated.active()
            && self.state.borrow().special.mode == 0
        {
            let escape = p.get("input", "escape", "staged");
            if escape == "ignore" {
                return Ok(true);
            }
            if escape == "clear" {
                let data = self.clone();
                let current = context.clone();
                self.request(context, TF_ES_SYNC | TF_ES_READWRITE, move |cookie| {
                    let mut out = {
                        let mut s = data.state.borrow_mut();
                        s.pinyin.clear();
                        s.raw_reading.clear();
                        output(s.engine.reset(true))?
                    };
                    out.handled = true;
                    data.apply(&current, cookie, out, false)
                })?;
                return Ok(true);
            }
        }
        if p.flag("shortcuts", "ctrl_punctuation", true) && input::is_punctuation(key) {
            let mark = std::ffi::CString::new(input::punctuation(key)).unwrap();
            let data = self.clone();
            let current = context.clone();
            self.request(context, TF_ES_SYNC | TF_ES_READWRITE, move |cookie| {
                let out = {
                    let mut s = data.state.borrow_mut();
                    let mut converted = Snapshot::default();
                    if !s.pinyin.pending.is_empty() {
                        let state = s.snapshot.clone();
                        let super::service::State { pinyin, engine, .. } = &mut *s;
                        converted = output(pinyin.key(
                            ShanjieKey {
                                kind: 2,
                                ch: 0,
                                modifiers: 0,
                            },
                            engine,
                            state,
                            p.flag("input", "alphanumeric", true),
                            true,
                        ))?;
                        if !pinyin.pending.is_empty() {
                            drop(s);
                            return data.apply(&current, cookie, converted, false);
                        }
                    }
                    s.raw_reading.clear();
                    s.associated.clear();
                    let mut out = output(s.engine.perform(crate::broker::Op::Punctuation(
                        mark.to_str().unwrap().into(),
                    )))?;
                    out.commit = converted.commit + &out.commit;
                    out
                };
                data.apply(&current, cookie, out, false)
            })?;
            return Ok(true);
        }
        let space_candidates = p.flag("candidates", "space_operates", false);
        let trigger = if space_candidates {
            p.get("candidates", "open", "down")
        } else {
            "down"
        };
        if key.kind == 2
            && key.modifiers == 0
            && self.state.borrow().special.mode == 0
            && self.state.borrow().pinyin.pending.is_empty()
            && !input::pending(&state)
            && !self.state.borrow().engine.mixed_pending()
            && !(space_candidates && (!closed || trigger != "down"))
        {
            let data = self.clone();
            let current = context.clone();
            self.request(context, TF_ES_SYNC | TF_ES_READWRITE, move |cookie| {
                let out = {
                    let mut s = data.state.borrow_mut();
                    s.associated.clear();
                    s.association_caret = None;
                    s.raw_reading.clear();
                    if p.flag("input", "space_commits", false) {
                        let mut out = output(s.engine.reset(false))?;
                        out.handled = true;
                        if p.flag("input", "space_after_commit", false) {
                            out.commit.push(' ');
                        }
                        out
                    } else {
                        output(s.engine.perform(crate::broker::Op::Space))?
                    }
                };
                data.apply(&current, cookie, out, false)
            })?;
            return Ok(true);
        }
        if self.state.borrow().associated.handles(key) {
            if key.kind == 2
                && (!p.flag("candidates", "space_pages", true)
                    || (!p.flag("candidates", "wrap_pages", true)
                        && state.first + 9 >= state.total))
            {
                return Ok(true);
            }
            let data = self.clone();
            let current = context.clone();
            self.request(context, TF_ES_SYNC | TF_ES_READWRITE, move |cookie| {
                let out = {
                    let mut s = data.state.borrow_mut();
                    if key.kind == 1 {
                        pick_associated(&mut s, key.ch - 49)?
                    } else {
                        s.associated.key(key)
                    }
                };
                data.apply(&current, cookie, out, false)
            })?;
            return Ok(true);
        }
        if self.state.borrow().associated.active() {
            let mut s = self.state.borrow_mut();
            s.associated.clear();
            s.association_caret = None;
            s.snapshot.associated = false;
            s.snapshot.candidates.clear();
            if let Some(c) = &s.candidates {
                c.data.close();
            }
        }
        if self.state.borrow().pinyin.handles(key) {
            let data = self.clone();
            let current = context.clone();
            self.request(context, TF_ES_SYNC | TF_ES_READWRITE, move |cookie| {
                let out = {
                    let mut s = data.state.borrow_mut();
                    let state = s.snapshot.clone();
                    let super::service::State { pinyin, engine, .. } = &mut *s;
                    output(pinyin.key(
                        key,
                        engine,
                        state,
                        p.flag("input", "alphanumeric", true),
                        p.flag("input", "pinyin_space_converts", false),
                    ))?
                };
                data.apply(&current, cookie, out, false)
            })?;
            return Ok(true);
        }
        if self.state.borrow().special.mode != 0 || Special::starts(key, &p) {
            let data = self.clone();
            let current = context.clone();
            self.request(context, TF_ES_SYNC | TF_ES_READWRITE, move |cookie| {
                let out = {
                    let mut s = data.state.borrow_mut();
                    let pending = if s.special.mode == 0 {
                        output(s.engine.reset(false))?
                    } else {
                        Snapshot::default()
                    };
                    let mut out = s.special.key(key, &p);
                    out.commit = pending.commit + &out.commit;
                    out
                };
                data.apply(&current, cookie, out, false)
            })?;
            return Ok(true);
        }
        let selected = !state.candidates.is_empty()
            && state.selected.is_some()
            && key.modifiers == 0
            && (key.kind == 3 || (key.kind == 1 && (49..=57).contains(&key.ch)));
        key = input::candidate_key(
            key,
            &state,
            p.get("profile", "interaction", "windows") == "windows",
        );
        if p.flag("shortcuts", "forget_candidate", false)
            && p.flag("input", "learning", true)
            && key.kind == 4
            && key.modifiers == 3
            && state.selected.is_some()
        {
            key.modifiers = 8;
        }
        let english = p.flag("input", "caps_lock_english", false)
            && key.modifiers & 16 != 0
            && key.modifiers & 14 == 0
            && [1, 2].contains(&key.kind);
        let mixed_letter = (p.flag("input", "mixed_input", false) || (p.flag("input","upstream_english",false) && crate::installation::layout(p.number("input","keyboard",0),self.state.borrow().profile_layout,crate::installation::available())==0))
            && p.number("input", "keyboard", 0) < 3
            && key.kind == 1
            && (key.ch as u8).is_ascii_alphabetic()
            && key.modifiers & 14 == 0
            && !english;
        let latin = if mixed_letter {
            let mut keyboard = [0u8; 256];
            let mut text = [0u16; 4];
            unsafe {
                GetKeyboardState(&mut keyboard)?;
                let n = ToUnicodeEx(
                    vk.0 as u32,
                    ((flags.0 >> 16) & 255) as u32,
                    &keyboard,
                    &mut text,
                    4,
                    Some(GetKeyboardLayout(0)),
                );
                (n == 1)
                    .then(|| char::from_u32(text[0] as u32))
                    .flatten()
                    .filter(|ch| ch.is_ascii_alphabetic())
            }
        } else {
            None
        };
        let ascii = !mixed_letter
            && key.kind == 1
            && key.modifiers == 1
            && !p.shift_punctuation(key.ch as u8 as char);
        if (self.state.borrow().full_width && !self.chinese()) || english || ascii {
            let mut keyboard = [0u8; 256];
            let mut text = [0u16; 4];
            let length = unsafe {
                GetKeyboardState(&mut keyboard)?;
                ToUnicodeEx(
                    vk.0 as u32,
                    ((flags.0 >> 16) & 255) as u32,
                    &keyboard,
                    &mut text,
                    4,
                    Some(GetKeyboardLayout(0)),
                )
            };
            if length != 1 || !(32..=126).contains(&text[0]) {
                return Ok(false);
            }
            let ch = char::from_u32(text[0] as u32).unwrap();
            let data = self.clone();
            let current = context.clone();
            self.request(context, TF_ES_SYNC | TF_ES_READWRITE, move |cookie| {
                let mut out = if ascii && !english && data.chinese() {
                    output(data.state.borrow_mut().engine.perform(crate::broker::Op::Punctuation(ch.to_string())))?
                } else {
                    let mut out = output(data.state.borrow_mut().engine.reset(false))?;
                    out.commit.push(ch);
                    out
                };
                out.handled = true;
                data.apply(&current, cookie, out, false)
            })?;
            return Ok(true);
        }
        if !state.prediction
            && key.kind == 2
            && !state.candidates.is_empty()
            && (!p.flag("candidates", "space_pages", true)
                || (!p.flag("candidates", "wrap_pages", true)
                    && state.first + (state.selected.unwrap_or(0) / 9 + 1) as u32 * 9
                        >= state.total))
        {
            return Ok(true);
        }
        let delete = p.flag("candidates", "backspace_deletes", false)
            && !state.prediction
            && !state.candidates.is_empty()
            && key.kind == 4
            && key.modifiers == 0;
        let mixed_pending = self.state.borrow().engine.mixed_pending();
        let bypass = key.modifiers == 0
            && closed
            && !state.preedit.is_empty()
            && !input::pending(&state)
            && !mixed_pending
            && key.kind == 10
            && trigger == "space";
        let cycle = p.flag("candidates", "up_cycles", true)
            && key.kind == 9
            && key.modifiers == 0
            && closed
            && !state.preedit.is_empty()
            && !input::pending(&state);
        let cycle = cycle && !mixed_pending;
        let handled = Rc::new(Cell::new(false));
        let result_handled = handled.clone();
        let data = self.clone();
        let current = context.clone();
        self.request(
            context,
            TF_ES_SYNC | TF_ES_READWRITE,
            move |cookie| unsafe {
                if data.state.borrow().snapshot.preedit.is_empty()
                    && p.flag("input", "context", true)
                {
                    let left = range(selection(&current, cookie)?);
                    left.Collapse(cookie, TF_ANCHOR_START)?;
                    let mut moved = 0;
                    left.ShiftStart(cookie, -8, &mut moved, std::ptr::null())?;
                    let mut text = [0u16; 8];
                    let mut count = 0;
                    left.GetText(cookie, 0, &mut text, &mut count)?;
                    let mut units = &text[..count as usize];
                    if units.first().is_some_and(|u| (0xdc00..=0xdfff).contains(u)) {
                        units = &units[1..];
                    }
                    if units.last().is_some_and(|u| (0xd800..=0xdbff).contains(u)) {
                        units = &units[..units.len() - 1];
                    }
                    let value = std::ffi::CString::new(
                        String::from_utf16(units).map_err(|_| Error::from_hresult(E_INVALIDARG))?,
                    )
                    .unwrap();
                    output(
                        data.state
                            .borrow_mut()
                            .engine
                            .perform(crate::broker::Op::Context(value.to_str().unwrap().into())),
                    )?;
                }
                let out = {
                    let mut s = data.state.borrow_mut();
                    let mut out = if bypass {
                        let mut out = output(s.engine.reset(false))?;
                        out.handled =
                            key.kind == 2 && !p.flag("input", "space_after_commit", false);
                        out
                    } else if cycle {
                        output(s.engine.perform(crate::broker::Op::Cycle))?
                    } else {
                        let mut raw = s.raw_reading.clone();
                        if key.kind == 4 {
                            raw.pop();
                        }
                        if key.kind == 1 && key.modifiers == 0 {
                            raw.push(key.ch as u8 as char);
                        } else if key.kind == 2 && key.modifiers == 0 {
                            raw.push(' ');
                        }
                        let out = if let Some(out) = s.engine.key_with_latin(key, latin) {
                            out
                        } else {
                            // Raw-reading fallback is only for invalid phonetic input.
                            // Transport failure or a busy learning writer retains the key.
                            if s.engine.failure() != Some(crate::engine::Failure::Failed) {
                                return Err(E_FAIL.into());
                            }
                            if !p.flag("input", "alphanumeric", true)
                                || raw.is_empty()
                                || !input::pending(&s.snapshot)
                            {
                                return Err(E_FAIL.into());
                            }
                            let mut units: Vec<_> = s.snapshot.preedit.encode_utf16().collect();
                            let end = (s.snapshot.cursor as usize).min(units.len());
                            let mut start = end;
                            while start > 0
                                && ((0x3105..=0x312f).contains(&units[start - 1])
                                    || [0x2ca, 0x2c7, 0x2cb, 0x2d9].contains(&units[start - 1]))
                            {
                                start -= 1;
                            }
                            units.splice(start..end, raw.encode_utf16());
                            Snapshot {
                                handled: true,
                                commit: String::from_utf16(&units).unwrap(),
                                ..Default::default()
                            }
                        };
                        s.raw_reading = if input::pending(&out) {
                            raw
                        } else {
                            String::new()
                        };
                        out
                    };
                    if delete {
                        out = output(s.engine.key(key))?;
                    }
                    if key.kind == 2 && key.modifiers == 0
                        && !p.flag("input", "space_inputs_zhuyin", true)
                        && p.flag("input", "space_commits", false)
                        && !input::pending(&out) && out.preedit.ends_with(' ') {
                        let committed = out.commit;
                        out = output(s.engine.reset(false))?;
                        out.commit = committed + &out.commit;
                    }
                    if p.flag("candidates", "enter_commits", true)
                        && selected
                        && key.kind == 3
                        && key.modifiers == 0
                        && out.candidates.is_empty()
                        && !input::pending(&out)
                        && !out.preedit.is_empty()
                    {
                        let committed = out.commit;
                        out = output(s.engine.key(key))?;
                        out.commit = committed + &out.commit;
                    }
                    out
                };
                let was_handled = out.handled;
                data.apply(
                    &current,
                    cookie,
                    out,
                    selected && !state.prediction && key.kind != 3,
                )?;
                result_handled.set(was_handled);
                Ok(())
            },
        )?;
        Ok(handled.get())
    }
}
impl ITfKeyEventSink_Impl for Service_Impl {
    fn OnSetFocus(&self, focused: BOOL) -> Result<()> {
        self.data.diagnostic("key_focus",None,None,Some(focused.0));
        if !focused.as_bool() {
            self.data.cancel()?;
        }
        Ok(())
    }
    fn OnTestKeyDown(&self, context: Ref<ITfContext>, key: WPARAM, flags: LPARAM) -> Result<BOOL> {
        self.data.diagnostic("test_down.enter",context.as_ref(),Some(key),None);
        self.data.key_handoff(context.as_ref())?;
        self.data.observe_modifier(key, true);
        self.data.ctrl_event(context.as_ref(),key,flags,true)?;
        if !self.data.popup_accepts(key) && ![16,17,160,161,162,163].contains(&key.0) {
            self.data.close_word();
        }
        if system_shortcut(key) {
            return Ok(false.into());
        }
        let result=self.data.wants(context.ok()?, key, flags);
        self.data.diagnostic("test_down.exit",context.as_ref(),Some(key),Some(match &result{Ok(v)=>*v as i32,Err(e)=>e.code().0}));
        Ok(result?.into())
    }
    fn OnTestKeyUp(&self, context: Ref<ITfContext>, key: WPARAM, flags: LPARAM) -> Result<BOOL> {
        self.data.diagnostic("test_up.enter",context.as_ref(),Some(key),None);
        self.data.key_handoff(context.as_ref())?;
        self.data.observe_modifier(key, false);
        self.data.ctrl_event(context.as_ref(),key,flags,false)?;
        let result=shift(key) && self.data.state.borrow().shift_tap;
        self.data.diagnostic("test_up.exit",context.as_ref(),Some(key),Some(result as i32));
        Ok(result.into())
    }
    fn OnKeyDown(&self, context: Ref<ITfContext>, key: WPARAM, flags: LPARAM) -> Result<BOOL> {
        self.data.diagnostic("down.enter",context.as_ref(),Some(key),None);
        self.data.key_handoff(context.as_ref())?;
        self.data.observe_modifier(key, true);
        self.data.ctrl_event(context.as_ref(),key,flags,true)?;
        if self.data.add_word_shortcut(key) {
            if flags.0 & (1<<30)==0 { self.data.show_word(context.ok()?)?; }
            return Ok(true.into());
        }
        if self.data.popup_accepts(key) {
            let ch=translate(key,flags).and_then(|k|char::from_u32(k.ch));
            if let Some(popup)=self.data.word_popup.borrow().as_ref() { popup.key(key.0,ch,held(VK_CONTROL)); }
            return Ok(true.into());
        }
        if system_shortcut(key) {
            return Ok(false.into());
        }
        let result=self.data.dispatch_key(context.ok()?, key, flags);
        self.data.diagnostic("down.exit",context.as_ref(),Some(key),Some(match &result{Ok(v)=>*v as i32,Err(e)=>e.code().0}));
        match result {
            Ok(handled) => Ok(handled.into()),
            Err(e) => {
                super::trace(format_args!("key: {e}"));
                Ok(true.into())
            }
        }
    }
    fn OnKeyUp(&self, context: Ref<ITfContext>, key: WPARAM, flags: LPARAM) -> Result<BOOL> {
        self.data.diagnostic("up.enter",context.as_ref(),Some(key),None);
        self.data.key_handoff(context.as_ref())?;
        self.data.observe_modifier(key, false);
        self.data.ctrl_event(context.as_ref(),key,flags,false)?;
        if shift(key) && self.data.state.borrow().shift_tap {
            #[cfg(feature="input-diagnostics")]
            if crate::input_diagnostics::enabled(){
                let s=self.data.state.borrow();
                crate::input_diagnostics::emit("shift.decision",serde_json::json!({
                    "physical_released":physical_shift(key,flags),"physical_started":s.shift_physical,
                    "used":s.shift_used,"epoch_started":s.shift_epoch,"epoch_now":super::modifiers::epoch(),
                    "other_key_held":other_held(s.shift_physical)}));
            }
            let toggle = {
                let mut s = self.data.state.borrow_mut();
                s.shift_tap = false;
                let toggle = !s.shift_used && s.shift_epoch == super::modifiers::epoch()
                    && s.shift_physical == physical_shift(key, flags) && !other_held(s.shift_physical);
                s.shift_used = false;
                toggle
            };
            if toggle {
                self.data.diagnostic("shift.toggle",context.as_ref(),Some(key),Some(1));
                self.data.toggle(context.as_ref().cloned())?;
            } else {
                self.data.diagnostic("shift.cancelled",context.as_ref(),Some(key),Some(0));
            }
            self.data.diagnostic("up.exit",context.as_ref(),Some(key),Some(1));
            return Ok(true.into());
        }
        self.data.diagnostic("up.exit",context.as_ref(),Some(key),Some(0));
        Ok(false.into())
    }
    fn OnPreservedKey(&self, _: Ref<ITfContext>, _: *const GUID) -> Result<BOOL> {
        Ok(false.into())
    }
}
