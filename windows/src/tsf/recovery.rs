//! Keep failed keys in memory, tied to the original field and input generation.
use super::{
    keys::shift,
    service::{range, selection, Data, State},
};
use crate::{
    engine::{Engine, Failure, Snapshot},
    input::{Associated, Pinyin, Special},
};
use std::{
    collections::VecDeque,
    rc::Rc,
    time::{Duration, Instant},
};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        UI::{Input::KeyboardAndMouse::*, TextServices::*},
    },
};
#[derive(Clone)]
struct Key {
    context: ITfContext,
    generation: u64,
    vk: WPARAM,
    flags: LPARAM,
    keyboard: [u8; 256],
    pick: Option<(u32, Snapshot)>,
}
#[derive(Default)]
pub(super) struct Recovery {
    keys: VecDeque<Key>,
    deadline: Option<Instant>,
    pub refresh: bool,
}
impl Recovery {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}
struct Checkpoint {
    pinyin: Pinyin,
    special: Special,
    associated: Associated,
    caret: Option<ITfRange>,
    raw: String,
    snapshot: Snapshot,
}
impl Checkpoint {
    fn capture(s: &State) -> Self {
        Self {
            pinyin: s.pinyin.clone(),
            special: s.special.clone(),
            associated: s.associated.clone(),
            caret: s.association_caret.clone(),
            raw: s.raw_reading.clone(),
            snapshot: s.snapshot.clone(),
        }
    }
    fn restore(self, s: &mut State) {
        s.pinyin = self.pinyin;
        s.special = self.special;
        s.associated = self.associated;
        s.association_caret = self.caret;
        s.raw_reading = self.raw;
        s.snapshot = self.snapshot;
    }
}
struct Keyboard([u8; 256]);
impl Drop for Keyboard {
    fn drop(&mut self) {
        unsafe {
            let _ = SetKeyboardState(&self.0);
        }
    }
}
impl Data {
    fn attempt_key(self: &Rc<Self>, key: &Key, cookie: Option<u32>) -> Result<bool> {
        if self.generation.get() != key.generation
            || self.state.borrow().context.as_ref() != Some(&key.context)
        {
            return Ok(true);
        }
        if self
            .retiring
            .borrow()
            .iter()
            .any(|(_, context)| context == &key.context)
        {
            return Err(E_FAIL.into());
        }
        self.ensure_engine()?;
        if let Some((_, old)) = &key.pick {
            let current = &self.state.borrow().snapshot;
            if current.preedit != old.preedit
                || current.cursor != old.cursor
                || current.candidates != old.candidates
                || current.first != old.first
                || current.total != old.total
                || current.prediction != old.prediction
            {
                return Ok(true);
            }
        }
        let checkpoint = Checkpoint::capture(&self.state.borrow());
        self.application_written.set(false);
        self.state.borrow_mut().engine.begin_attempt();
        let mut previous = [0; 256];
        unsafe {
            GetKeyboardState(&mut previous)?;
            SetKeyboardState(&key.keyboard)?;
        }
        let _keyboard = Keyboard(previous);
        let result = if let Some((index, _)) = key.pick.as_ref() {
            if let Some(cookie) = cookie {
                self.pick_at(&key.context, cookie, *index).map(|_| true)
            } else {
                let data = self.clone();
                let context = key.context.clone();
                let index = *index;
                self.request_current(&key.context, TF_ES_SYNC | TF_ES_READWRITE, move |cookie| {
                    data.pick_at(&context, cookie, index)
                })
                .map(|_| true)
            }
        } else {
            self.key_down(&key.context, key.vk, key.flags)
        };
        let written = self.application_written.get();
        self.state
            .borrow_mut()
            .engine
            .finish_attempt(result.is_ok() || written);
        if result.is_err() && !written {
            checkpoint.restore(&mut self.state.borrow_mut());
        }
        if written {
            Ok(true)
        } else {
            result
        }
    }
    fn schedule_retry(&self) {
        if let Some(watcher) = self.state.borrow().watcher.as_ref() {
            watcher.retry();
        }
    }
    fn recovery_hint(self: &Rc<Self>, context: &ITfContext, expired: bool) {
        let failure = self.state.borrow().engine.failure();
        let message = if self
            .retiring
            .borrow()
            .iter()
            .any(|(_, retiring)| retiring == context)
        {
            "正在完成上一段組字，後續輸入已保留。"
        } else if failure == Some(Failure::Ambiguous) {
            "上次輸入結果尚未確認，組字已保留；Esc 取消後重試。"
        } else if expired {
            "組字與待處理按鍵已保留；按任一鍵重試，Esc 取消。"
        } else if failure == Some(Failure::Busy) {
            "選字資料暫時忙碌，正在重試。"
        } else {
            "核心暫時無法回應，組字已保留；正在重試。"
        };
        crate::bridge::failure(message.to_owned());
        let data = self.clone();
        let current = context.clone();
        let message = message.to_owned();
        let _ = self.request_current(
            context,
            TF_ES_READ | TF_ES_ASYNCDONTCARE,
            move |cookie| unsafe {
                if data.recovery.borrow().is_empty() {
                    return Ok(());
                }
                let (candidate, mut snapshot) = {
                    let state = data.state.borrow();
                    (
                        state.candidates.as_ref().map(|c| c.data.clone()),
                        state.snapshot.clone(),
                    )
                };
                if let Some(candidate) = candidate {
                    let caret = range(selection(&current, cookie)?);
                    caret.Collapse(cookie, TF_ANCHOR_END)?;
                    snapshot.candidates = vec![message];
                    snapshot.selected = None;
                    snapshot.columns = 0;
                    snapshot.first = 0;
                    snapshot.total = 1;
                    snapshot.reading_hint = true;
                    snapshot.prediction = true;
                    candidate.update(&current, cookie, &caret, snapshot)?;
                }
                Ok(())
            },
        );
    }
    pub(super) fn dispatch_key(
        self: &Rc<Self>,
        context: &ITfContext,
        vk: WPARAM,
        flags: LPARAM,
    ) -> Result<bool> {
        if !self.wants(context, vk, flags)? {
            return Ok(false);
        }
        if shift(vk) {
            return self.key_down(context, vk, flags);
        }
        self.watch(context)?;
        if !self.recovery.borrow().is_empty() && vk.0 == VK_ESCAPE.0 as usize {
            self.recovery.borrow_mut().clear();
            if let Some(watcher) = self.state.borrow().watcher.as_ref() {
                watcher.stop_retry();
            }
            {
                let mut state = self.state.borrow_mut();
                state.engine = Engine::default();
                state.pinyin.clear();
                state.special.clear();
                state.associated.clear();
                state.raw_reading.clear();
            }
            let data = self.clone();
            let current = context.clone();
            self.request(context, TF_ES_SYNC | TF_ES_READWRITE, move |cookie| {
                data.apply(
                    &current,
                    cookie,
                    Snapshot {
                        handled: true,
                        ..Default::default()
                    },
                    false,
                )
            })?;
            return Ok(true);
        }
        let mut keyboard = [0; 256];
        unsafe {
            GetKeyboardState(&mut keyboard)?;
        }
        let key = Key {
            context: context.clone(),
            generation: self.generation.get(),
            vk,
            flags,
            keyboard,
            pick: None,
        };
        if self.recovery.borrow().is_empty() {
            if let Ok(handled) = self.attempt_key(&key, None) {
                return Ok(handled);
            }
        }
        let mut recovery = self.recovery.borrow_mut();
        recovery.keys.push_back(key);
        if recovery
            .deadline
            .is_none_or(|deadline| Instant::now() >= deadline)
        {
            recovery.deadline = Some(Instant::now() + Duration::from_secs(5));
        }
        drop(recovery);
        self.recovery_hint(context, false);
        self.schedule_retry();
        Ok(true)
    }
    pub(super) fn dispatch_pick(
        self: &Rc<Self>,
        context: &ITfContext,
        cookie: u32,
        index: u32,
    ) -> Result<()> {
        let key = Key {
            context: context.clone(),
            generation: self.generation.get(),
            vk: WPARAM(0),
            flags: LPARAM(0),
            keyboard: [0; 256],
            pick: Some((index, self.state.borrow().snapshot.clone())),
        };
        if self.recovery.borrow().is_empty() && self.attempt_key(&key, Some(cookie)).is_ok() {
            return Ok(());
        }
        let mut recovery = self.recovery.borrow_mut();
        recovery.keys.push_back(key);
        if recovery
            .deadline
            .is_none_or(|deadline| Instant::now() >= deadline)
        {
            recovery.deadline = Some(Instant::now() + Duration::from_secs(5));
        }
        drop(recovery);
        self.recovery_hint(context, false);
        self.schedule_retry();
        Ok(())
    }
    pub(super) fn retry_pending(self: &Rc<Self>) {
        // Bound each timer dispatch so a burst does not monopolize the UI thread.
        let started = Instant::now();
        for _ in 0..16 {
            let Some(key) = self.recovery.borrow().keys.front().cloned() else {
                let refresh = self.recovery.borrow().refresh;
                self.recovery.borrow_mut().clear();
                if refresh {
                    let _ = self.refresh();
                }
                return;
            };
            if key.generation != self.generation.get()
                || self.state.borrow().context.as_ref() != Some(&key.context)
            {
                self.recovery.borrow_mut().clear();
                return;
            }
            if self
                .recovery
                .borrow()
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                self.recovery_hint(&key.context, true);
                return;
            }
            if self.attempt_key(&key, None).is_err() {
                self.recovery_hint(&key.context, false);
                if self.state.borrow().engine.failure() != Some(Failure::Ambiguous) {
                    self.schedule_retry();
                }
                return;
            }
            let generation = self.generation.get();
            let mut recovery = self.recovery.borrow_mut();
            recovery.keys.pop_front();
            // Following keys are a new composition in the same valid field.
            for next in &mut recovery.keys {
                if next.generation == key.generation {
                    next.generation = generation;
                }
            }
            drop(recovery);
            if started.elapsed() > Duration::from_millis(30) {
                self.schedule_retry();
                return;
            }
        }
        self.schedule_retry();
    }
}
