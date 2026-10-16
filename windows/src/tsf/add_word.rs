use super::service::{Data,selection,range,compartment};
use std::{rc::Rc, time::Instant};
use windows::{core::*, Win32::{Foundation::*, System::Com::CoTaskMemFree,
    UI::{TextServices::*, WindowsAndMessaging::*}}};
#[derive(Default)]
pub(super) struct Gesture {
    down: Option<(usize,u64,Instant)>,
    previous: Option<(usize,Instant,u64)>,
}
impl Gesture {
    pub fn reset(&mut self) { *self = Self::default(); }
    pub fn observe(&mut self, key:usize, down:bool, repeat:bool, epoch:u64, pure:bool, interval:u32) -> bool {
        if ![0xa2,0xa3].contains(&key) { self.reset(); return false; }
        if down {
            if repeat { self.reset(); return false; }
            if self.down.is_some() { return false; }
            if pure { self.down=Some((key,epoch,Instant::now())); } else { self.reset(); }
            return false;
        }
        let Some((side,start,pressed)) = self.down.take() else { return false; };
        if key != side || !pure || epoch.wrapping_sub(start)>1 || pressed.elapsed().as_millis()>interval as u128 { self.reset(); return false; }
        let now = Instant::now();
        if self.previous.take().is_some_and(|(last,time,last_epoch)| last==side && now.duration_since(time).as_millis() <= interval as u128 && start.wrapping_sub(last_epoch)<=1) {
            return true;
        }
        self.previous=Some((side,now,epoch)); false
    }
}
pub(super) struct Selection {
    context: ITfContext,
    generation: u64,
    owner: HWND,
    rect: RECT,
    word: String,
}
impl Data {
    pub(super) fn show_word(self:&Rc<Self>, context:&ITfContext) -> Result<()> {
        let status = unsafe { context.GetStatus()? };
        if status.dwStaticFlags & TF_SD_READONLY != 0 || unsafe { compartment(context,&GUID_COMPARTMENT_KEYBOARD_DISABLED) || compartment(context,&GUID_COMPARTMENT_EMPTYCONTEXT) }
            || self.state.borrow().composition.is_some() { return Ok(()); }
        self.watch(context)?;
        let generation = self.generation.get(); let data=self.clone(); let current=context.clone();
        self.request(context,TF_ES_READ|TF_ES_ASYNCDONTCARE,move|cookie| unsafe {
            if data.generation.get()!=generation || data.state.borrow().context.as_ref()!=Some(&current) { return Ok(()); }
            let selected=range(selection(&current,cookie)?);
            // Password scopes and native password edits are never inspected/saved.
            if let Ok(value)=current.GetAppProperty(&GUID_PROP_INPUTSCOPE)?.GetValue(cookie,&selected) {
                if let Ok(scope)=IUnknown::try_from(&value).and_then(|u|u.cast::<ITfInputScope>()) {
                    let mut scopes=std::ptr::null_mut(); let mut count=0;
                    scope.GetInputScopes(&mut scopes,&mut count)?;
                    let protected = !scopes.is_null() && std::slice::from_raw_parts(scopes,count as usize).contains(&IS_PASSWORD);
                    CoTaskMemFree(Some(scopes.cast())); if protected { return Ok(()); }
                }
            }
            let view=current.GetActiveView()?; let owner=view.GetWnd()?;
            if owner.is_invalid() || GetWindowLongW(owner,GWL_STYLE) as u32 & ES_PASSWORD as u32 != 0 { return Ok(()); }
            let mut text=[0u16;65]; let mut count=0;
            selected.GetText(cookie,0,&mut text,&mut count)?;
            if count==0 || count as usize>=text.len() { return Ok(()); }
            let word=String::from_utf16(&text[..count as usize]).map_err(|_| Error::from_hresult(E_FAIL))?;
            if crate::word_store::validate_word(&word).is_err() { return Ok(()); }
            let mut rect=RECT::default(); let mut clipped=BOOL::default();
            if view.GetTextExt(cookie,&selected,&mut rect,&mut clipped).is_err() || rect==RECT::default() || clipped.as_bool() { return Ok(()); }
            super::service::physical(owner,&mut rect)?;
            *data.word_selection.borrow_mut()=Some(Selection { context:current,generation,owner,rect,word });
            if let Some(watcher)=data.state.borrow().watcher.as_ref() { watcher.add_word(); }
            Ok(())
        })
    }
    pub(super) fn open_word_popup(&self) {
        let Some(selected)=self.word_selection.borrow_mut().take() else { return; };
        let s=self.state.borrow();
        if self.generation.get()!=selected.generation || s.context.as_ref()!=Some(&selected.context) { return; }
        let keyboard=s.prefs.number("input","keyboard",0);
        let layout=crate::installation::layout(keyboard,s.profile_layout,crate::installation::available());
        let layout=if layout==1 { shanjie_core::engine::Layout::Eten } else { shanjie_core::engine::Layout::Standard };
        match unsafe { crate::word_popup::Popup::open(selected.owner,selected.rect,selected.word,s.data.clone(),layout) } {
            Ok(popup)=>*self.word_popup.borrow_mut()=Some(popup), Err(e)=>super::trace(format_args!("word popup: {}",e.code()))
        }
    }
    pub(super) fn close_word(&self) {
        self.word_selection.borrow_mut().take();
        if let Some(popup)=self.word_popup.borrow_mut().as_mut() { popup.close(); }
        self.ctrl_gesture.borrow_mut().reset();
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn ctrl_taps_ignore_duplicate_callbacks_and_repeats() {
        let mut g=Gesture::default();
        assert!(!g.observe(0xa2,true,false,1,true,450));
        assert!(!g.observe(0xa2,true,false,1,true,450));
        assert!(!g.observe(0xa2,false,false,2,true,450));
        assert!(!g.observe(0xa2,false,false,2,true,450));
        assert!(!g.observe(0xa2,true,false,3,true,450));
        assert!(g.observe(0xa2,false,false,4,true,450));
    }
    #[test] fn ctrl_chords_sides_and_unseen_hook_events_cancel() {
        for side in [0xa2,0xa3] {
            let mut g=Gesture::default();
            g.observe(side,true,false,1,true,450); g.observe(side,false,false,2,true,450);
            g.observe(side,true,false,3,true,450);
            assert!(!g.observe(side,false,false,6,true,450));
            g.observe(side,true,false,7,false,450);
            assert!(!g.observe(side,false,false,8,true,450));
            g.observe(side,true,false,9,true,450); g.observe(side,true,true,9,true,450);
            assert!(!g.observe(side,false,false,10,true,450));
        }
    }
}
