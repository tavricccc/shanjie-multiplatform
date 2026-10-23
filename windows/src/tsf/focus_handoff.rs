//! A document can temporarily lose focus while a host starts editing (Excel).
//! Keep only a live composition, for a bounded interval and identical identities.
use super::service::Data;
use std::{rc::Rc,time::{Duration,Instant}};
use windows::{core::Result,Win32::UI::TextServices::*};
pub(super) struct Pending {
    document: ITfDocumentMgr,
    context: ITfContext,
    generation: u64,
    started: Instant,
}
impl Data {
    pub(super) fn clear_handoff(&self) {
        self.focus_handoff.borrow_mut().take();
        if let Some(w)=self.state.borrow().watcher.as_ref() { w.focus_handoff(false); }
    }
    pub(super) fn document_focus(self:&Rc<Self>, current:Option<&ITfDocumentMgr>, previous:Option<&ITfDocumentMgr>) -> Result<()> {
        if self.focus_handoff.borrow().is_some() {
            if current.is_some() { return self.resolve_handoff(current); }
            // Repeated NULL notifications do not extend the deadline.
            return Ok(());
        }
        if current==previous { return Ok(()); }
        if current.is_none() {
            let captured={ let s=self.state.borrow();
                s.context.as_ref().filter(|_|s.composition.is_some()).and_then(|c|
                    unsafe { c.GetDocumentMgr().ok() }.filter(|doc|Some(doc)==previous).map(|doc|(doc,c.clone()))) };
            if let Some((document,context))=captured {
                self.close_word();
                { let mut s=self.state.borrow_mut(); s.shift_tap=false; s.shift_used=true; }
                *self.focus_handoff.borrow_mut()=Some(Pending {document,context,generation:self.generation.get(),started:Instant::now()});
                if let Some(w)=self.state.borrow().watcher.as_ref() { w.focus_handoff(true); }
                self.diagnostic("focus.handoff.begin",None,None,None);
                return Ok(());
            }
        }
        self.cancel()
    }
    fn resolve_handoff(self:&Rc<Self>, current:Option<&ITfDocumentMgr>) -> Result<()> {
        let valid=self.focus_handoff.borrow().as_ref().is_some_and(|p|
            p.started.elapsed()<=Duration::from_millis(150) && p.generation==self.generation.get()
            && current==Some(&p.document) && self.state.borrow().context.as_ref()==Some(&p.context)
            && unsafe { p.document.GetTop().ok() }.as_ref()==Some(&p.context));
        self.clear_handoff();
        if valid { self.diagnostic("focus.handoff.resume",None,None,None); Ok(()) }
        else { self.cancel() }
    }
    pub(super) fn expire_handoff(self:&Rc<Self>) {
        if self.focus_handoff.borrow().is_none() { return; }
        let current=self.state.borrow().thread.as_ref().and_then(|t|unsafe{t.GetFocus().ok()});
        let _=self.resolve_handoff(current.as_ref());
    }
    pub(super) fn key_handoff(self:&Rc<Self>, context:Option<&ITfContext>) -> Result<()> {
        if self.focus_handoff.borrow().is_none() { return Ok(()); }
        if context!=self.state.borrow().context.as_ref() { return self.cancel(); }
        let current=self.state.borrow().thread.as_ref().and_then(|t|unsafe{t.GetFocus().ok()});
        // No text is accepted into an unidentified document during the handoff.
        self.resolve_handoff(current.as_ref())
    }
}
