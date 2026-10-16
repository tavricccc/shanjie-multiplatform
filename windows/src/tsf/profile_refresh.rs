//! A live setting change owns its unacknowledged operation until it completes.
use super::service::{output, Data};
use crate::{broker::Op, engine::Snapshot};
use std::{
    collections::VecDeque,
    rc::Rc,
    time::{Duration, Instant},
};
use windows::{core::*, Win32::UI::TextServices::*};

pub(super) struct Pending {
    context: Option<ITfContext>,
    generation: u64,
    operations: VecDeque<Op>,
    snapshot: Option<Snapshot>,
    deadline: Instant,
    scheduled: bool,
}
impl Pending {
    pub fn new(context: Option<ITfContext>, generation: u64, operations: VecDeque<Op>) -> Self {
        Self {
            context,
            generation,
            operations,
            snapshot: None,
            deadline: Instant::now() + Duration::from_secs(5),
            scheduled: false,
        }
    }
    pub fn wake(&mut self) {
        self.deadline = Instant::now() + Duration::from_secs(5);
    }
}
impl Data {
    pub(super) fn wake_profile_update(&self) {
        if let Some(pending) = self.profile_update.borrow_mut().as_mut() {
            pending.wake();
        }
    }
    pub(super) fn retry_profile_update(self: &Rc<Self>) -> bool {
        let expired = self
            .profile_update
            .borrow()
            .as_ref()
            .filter(|pending| pending.deadline <= Instant::now())
            .map(|pending| pending.context.clone());
        if let Some(context) = expired {
            crate::bridge::failure(
                "設定及按鍵仍保留；核心暫時無法回應，按任一鍵重試或 Esc 取消組字。",
            );
            if let Some(context) = context {
                self.recovery_hint(&context, true);
            }
            return true;
        }
        let request = {
            let mut slot = self.profile_update.borrow_mut();
            let Some(pending) = slot.as_mut() else {
                return false;
            };
            if pending.scheduled {
                None
            } else {
                pending.scheduled = true;
                Some((pending.context.clone(), pending.generation))
            }
        };
        if let Some((context, generation)) = request {
            if let Some(context) = context {
                let data = self.clone();
                let result = self.request(
                    &context,
                    TF_ES_READWRITE | TF_ES_ASYNCDONTCARE,
                    move |cookie| {
                        data.perform_profile_update(generation, cookie);
                        Ok(())
                    },
                );
                if result.is_err() {
                    if let Some(pending) = self.profile_update.borrow_mut().as_mut() {
                        pending.scheduled = false;
                    }
                }
            } else {
                self.perform_profile_update(generation, 0);
            }
        }
        let pending = self.profile_update.borrow().is_some();
        if pending {
            self.schedule_retry();
        }
        pending
    }
    fn perform_profile_update(self: &Rc<Self>, generation: u64, cookie: u32) {
        let mut pending = {
            let mut slot = self.profile_update.borrow_mut();
            if slot
                .as_ref()
                .is_none_or(|pending| pending.generation != generation)
            {
                return;
            }
            slot.take().unwrap()
        };
        if self.generation.get() != generation
            || pending.context.as_ref() != self.state.borrow().context.as_ref()
        {
            return;
        }
        pending.scheduled = false;
        let result = (|| -> Result<()> {
            while let Some(op) = pending.operations.front().cloned() {
                let view = matches!(op, Op::Prediction(_) | Op::Demote(_));
                let snapshot = output(self.state.borrow_mut().engine.perform(op))?;
                // Pop only after acknowledgement. A timed out request resumes the
                // same broker sequence; earlier settings are not sent again.
                pending.operations.pop_front();
                if view {
                    pending.snapshot = Some(snapshot);
                }
            }
            if let Some(context) = pending.context.as_ref() {
                if let Some(snapshot) = pending.snapshot.as_ref() {
                    self.apply(context, cookie, snapshot.clone(), false)?;
                } else {
                    self.layout(context, TF_LC_CHANGE)?;
                }
            }
            Ok(())
        })();
        if result.is_err() {
            let context = pending.context.clone();
            *self.profile_update.borrow_mut() = Some(pending);
            if let Some(context) = context {
                self.recovery_hint(&context, false);
            }
            self.schedule_retry();
        } else {
            self.recovery
                .borrow_mut()
                .rebase(generation, self.generation.get());
            if !self.recovery.borrow().is_empty() || self.recovery.borrow().refresh {
                self.schedule_retry();
            }
        }
    }
}
