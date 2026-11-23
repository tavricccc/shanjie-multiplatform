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
    cell::Cell,
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
    toggle: bool,
}
impl Key {
    fn is_forward_tab(&self) -> bool {
        self.vk.0 == VK_TAB.0 as usize && self.keyboard[VK_SHIFT.0 as usize] & 0x80 == 0
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Waiting {
    ForwardTab,
    Other,
}
#[derive(Default)]
pub(super) struct Recovery {
    keys: VecDeque<Key>,
    deadline: Option<Instant>,
    waiting_for_lock: Option<Waiting>,
    pub refresh: bool,
}
impl Recovery {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.waiting_for_lock.is_none()
    }
    pub(super) fn rebase(&mut self, from:u64, to:u64) {
        for key in &mut self.keys {if key.generation==from {key.generation=to;}}
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
    fn attempt_key(self: &Rc<Self>, key: &Key, cookie: u32) -> Result<bool> {
        if self.generation.get() != key.generation
            || self.state.borrow().context.as_ref() != Some(&key.context)
        {
            return Ok(true);
        }
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
        let result = self.update_privacy(&key.context, cookie).and_then(|_| if key.toggle {
            self.toggle_in_session(&key.context, cookie, !self.chinese()).map(|_| true)
        } else if let Some((index, _)) = key.pick.as_ref() {
            self.pick_at(&key.context, cookie, *index).map(|_| true)
        } else {
            self.key_down(&key.context, cookie, key.vk, key.flags)
        });
        let written = self.application_written.get();
        self.state
            .borrow_mut()
            .engine
            .finish_attempt(result.is_ok() || written);
        if result.is_err() && !written {
            checkpoint.restore(&mut self.state.borrow_mut());
        }
        // Text already written must not be retried, so a written key counts as eaten. Tab in the Shanjie
        // interaction is the exception: as in upstream, the composition is sent and the App still gets the key.
        let tab_passes = matches!(result, Ok(false))
            && key.vk.0 == VK_TAB.0 as usize
            && self.state.borrow().prefs.get("profile", "interaction", "windows") != "windows";
        if written && !tab_passes {
            Ok(true)
        } else {
            result
        }
    }
    // One outstanding TSF lock request owns its key. Only a rejected request or
    // a failed execution may return that key to the recovery queue.
    fn request_key(self: &Rc<Self>, key: &Key, allow_async: bool) -> Result<Option<bool>> {
        if let Err(error) = self.retry_retirement(&key.context, false).and_then(|_| self.ensure_engine()) {
            return if allow_async { Err(error) } else { Ok(Some(false)) };
        }
        let result = Rc::new(Cell::new(None));
        let deferred = Rc::new(Cell::new(false));
        let request = |flags| {
            let data = self.clone();
            let result = result.clone();
            let deferred = deferred.clone();
            let key = key.clone();
            let context = key.context.clone();
            self.request_status(&context, flags | TF_ES_READWRITE, move |cookie| {
                if data.generation.get() != key.generation
                    || data.state.borrow().context.as_ref() != Some(&key.context)
                {
                    result.set(Some(true));
                    return Ok(());
                }
                let outcome = data.attempt_key(&key, cookie);
                result.set(Some(outcome.as_ref().copied().unwrap_or(false)));
                if deferred.get() {
                    data.recovery.borrow_mut().waiting_for_lock = None;
                    if outcome.is_err() {
                        data.keep_failed_key(key);
                    } else {
                        data.recovery.borrow_mut().rebase(key.generation, data.generation.get());
                        data.schedule_retry();
                    }
                }
                outcome.map(|_| ())
            })
        };
        let respond = |flags| request(flags).and_then(|status| { status.ok()?; Ok(status) });
        let status = match respond(TF_ES_SYNC) {
            Err(_) if result.get().is_none() && !allow_async => return Ok(Some(false)),
            Err(error) if result.get().is_none() && [TF_E_SYNCHRONOUS, TF_E_LOCKED].contains(&error.code()) => respond(TF_ES_ASYNC)?,
            other => other?,
        };
        if status == TF_S_ASYNC {
            deferred.set(true);
            let waiting = if key.is_forward_tab() { Waiting::ForwardTab } else { Waiting::Other };
            self.recovery.borrow_mut().waiting_for_lock = Some(waiting);
            Ok(None)
        } else {
            Ok(Some(result.get().ok_or(E_FAIL)?))
        }
    }
    fn keep_failed_key(self: &Rc<Self>, key: Key) {
        let mut recovery = self.recovery.borrow_mut();
        recovery.deadline.get_or_insert_with(|| Instant::now() + Duration::from_secs(5));
        let context = key.context.clone();
        recovery.keys.push_front(key);
        drop(recovery);
        self.recovery_hint(&context, false);
        if self.state.borrow().engine.failure() != Some(Failure::Ambiguous) {
            self.schedule_retry();
        }
    }
    pub(super) fn schedule_retry(&self) {
        if let Some(watcher) = self.state.borrow().watcher.as_ref() {
            watcher.retry();
        }
    }
    pub(super) fn recovery_hint(self: &Rc<Self>, context: &ITfContext, expired: bool) {
        let failure = self.state.borrow().engine.failure();
        let message = if self.is_retiring(context) {
            if expired { "上一段組字尚未完成；按任一鍵重試，Esc 保留畫面文字並取消待處理按鍵。" } else { "正在完成上一段組字，後續輸入已保留。" }
        } else if failure == Some(Failure::Ambiguous) {
            "上次輸入結果尚未確認，組字已保留；Esc 取消後重試。"
        } else if expired && self.profile_update.borrow().is_some() {
            "設定及待處理按鍵已保留；按任一鍵重試，Esc 取消組字。"
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
                if data.recovery.borrow().is_empty() && data.profile_update.borrow().is_none() && !data.is_retiring(&current) {
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
            self.shift_down(vk, flags);
            return Ok(true);
        }
        self.watch(context)?;
        if vk.0 == VK_ESCAPE.0 as usize && self.recovery.borrow().waiting_for_lock.is_none()
            && (self.is_retiring(context) || self.recovery.borrow().deadline.is_some() || self.profile_update.borrow().is_some())
        {
            self.profile_update.borrow_mut().take();
            self.recovery.borrow_mut().clear();
            if self.is_retiring(context) {
                if self.retry_retirement(context, true).is_err() {
                    self.recovery_hint(context,true);
                    return Ok(true);
                }
            }
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
            toggle: false,
        };
        let allow_async = if vk.0 == VK_TAB.0 as usize {
            self.can_defer_tab(vk, flags)
        } else {
            true
        };
        self.dispatch_edit(key, allow_async)
    }
    pub(super) fn dispatch_toggle(self: &Rc<Self>, context: &ITfContext) -> Result<()> {
        self.dispatch_edit(Key {
            context: context.clone(),
            generation: self.generation.get(),
            vk: WPARAM(0),
            flags: LPARAM(0),
            keyboard: [0; 256],
            pick: None,
            toggle: true,
        }, true).map(|_| ())
    }
    fn dispatch_edit(self: &Rc<Self>, key: Key, allow_async: bool) -> Result<bool> {
        self.wake_profile_update();
        if self.profile_update.borrow().is_none() && self.recovery.borrow().is_empty() {
            return match self.request_key(&key, allow_async) {
                Ok(Some(handled)) => Ok(handled),
                Ok(None) => Ok(true), // The lock callback now owns an IME key; it has not run yet.
                Err(_) => { self.keep_failed_key(key); Ok(true) }
            };
        }
        let mut recovery = self.recovery.borrow_mut();
        if key.vk.0 == VK_TAB.0 as usize && (!allow_async
            || self.profile_update.borrow().is_some()
            || recovery.deadline.is_some()
            || recovery.waiting_for_lock == Some(Waiting::Other)
            || !recovery.keys.iter().all(Key::is_forward_tab))
        {
            // An older edit can close the list or change modes. Forward the original
            // Tab while the host can still receive it; there must be no replay for it.
            return Ok(false);
        }
        recovery.keys.push_back(key);
        if recovery.deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            recovery.deadline = Some(Instant::now() + Duration::from_secs(5));
        }
        let waiting = recovery.waiting_for_lock.is_some();
        drop(recovery);
        if !waiting { self.schedule_retry(); }
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
            toggle: false,
        };
        self.wake_profile_update();
        if self.profile_update.borrow().is_none() && self.recovery.borrow().is_empty() {
            if self.attempt_key(&key, cookie).is_err() {
                self.keep_failed_key(key);
            }
        } else {
            self.recovery.borrow_mut().keys.push_back(key);
            self.schedule_retry();
        }
        Ok(())
    }
    pub(super) fn retry_pending(self: &Rc<Self>) {
        if self.recovery.borrow().waiting_for_lock.is_some() || self.retry_profile_update() { return; }
        // Bound each timer dispatch so a burst does not monopolize the UI thread.
        let started = Instant::now();
        for _ in 0..16 {
            let front = self.recovery.borrow().keys.front().cloned();
            let Some(key) = front else {
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
            self.recovery.borrow_mut().keys.pop_front();
            match self.request_key(&key, true) {
                Ok(None) => return,
                Ok(Some(_)) => {
                    // Following keys are a new composition in the same valid field.
                    self.recovery.borrow_mut().rebase(key.generation, self.generation.get());
                }
                Err(_) => {
                    self.keep_failed_key(key);
                    return;
                }
            }
            if started.elapsed() > Duration::from_millis(30) {
                self.schedule_retry();
                return;
            }
        }
        self.schedule_retry();
    }
}
