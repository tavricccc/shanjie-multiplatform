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
impl Data {
    fn wants(self: &Rc<Self>, context: &ITfContext, vk: WPARAM, flags: LPARAM) -> Result<bool> {
        if system_shortcut(vk) {
            return Ok(false);
        }
        unsafe {
            let status = context.GetStatus()?;
            if status.dwStaticFlags & TF_SD_READONLY != 0
                || compartment(context, &GUID_COMPARTMENT_KEYBOARD_DISABLED)
                || compartment(context, &GUID_COMPARTMENT_EMPTYCONTEXT)
            {
                return Ok(false);
            }
            let s = self.state.borrow();
            let p = &s.prefs;
            let physical = if vk.0 == 16 {
                MapVirtualKeyW(((flags.0 >> 16) & 255) as u32, MAPVK_VSC_TO_VK_EX)
            } else {
                vk.0 as u32
            };
            let side = p.number("shortcuts", "shift_side", 0);
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
    fn key_down(self: &Rc<Self>, context: &ITfContext, vk: WPARAM, flags: LPARAM) -> Result<bool> {
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
            if flags.0 & (1 << 30) == 0 {
                let mut s = self.state.borrow_mut();
                s.shift_tap = true;
                s.shift_used = false;
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
                    let mut out = output(s.engine.operation(|h, o| unsafe {
                        shanjie_core::ffi::shanjie_windows_punctuation(h, mark.as_ptr(), o)
                    }))?;
                    out.commit = converted.commit + &out.commit;
                    out
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
        let ascii = key.kind == 1
            && key.modifiers == 1
            && (!p.flag("input", "shift_punctuation", false)
                || !",./1;[]90`\\'=".contains(key.ch as u8 as char));
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
                let mut out = output(data.state.borrow_mut().engine.reset(false))?;
                out.handled = true;
                out.commit.push(ch);
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
        let trigger = p.get("candidates", "open", "down");
        let bypass = key.modifiers == 0
            && closed
            && !state.preedit.is_empty()
            && !input::pending(&state)
            && ((key.kind == 2 && trigger == "down") || (key.kind == 10 && trigger == "space"));
        let cycle = p.flag("candidates", "up_cycles", true)
            && key.kind == 9
            && key.modifiers == 0
            && closed
            && !state.preedit.is_empty()
            && !input::pending(&state);
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
                    shanjie_core::ffi::shanjie_engine_set_left_context(
                        data.state.borrow().engine.handle,
                        value.as_ptr(),
                    );
                }
                let out = {
                    let mut s = data.state.borrow_mut();
                    let mut out = if bypass {
                        let mut out = output(s.engine.reset(false))?;
                        out.handled =
                            key.kind == 2 && !p.flag("input", "space_after_commit", false);
                        out
                    } else if cycle {
                        output(s.engine.operation(|h, o| {
                            shanjie_core::ffi::shanjie_windows_cycle_candidate(h, o)
                        }))?
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
                        let out = if let Some(out) = s.engine.key(key) {
                            out
                        } else {
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
        if !focused.as_bool() {
            self.data.cancel()?;
        }
        Ok(())
    }
    fn OnTestKeyDown(&self, context: Ref<ITfContext>, key: WPARAM, flags: LPARAM) -> Result<BOOL> {
        if !shift(key) && self.data.state.borrow().shift_tap {
            self.data.state.borrow_mut().shift_used = true;
        }
        if system_shortcut(key) {
            return Ok(false.into());
        }
        Ok(self.data.wants(context.ok()?, key, flags)?.into())
    }
    fn OnTestKeyUp(&self, _: Ref<ITfContext>, key: WPARAM, _: LPARAM) -> Result<BOOL> {
        Ok((shift(key) && self.data.state.borrow().shift_tap).into())
    }
    fn OnKeyDown(&self, context: Ref<ITfContext>, key: WPARAM, flags: LPARAM) -> Result<BOOL> {
        if system_shortcut(key) {
            return Ok(false.into());
        }
        match self.data.key_down(context.ok()?, key, flags) {
            Ok(handled) => Ok(handled.into()),
            Err(_) => {
                self.data.cancel()?;
                Ok(false.into())
            }
        }
    }
    fn OnKeyUp(&self, context: Ref<ITfContext>, key: WPARAM, _: LPARAM) -> Result<BOOL> {
        if shift(key) && self.data.state.borrow().shift_tap {
            let toggle = {
                let mut s = self.data.state.borrow_mut();
                s.shift_tap = false;
                let toggle = !s.shift_used;
                s.shift_used = false;
                toggle
            };
            if toggle {
                self.data.toggle(context.as_ref().cloned())?;
            }
            return Ok(true.into());
        }
        Ok(false.into())
    }
    fn OnPreservedKey(&self, _: Ref<ITfContext>, _: *const GUID) -> Result<BOOL> {
        Ok(false.into())
    }
}
