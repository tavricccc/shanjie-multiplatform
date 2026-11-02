//! Finish captured compositions independently of the current engine. A failed edit is
//! retryable; each request supersedes its older callbacks before another write can run.
use super::service::Data;
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};
use windows::{
    core::*,
    Win32::{Foundation::E_FAIL, UI::TextServices::*},
};

pub(super) struct Retirement {
    pub context: ITfContext,
    pub composition: ITfComposition,
    text: Vec<u16>,
    id: u64,
    ticket: Cell<u64>,
    written: Cell<bool>,
    pending: Cell<bool>,
    started: Cell<Instant>,
}
fn finish_steps(
    written: &Cell<bool>,
    live: impl Fn() -> bool,
    write: impl FnOnce() -> Result<()>,
    end: impl FnOnce() -> Result<()>,
) -> Result<()> {
    if !live() {
        return Ok(());
    }
    if !written.get() {
        write()?;
        if !live() {
            return Ok(());
        }
        written.set(true);
    }
    if live() {
        end()?;
    }
    Ok(())
}
impl Data {
    pub(super) fn is_retiring(&self, context: &ITfContext) -> bool {
        self.retiring.borrow().iter().any(|r| &r.context == context)
    }
    fn retirement_live(&self, r: &Rc<Retirement>, ticket: u64) -> bool {
        r.ticket.get() == ticket
            && self
                .retiring
                .borrow()
                .iter()
                .any(|entry| Rc::ptr_eq(entry, r))
    }
    fn retirement_error(&self, stage: &'static str, error: &Error) {
        crate::runtime_log::io(stage, &std::io::Error::from_raw_os_error(error.code().0));
    }
    pub(super) fn retire(
        self: &Rc<Self>,
        context: ITfContext,
        composition: ITfComposition,
        text: String,
    ) {
        let r = Rc::new(Retirement {
            context,
            composition,
            text: text.encode_utf16().collect(),
            id: self.generation.get(),
            ticket: Cell::new(0),
            written: Cell::new(false),
            pending: Cell::new(false),
            started: Cell::new(Instant::now()),
        });
        self.retiring.borrow_mut().push(r.clone());
        let _ = self.request_retirement(&r, false);
    }
    fn request_retirement(self: &Rc<Self>, r: &Rc<Retirement>, synchronous: bool) -> Result<()> {
        let ticket = r.ticket.get().wrapping_add(1);
        r.ticket.set(ticket);
        r.pending.set(true);
        r.started.set(Instant::now());
        let captured = r.clone();
        let data = self.clone();
        let flags = TF_ES_READWRITE
            | if synchronous {
                TF_ES_SYNC
            } else {
                TF_ES_ASYNCDONTCARE
            };
        let result = self.request(&r.context, flags, move |cookie| {
            if !data.retirement_live(&captured, ticket) {
                return Ok(());
            }
            let result = data.finish_retirement(&captured, ticket, cookie);
            if data.retirement_live(&captured, ticket) {
                captured.pending.set(false);
            }
            result
        });
        if let Err(error) = &result {
            if self.retirement_live(r, ticket) {
                r.pending.set(false);
            }
            self.retirement_error("retirement.request", error);
        }
        result
    }
    fn finish_retirement(&self, r: &Rc<Retirement>, ticket: u64, cookie: u32) -> Result<()> {
        finish_steps(
            &r.written,
            || self.retirement_live(r, ticket),
            || unsafe {
                let range = r
                    .composition
                    .GetRange()
                    .inspect_err(|e| self.retirement_error("retirement.range", e))?;
                if !self.retirement_live(r, ticket) {
                    return Ok(());
                }
                range
                    .SetText(cookie, 0, &r.text)
                    .inspect_err(|e| self.retirement_error("retirement.write", e))
            },
            || unsafe {
                self.diagnostic_retirement(
                    "retirement.end.begin",
                    r.id,
                    &r.context,
                    &r.composition,
                    r.text.len(),
                    None,
                );
                r.composition
                    .EndComposition(cookie)
                    .inspect_err(|e| self.retirement_error("retirement.end", e))
            },
        )?;
        if self.retirement_live(r, ticket) {
            self.retiring
                .borrow_mut()
                .retain(|entry| !Rc::ptr_eq(entry, r));
        }
        Ok(())
    }
    /// Retry failures immediately. A host may also accept a request and never grant it;
    /// after a short grace period a synchronous request invalidates that stale writer.
    pub(super) fn retry_retirement(
        self: &Rc<Self>,
        context: &ITfContext,
        preserve_visible: bool,
    ) -> Result<()> {
        let entries: Vec<_> = self
            .retiring
            .borrow()
            .iter()
            .filter(|r| &r.context == context)
            .cloned()
            .collect();
        for r in entries {
            if preserve_visible {
                r.written.set(true);
            }
            if preserve_visible
                || !r.pending.get()
                || r.started.get().elapsed() >= Duration::from_millis(750)
            {
                if let Err(error) = self.request_retirement(&r, true) {
                    // Some hosts only grant asynchronous locks. Keep one fresh request
                    // available instead of retrying a synchronous lock forever.
                    if error.code() == TF_E_SYNCHRONOUS || error.code() == TF_E_LOCKED {
                        self.request_retirement(&r, false)?;
                    } else {
                        return Err(error);
                    }
                }
            }
        }
        if self.is_retiring(context) {
            Err(E_FAIL.into())
        } else {
            Ok(())
        }
    }
    pub(super) fn retire_before_deactivate(self: &Rc<Self>) {
        let entries = self.retiring.borrow().clone();
        for r in entries {
            // Try to finish normally while the client ID is still valid.
            let _ = self.request_retirement(&r, true);
            // No old request may write after this service has been deactivated.
            r.ticket.set(r.ticket.get().wrapping_add(1));
            r.pending.set(false);
            r.written.set(true);
        }
    }
    pub(super) fn retirement_terminated(&self, ended: Option<&ITfComposition>) {
        self.retiring
            .borrow_mut()
            .retain(|r| Some(&r.composition) != ended);
    }
    pub(super) fn forget_retirement(&self, context: &ITfContext) {
        self.retiring.borrow_mut().retain(|r| &r.context != context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_write_is_retryable_and_failed_end_never_rewrites() {
        let written = Cell::new(false);
        let writes = Cell::new(0);
        assert!(finish_steps(
            &written,
            || true,
            || Err(E_FAIL.into()),
            || panic!("no end after a failed write")
        )
        .is_err());
        assert!(!written.get());
        assert!(finish_steps(
            &written,
            || true,
            || {
                writes.set(writes.get() + 1);
                Ok(())
            },
            || Err(E_FAIL.into())
        )
        .is_err());
        assert!(written.get());
        finish_steps(
            &written,
            || true,
            || panic!("must not write twice"),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(writes.get(), 1);
    }
    #[test]
    fn superseded_and_terminated_callbacks_cannot_write_or_end() {
        let written = Cell::new(false);
        let live = Cell::new(false);
        finish_steps(
            &written,
            || live.get(),
            || panic!("stale write"),
            || panic!("stale end"),
        )
        .unwrap();
        live.set(true);
        finish_steps(
            &written,
            || live.get(),
            || {
                live.set(false);
                Ok(())
            },
            || panic!("terminated during write"),
        )
        .unwrap();
        assert!(
            !written.get(),
            "a stale writer cannot advance its successor's stage"
        );
    }
}
