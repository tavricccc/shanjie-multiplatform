use super::service::{Data,selection,range,compartment};
use std::{rc::Rc, time::Instant};
use windows::{core::*, Win32::{Foundation::*, System::Com::CoTaskMemFree,
    UI::{TextServices::*, WindowsAndMessaging::*, Input::KeyboardAndMouse::GetFocus}}};
// The double-tap interval is release -> next press. Holding either Ctrl has
// its own bound; duplicate TSF callbacks and keyboard repeat are not new taps.
#[derive(Default)]
pub(super) struct Gesture {
    down: Option<(usize,u64,Instant,bool)>,
    previous: Option<(usize,Instant,u64)>,
}
impl Gesture {
    pub fn reset(&mut self) { *self = Self::default(); }
    pub fn observe(&mut self, key:usize, down:bool, repeat:bool, epoch:u64, pure:bool, interval:u32) -> bool {
        self.at(key,down,repeat,epoch,pure,interval,Instant::now())
    }
    fn at(&mut self, key:usize, down:bool, repeat:bool, epoch:u64, pure:bool, interval:u32, now:Instant) -> bool {
        if ![0xa2,0xa3].contains(&key) || !pure { self.reset(); return false; }
        if down {
            if let Some((side,start,_,_))=self.down {
                if side!=key || start!=epoch { self.reset(); }
                return false;
            }
            if repeat { self.reset(); return false; }
            let second=self.previous.take().is_some_and(|(side,released,activity)|
                side==key && activity==epoch && now.duration_since(released).as_millis()<=interval as u128);
            self.down=Some((key,epoch,now,second));
            return false;
        }
        let Some((side,start,pressed,second))=self.down.take() else { return false; };
        if side!=key || start!=epoch || now.duration_since(pressed).as_millis()>600 { self.reset(); return false; }
        if second { self.previous=None; return true; }
        self.previous=Some((key,now,epoch)); false
    }
}
#[derive(Clone)]
pub(super) struct Request {
    context:ITfContext, generation:u64, revision:u64, deadline:Instant, focus:HWND, foreground:HWND, owner:HWND,
}
impl Request {
    fn focused(&self)->bool { unsafe { GetFocus()==self.focus && GetForegroundWindow()==self.foreground && IsWindow(Some(self.owner)).as_bool() } }
}
pub(super) struct Selection {
    context: ITfContext, generation: u64, owner: HWND, rect: RECT, word: String,
}
fn editable(context:&ITfContext)->Result<()> { unsafe {
    if context.GetStatus()?.dwStaticFlags & TF_SD_READONLY!=0
        || compartment(context,&GUID_COMPARTMENT_KEYBOARD_DISABLED)
        || compartment(context,&GUID_COMPARTMENT_EMPTYCONTEXT) { return Err(E_ACCESSDENIED.into()); }
    Ok(())
} }
unsafe fn native_password(owner:HWND)->bool {
    let mut class=[0u16;128]; let len=GetClassNameW(owner,&mut class);
    let class=String::from_utf16_lossy(&class[..len as usize]).to_ascii_lowercase();
    (class=="edit" || class.starts_with("richedit")) && GetWindowLongW(owner,GWL_STYLE) as u32 & ES_PASSWORD as u32!=0
}
impl Data {
    pub(super) fn show_word(self:&Rc<Self>, context:&ITfContext) -> Result<()> {
        self.diagnostic("word.enter",Some(context),None,None);
        if self.state.borrow().composition.is_some() { return Ok(()); }
        if editable(context).is_err() { return Ok(()); }
        self.close_word();
        let focus=unsafe {GetFocus()};
        let owner=if focus.is_invalid() {unsafe {context.GetActiveView()?.GetWnd()?}}else{focus};
        if owner.is_invalid() || unsafe {native_password(owner)} { return Ok(()); }
        *self.word_request.borrow_mut()=Some(Request {context:context.clone(),generation:self.generation.get(),revision:self.word_revision.get(),deadline:Instant::now()+std::time::Duration::from_millis(1500),
            focus,foreground:unsafe{GetForegroundWindow()},owner});
        if let Some(w)=self.state.borrow().watcher.as_ref() { w.word_read(); }
        Ok(())
    }
    // Run after the host has finished the key-up callback and synchronized selection.
    pub(super) fn read_word(self:&Rc<Self>)->Result<()> {
        let Some(mut request)=self.word_request.borrow_mut().take() else { return Ok(()); };
        if !request.focused() || request.generation!=self.generation.get() { return Ok(()); }
        let focused=self.state.borrow().thread.as_ref().and_then(|t|unsafe{t.GetFocus().ok()}).and_then(|d|unsafe{d.GetTop().ok()});
        if let Some(context)=focused { request.context=context; }
        if editable(&request.context).is_err() || self.state.borrow().composition.is_some() { return Ok(()); }
        self.watch(&request.context)?;
        request.generation=self.generation.get();
        request.revision=self.word_revision.get();
        *self.word_request.borrow_mut()=Some(request.clone());
        if let Some(w)=self.state.borrow().watcher.as_ref() {w.word_poll(true);}
        let data=self.clone(); let context=request.context.clone();
        let result=self.request(&context,TF_ES_READ|TF_ES_ASYNCDONTCARE,move|cookie| {
            if !data.word_current(&request) { return Ok(()); }
            match unsafe {data.read_selected(&request,cookie)} {
                Ok(Some((word,rect)))=>data.word_ready(&request,word,rect),
                Ok(None)=>{ if let Some(w)=data.state.borrow().watcher.as_ref() {w.word_fallback();} },
                Err(_)=>data.word_failed(),
            }
            Ok(())
        });
        if result.is_err() { self.word_failed(); }
        result
    }
    fn word_current(&self,request:&Request)->bool {
        request.revision==self.word_revision.get() && request.generation==self.generation.get() && self.state.borrow().context.as_ref()==Some(&request.context)
            && request.focused() && editable(&request.context).is_ok() && self.state.borrow().composition.is_none()
    }
    unsafe fn read_selected(&self,request:&Request,cookie:u32)->Result<Option<(String,RECT)>> {
        let current=&request.context;
        let selected=selection(current,cookie).map(range);
        let scope_range=match &selected {Ok(r)=>r.clone(),Err(_)=>current.GetStart(cookie)?};
        if let Ok(value)=current.GetAppProperty(&GUID_PROP_INPUTSCOPE)?.GetValue(cookie,&scope_range) {
            if let Ok(scope)=IUnknown::try_from(&value).and_then(|u|u.cast::<ITfInputScope>()) {
                let mut scopes=std::ptr::null_mut(); let mut count=0;
                scope.GetInputScopes(&mut scopes,&mut count)?;
                let protected=!scopes.is_null() && std::slice::from_raw_parts(scopes,count as usize).contains(&IS_PASSWORD);
                CoTaskMemFree(Some(scopes.cast())); if protected {return Err(E_ACCESSDENIED.into());}
            }
        }
        if native_password(request.owner) {return Err(E_ACCESSDENIED.into());}
        let Ok(selected)=selected else {return Ok(None);};
        self.diagnostic_range("word.selection.range",&selected,cookie,0);
        let mut text=[0u16;65]; let mut count=0;
        if selected.GetText(cookie,0,&mut text,&mut count).is_err() {return Ok(None);}
        self.diagnostic_range("word.text.length",&selected,cookie,count as usize);
        if count==0 { self.diagnostic("word.fallback.empty_selection",Some(current),None,None); return Ok(None); }
        if count as usize>=text.len() { return Err(E_INVALIDARG.into()); }
        let word=String::from_utf16(&text[..count as usize]).map_err(|_|Error::from_hresult(E_INVALIDARG))?;
        crate::word_store::validate_word(&word).map_err(|_|Error::from_hresult(E_INVALIDARG))?;
        let Ok(view)=current.GetActiveView() else {return Ok(None);};
        let mut rect=RECT::default(); let mut clipped=BOOL::default();
        if view.GetTextExt(cookie,&selected,&mut rect,&mut clipped).is_err() || rect==RECT::default() || clipped.as_bool() {return Ok(None);}
        super::service::physical(view.GetWnd()?,&mut rect)?;
        Ok(Some((word,rect)))
    }
    fn word_ready(&self,request:&Request,word:String,rect:RECT) {
        if !self.word_current(request) {return;}
        *self.word_selection.borrow_mut()=Some(Selection{context:request.context.clone(),generation:request.generation,owner:request.owner,rect,word});
        if let Some(w)=self.state.borrow().watcher.as_ref() {w.add_word();}
    }
    pub(super) fn fallback_word(&self) {
        let request=self.word_request.borrow().clone();
        let Some(request)=request.filter(|r|self.word_current(r)) else {self.close_word();return;};
        let lookup=crate::selection_text::start(request.owner);
        if let Some(lookup)=lookup {
            *self.word_lookup.borrow_mut()=Some(lookup);
            if let Some(w)=self.state.borrow().watcher.as_ref() {w.word_poll(true);}
        } else {self.word_failed();}
    }
    pub(super) fn poll_word(&self) {
        let request=self.word_request.borrow().clone();
        let Some(request)=request.filter(|r|self.word_current(r)) else {self.close_word();return;};
        let mut event=None; let mut failed=false; let mut finished=false;
        if self.word_lookup.borrow().is_none() && Instant::now()>request.deadline {self.word_failed();return;}
        if let Some(lookup)=self.word_lookup.borrow_mut().as_mut() {
            match lookup.receiver.try_recv() {
                Ok(crate::selection_text::Event::Ready(word,rect))=>{lookup.delivered=true;event=Some((word,rect));},
                Ok(crate::selection_text::Event::Invalid)=>{failed=!lookup.delivered;finished=true;},
                Err(std::sync::mpsc::TryRecvError::Disconnected)=>{failed=!lookup.delivered;finished=true;},
                Err(_)=>if !lookup.delivered && std::time::Instant::now()>lookup.deadline {failed=true;finished=true;},
            }
            if lookup.delivered && event.is_none() && self.word_selection.borrow().is_none()
                && !self.word_popup.borrow().as_ref().is_some_and(|p|p.active()) {finished=true;}
        }
        if finished {if failed {self.word_failed();}else{self.close_word();}return;}
        if let Some((word,rect))=event {self.word_ready(&request,word,rect);}
    }
    fn word_failed(&self) {
        let request=self.word_request.borrow().clone(); self.close_word();
        if let Some(request)=request.filter(|r|r.focused()) {
            let theme=self.state.borrow().prefs.number("appearance","theme",0);
            if let Ok(notice)=unsafe {crate::selection_notice::Notice::open(request.owner,theme)} {*self.word_notice.borrow_mut()=Some(notice);}
        }
    }
    pub(super) fn open_word_popup(&self) {
        self.diagnostic("word.popup.enter",None,None,None);
        let Some(selected)=self.word_selection.borrow_mut().take() else { self.diagnostic("word.blocked.no_selection",None,None,None); return; };
        if self.word_request.borrow().as_ref().is_some_and(|r|!self.word_current(r)) { self.close_word(); return; }
        if self.word_lookup.borrow().is_none() {if let Some(w)=self.state.borrow().watcher.as_ref(){w.word_poll(false);}}
        let (layout,data,theme,notify)={
            let s=self.state.borrow();
            if self.generation.get()!=selected.generation || s.context.as_ref()!=Some(&selected.context){return;}
            let keyboard=s.prefs.number("input","keyboard",0);
            let layout=crate::installation::layout(keyboard,s.profile_layout,crate::installation::available());
            let layout=if layout==1{shanjie_core::engine::Layout::Eten}else{shanjie_core::engine::Layout::Standard};
            (layout,s.data.clone(),s.prefs.number("appearance","theme",0),s.watcher.as_ref().map_or(HWND::default(),|w|w.word_target()))
        };
        let revision=self.word_revision.get();
        match unsafe{crate::word_popup::Popup::open(selected.owner,selected.rect,selected.word,data,layout,theme,notify)} {
            Ok(popup)=>{
                if self.generation.get()!=selected.generation || self.word_revision.get()!=revision {return;}
                // Drop the old command handle outside the Data borrow.
                self.word_popup_revision.set(revision);
                let retired=self.word_popup.borrow_mut().replace(popup);drop(retired);
                self.diagnostic("word.popup.result",Some(&selected.context),None,Some(0));
            },
            Err(e)=>{self.diagnostic("word.popup.result",Some(&selected.context),None,Some(e.code().0));super::trace(format_args!("word popup: {}",e.code()))}
        }
    }
    pub(super) fn word_closed(&self) {
        let id=self.state.borrow().watcher.as_ref().map_or(0,|w|w.take_word_closed());
        let current=self.word_popup.borrow().as_ref().is_some_and(|p|p.id()==id);
        if current && self.word_popup_revision.get()==self.word_revision.get() {self.close_word();}
    }
    pub(super) fn close_word(&self) {
        self.diagnostic("word.close",None,None,None);
        self.word_revision.set(self.word_revision.get().wrapping_add(1));
        self.word_request.borrow_mut().take();
        self.word_lookup.borrow_mut().take();
        self.word_notice.borrow_mut().take();
        if let Some(w)=self.state.borrow().watcher.as_ref() {w.word_poll(false);}
        self.word_selection.borrow_mut().take();
        if let Some(popup)=self.word_popup.borrow().as_ref() { popup.close(); }
        self.ctrl_gesture.borrow_mut().reset();
    }
}
#[cfg(test)] mod tests {
    use super::*;
    use std::time::Duration;
    #[test] fn ctrl_natural_timing_and_duplicate_notifications() {
        let mut g=Gesture::default(); let t=Instant::now();
        let mut at=|down,repeat,ms| g.at(0xa2,down,repeat,7,true,450,t+Duration::from_millis(ms));
        assert!(!at(true,false,0)); assert!(!at(true,false,0));
        assert!(!at(false,false,60)); assert!(!at(false,false,60));
        assert!(!at(true,false,421)); assert!(!at(true,true,450)); assert!(!at(true,true,510));
        assert!(at(false,false,546)); // release-release 486 ms, press gap 361 ms
        assert!(!at(false,false,546));
    }
    #[test] fn ctrl_chords_sides_timeout_and_unseen_events_cancel() {
        let t=Instant::now();
        for side in [0xa2,0xa3] {
            for (other,activity,pure,held,gap) in [(side,8,true,80,200),(side,7,false,80,200),
                (if side==0xa2 {0xa3}else{0xa2},7,true,80,200),(side,7,true,601,200),(side,7,true,80,451)] {
                let mut g=Gesture::default();
                g.at(side,true,false,7,true,450,t);
                g.at(side,false,false,7,true,450,t+Duration::from_millis(50));
                g.at(other,true,false,activity,pure,450,t+Duration::from_millis(50+gap));
                assert!(!g.at(other,false,false,activity,pure,450,t+Duration::from_millis(50+gap+held)));
            }
        }
    }
}
