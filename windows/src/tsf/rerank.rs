//! The core reranks an output after replying, as upstream runs the job off the main thread (s7c2b section 2.1); the
//! result is asked for here once the core says it is done. Enter can also apply the result before commit.
use super::service::{output, Data};
use crate::broker::Op;
use std::{
    rc::Rc,
    time::{Duration, Instant},
};
use windows::Win32::UI::TextServices::*;

/// A job that has not finished by then is left alone; the next key makes its own.
const GIVE_UP: Duration = Duration::from_secs(2);

impl Data {
    /// Every applied output replaces the one being waited for.
    pub(super) fn watch_rerank(&self, pending: bool) {
        let ticket = self.rerank_ticket.get().wrapping_add(1);
        self.rerank_ticket.set(ticket);
        self.rerank_wait.set(pending.then(|| (ticket, Instant::now())));
        self.rerank_timer(pending);
    }
    fn rerank_timer(&self, on: bool) {
        if let Some(watcher) = self.state.borrow().watcher.as_ref() {
            watcher.rerank_poll(on);
        }
    }
    pub(super) fn poll_rerank(self: &Rc<Self>) {
        let (context, window_open) = {
            let s = self.state.borrow();
            (s.context.clone(), !s.snapshot.candidates.is_empty() && !s.snapshot.prediction)
        };
        #[cfg(feature = "rerank-timing")]
        let current_job_id = self.state.borrow().engine.current_rerank_job_id().unwrap_or(0);
        let Some(((ticket, _), context)) = self
            .rerank_wait
            .get()
            .filter(|(_, since)| since.elapsed() < GIVE_UP)
            .zip(context)
        else {
            #[cfg(feature = "rerank-timing")]
            if self.rerank_wait.get().is_some_and(|(_, since)| since.elapsed() >= GIVE_UP) {
                crate::rerank_timing::record(
                    crate::rerank_timing::ROLE_TIP,
                    crate::rerank_timing::EV_POLL_END,
                    0,
                    current_job_id,
                    0,
                    0,
                    0,
                    crate::rerank_timing::POLL_GIVE_UP_2S,
                );
                crate::rerank_timing::record(
                    crate::rerank_timing::ROLE_TIP,
                    crate::rerank_timing::EV_RESULT_DROPPED,
                    0,
                    current_job_id,
                    0,
                    0,
                    0,
                    crate::rerank_timing::DROP_GIVE_UP_TIMEOUT,
                );
            }
            return self.watch_rerank(false);
        };
        if window_open {
            #[cfg(feature = "rerank-timing")]
            crate::rerank_timing::record(
                crate::rerank_timing::ROLE_TIP,
                crate::rerank_timing::EV_POLL_END,
                0,
                current_job_id,
                0,
                ticket,
                0,
                crate::rerank_timing::POLL_WINDOW_OPEN,
            );
            return;
        }
        if !self.state.borrow_mut().engine.rerank_ready() {
            return;
        }
        let wait = self.rerank_wait.replace(None);
        self.rerank_timer(false);
        let generation = self.generation.get();
        let data = self.clone();
        let target = context.clone();
        if self
            .request(&target, TF_ES_READWRITE | TF_ES_ASYNCDONTCARE, move |cookie| {
            // A key or a cancel since the poll has its own output, and perhaps its own job.
            let current = data.state.borrow().context.clone();
            if data.rerank_ticket.get() != ticket || data.generation.get() != generation || current.as_ref() != Some(&context) {
                #[cfg(feature = "rerank-timing")]
                crate::rerank_timing::record(
                    crate::rerank_timing::ROLE_TIP,
                    crate::rerank_timing::EV_RESULT_DROPPED,
                    0,
                    current_job_id,
                    0,
                    ticket,
                    0,
                    crate::rerank_timing::DROP_SESSION_MISMATCH,
                );
                return Ok(());
            }
            data.update_privacy(&context, cookie)?;
            #[cfg(feature = "rerank-timing")]
            crate::rerank_timing::record(
                crate::rerank_timing::ROLE_TIP,
                crate::rerank_timing::EV_RERANK_FETCH_START,
                0,
                current_job_id,
                0,
                ticket,
                0,
                0,
            );
            let snapshot = output(data.state.borrow_mut().engine.perform(Op::Rerank))?;
            let unchanged = {
                let s = data.state.borrow();
                let shown = &s.snapshot;
                shown.preedit == snapshot.preedit
                    && shown.cursor == snapshot.cursor
                    && shown.active_segment == snapshot.active_segment
                    && shown.candidates == snapshot.candidates
                    && shown.selected == snapshot.selected
                    && shown.first == snapshot.first
                    && shown.total == snapshot.total
            };
            #[cfg(feature = "rerank-timing")]
            crate::rerank_timing::record(
                crate::rerank_timing::ROLE_TIP,
                crate::rerank_timing::EV_RERANK_FETCH_END,
                0,
                current_job_id,
                0,
                ticket,
                (!unchanged) as u8,
                0,
            );
            if unchanged {
                #[cfg(feature = "rerank-timing")]
                {
                    crate::rerank_timing::record(
                        crate::rerank_timing::ROLE_TIP,
                        crate::rerank_timing::EV_RESULT_APPLIED,
                        0,
                        current_job_id,
                        0,
                        ticket,
                        0,
                        crate::rerank_timing::APPLY_TSF_POLL_UNCHANGED,
                    );
                    crate::rerank_timing::mark_tip_applied(current_job_id);
                }
                return Ok(());
            }
            let res = data.apply(&context, cookie, snapshot, false);
            #[cfg(feature = "rerank-timing")]
            if res.is_ok() {
                crate::rerank_timing::record(
                    crate::rerank_timing::ROLE_TIP,
                    crate::rerank_timing::EV_RESULT_APPLIED,
                    0,
                    current_job_id,
                    0,
                    ticket,
                    1,
                    crate::rerank_timing::APPLY_TSF_POLL_CHANGED,
                );
                crate::rerank_timing::mark_tip_applied(current_job_id);
            }
            res
        })
            .is_err()
            && self.rerank_ticket.get() == ticket
        {
            #[cfg(feature = "rerank-timing")]
            crate::rerank_timing::record(
                crate::rerank_timing::ROLE_TIP,
                crate::rerank_timing::EV_RESULT_DROPPED,
                0,
                current_job_id,
                0,
                ticket,
                0,
                crate::rerank_timing::DROP_EDIT_SESSION_REFUSED,
            );
            self.rerank_wait.set(wait);
            self.rerank_timer(true);
        }
    }
}
