//! The core reranks an output after replying, as upstream runs the job off the main thread (s7c2b section 2.1); the
//! result is asked for here once the core says it is done, so a key never waits for the model.
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
        let context = self.state.borrow().context.clone();
        let Some(((ticket, _), context)) = self
            .rerank_wait
            .get()
            .filter(|(_, since)| since.elapsed() < GIVE_UP)
            .zip(context)
        else {
            return self.watch_rerank(false);
        };
        if !self.state.borrow_mut().engine.rerank_ready() {
            return;
        }
        self.rerank_wait.set(None);
        self.rerank_timer(false);
        let generation = self.generation.get();
        let data = self.clone();
        let target = context.clone();
        let _ = self.request(&target, TF_ES_READWRITE | TF_ES_ASYNCDONTCARE, move |cookie| {
            // A key or a cancel since the poll has its own output, and perhaps its own job.
            let current = data.state.borrow().context.clone();
            if data.rerank_ticket.get() != ticket || data.generation.get() != generation || current.as_ref() != Some(&context) {
                return Ok(());
            }
            let snapshot = output(data.state.borrow_mut().engine.perform(Op::Rerank))?;
            let unchanged = {
                let s = data.state.borrow();
                let shown = &s.snapshot;
                shown.preedit == snapshot.preedit
                    && shown.cursor == snapshot.cursor
                    && shown.candidates == snapshot.candidates
                    && shown.selected == snapshot.selected
                    && shown.first == snapshot.first
                    && shown.total == snapshot.total
            };
            if unchanged {
                return Ok(());
            }
            data.apply(&context, cookie, snapshot, false)
        });
    }
}
