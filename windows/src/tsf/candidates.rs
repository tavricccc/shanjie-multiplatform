use super::{service::Data, Count};
use crate::{candidate::Host, engine::Snapshot};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        UI::{HiDpi::*, TextServices::*, WindowsAndMessaging::*},
    },
};
const TF_INVALID_UIELEMENTID: u32 = u32::MAX;

pub struct Candidates {
    interface: Cell<*mut std::ffi::c_void>,
    manager: ITfUIElementMgr,
    document: RefCell<Option<ITfDocumentMgr>>,
    pub snapshot: RefCell<Snapshot>,
    host: RefCell<Option<Box<Host>>>,
    window: Cell<HWND>,
    id: Cell<u32>,
    native: Cell<bool>,
    pub revision: Cell<u64>,
    caret: Cell<RECT>,
    pub theme: Cell<u32>,
    pub size: Cell<u32>,
    owner: Weak<Data>,
    _count: Count,
}
#[implement(ITfCandidateListUIElementBehavior)]
struct Element {
    data: Rc<Candidates>,
}
pub struct Handle {
    pub data: Rc<Candidates>,
    pub _interface: ITfCandidateListUIElementBehavior,
}
pub unsafe fn new(
    thread: &ITfThreadMgr,
    owner: Weak<Data>,
    theme: u32,
    size: u32,
) -> Result<Handle> {
    let data = Rc::new(Candidates {
        interface: Cell::new(std::ptr::null_mut()),
        manager: thread.cast()?,
        document: RefCell::new(None),
        snapshot: RefCell::new(Snapshot::default()),
        host: RefCell::new(None),
        window: Cell::new(HWND::default()),
        id: Cell::new(TF_INVALID_UIELEMENTID),
        native: Cell::new(true),
        revision: Cell::new(0),
        caret: Cell::new(RECT::default()),
        theme: Cell::new(theme),
        size: Cell::new(size),
        owner,
        _count: Count::new(),
    });
    // The interface owns data; data only keeps a borrowed raw pointer during TSF Begin/Update calls.
    let interface: ITfCandidateListUIElementBehavior = Element { data: data.clone() }.into();
    data.interface.set(interface.as_raw());
    if std::env::var_os("SHANJIE_HEADLESS").is_none() {
        let wc = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: HINSTANCE(super::module().0),
            lpszClassName: w!("ShanjieRustCandidateChannel"),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            wc.lpszClassName,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(wc.hInstance),
            Some(Rc::as_ptr(&data).cast()),
        )?;
        data.window.set(window);
        *data.host.borrow_mut() = Some(Host::start(window)?);
    }
    Ok(Handle {
        data,
        _interface: interface,
    })
}
impl Candidates {
    pub fn close(&self) {
        self.revision.set(self.revision.get() + 1);
        if let Some(host) = self.host.borrow_mut().as_mut() {
            host.hide();
        }
        if self.id.get() != TF_INVALID_UIELEMENTID {
            unsafe {
                let id = self.id.replace(TF_INVALID_UIELEMENTID);
                let _ = self.manager.EndUIElement(id);
            }
        }
        self.document.borrow_mut().take();
        *self.snapshot.borrow_mut() = Snapshot::default();
    }
    pub fn release(&self) {
        self.close();
    }
    pub unsafe fn update(
        &self,
        context: &ITfContext,
        cookie: u32,
        range: &ITfRange,
        state: Snapshot,
    ) -> Result<()> {
        if state.candidates.is_empty() {
            self.close();
            return Ok(());
        }
        self.revision.set(self.revision.get() + 1);
        *self.snapshot.borrow_mut() = state;
        *self.document.borrow_mut() = Some(context.GetDocumentMgr()?);
        if self.id.get() == TF_INVALID_UIELEMENTID {
            let mut show = true.into();
            let raw = self.interface.get();
            let element = ITfCandidateListUIElementBehavior::from_raw_borrowed(&raw).unwrap();
            let mut id = 0;
            self.manager.BeginUIElement(element, &mut show, &mut id)?;
            self.id.set(id);
            self.native.set(show.as_bool());
        }
        self.manager.UpdateUIElement(self.id.get())?;
        if !self.native.get() {
            self.show(false)?;
            return Ok(());
        }
        let view = context.GetActiveView()?;
        let mut rect = RECT::default();
        let mut clipped = BOOL::default();
        if view
            .GetTextExt(cookie, range, &mut rect, &mut clipped)
            .is_err()
            || clipped.as_bool()
        {
            self.show(false)?;
            return Ok(());
        }
        let owner = view.GetWnd()?;
        if !owner.is_invalid()
            && GetAwarenessFromDpiAwarenessContext(GetWindowDpiAwarenessContext(owner))
                != DPI_AWARENESS_PER_MONITOR_AWARE
        {
            let mut start = POINT {
                x: rect.left,
                y: rect.top,
            };
            let mut end = POINT {
                x: rect.right,
                y: rect.bottom,
            };
            LogicalToPhysicalPointForPerMonitorDPI(Some(owner), &mut start).ok()?;
            LogicalToPhysicalPointForPerMonitorDPI(Some(owner), &mut end).ok()?;
            rect = RECT {
                left: start.x,
                top: start.y,
                right: end.x,
                bottom: end.y,
            };
        }
        self.caret.set(rect);
        self.show(true)
    }
    fn show(&self, visible: bool) -> Result<()> {
        if let Some(host) = self.host.borrow_mut().as_mut() {
            host.publish(
                self.snapshot.borrow().clone(),
                self.theme.get(),
                self.size.get(),
                self.caret.get(),
                visible && self.native.get(),
            )?;
        }
        Ok(())
    }
    fn pick(&self, index: u32) -> Result<()> {
        if let Some(owner) = self.owner.upgrade() {
            owner.pick(index, self.revision.get())?;
        }
        Ok(())
    }
}
impl Drop for Candidates {
    fn drop(&mut self) {
        unsafe {
            if !self.window.get().is_invalid() {
                let _ = DestroyWindow(self.window.get());
            }
        }
    }
}
unsafe extern "system" fn procedure(window: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let mut p = GetWindowLongPtrW(window, GWLP_USERDATA) as *const Candidates;
    if message == WM_NCCREATE {
        p = (*(lp.0 as *const CREATESTRUCTW)).lpCreateParams.cast();
        SetWindowLongPtrW(window, GWLP_USERDATA, p as _);
    }
    if !p.is_null() && message == WM_APP + 42 {
        let c = &*p;
        let current = c
            .host
            .borrow()
            .as_ref()
            .is_some_and(|h| h.sequence == lp.0 as i32 && h.shown());
        if current && wp.0 < c.snapshot.borrow().candidates.len() {
            let _ = c.pick(wp.0 as u32);
        }
        return LRESULT(0);
    }
    DefWindowProcW(window, message, wp, lp)
}
impl ITfUIElement_Impl for Element_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        Ok("善解候選".into())
    }
    fn GetGUID(&self) -> Result<GUID> {
        Ok(crate::registration::SERVICE)
    }
    fn Show(&self, value: BOOL) -> Result<()> {
        self.data.show(value.as_bool())
    }
    fn IsShown(&self) -> Result<BOOL> {
        Ok(self
            .data
            .host
            .borrow()
            .as_ref()
            .is_some_and(|h| h.shown())
            .into())
    }
}
impl ITfCandidateListUIElement_Impl for Element_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        Ok(TF_CLUIE_COUNT
            | TF_CLUIE_SELECTION
            | TF_CLUIE_STRING
            | TF_CLUIE_PAGEINDEX
            | TF_CLUIE_CURRENTPAGE)
    }
    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        self.data.document.borrow().clone().ok_or(E_FAIL.into())
    }
    fn GetCount(&self) -> Result<u32> {
        Ok(self.data.snapshot.borrow().candidates.len() as u32)
    }
    fn GetSelection(&self) -> Result<u32> {
        Ok(self.data.snapshot.borrow().selected.unwrap_or(0) as u32)
    }
    fn GetString(&self, index: u32) -> Result<BSTR> {
        self.data
            .snapshot
            .borrow()
            .candidates
            .get(index as usize)
            .map(|s| s.as_str().into())
            .ok_or(E_INVALIDARG.into())
    }
    fn GetPageIndex(&self, index: *mut u32, size: u32, count: *mut u32) -> Result<()> {
        unsafe {
            let pages = (self.data.snapshot.borrow().candidates.len() as u32 + 8) / 9;
            *count = pages;
            for i in 0..pages.min(size) {
                *index.add(i as usize) = i * 9;
            }
            if size < pages {
                return Err(S_FALSE.into());
            }
        }
        Ok(())
    }
    fn SetPageIndex(&self, _: *const u32, _: u32) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn GetCurrentPage(&self) -> Result<u32> {
        Ok(self.data.snapshot.borrow().selected.unwrap_or(0) as u32 / 9)
    }
}
impl ITfCandidateListUIElementBehavior_Impl for Element_Impl {
    fn SetSelection(&self, index: u32) -> Result<()> {
        let mut s = self.data.snapshot.borrow_mut();
        if index as usize >= s.candidates.len() {
            return Err(E_INVALIDARG.into());
        }
        s.selected = Some(index as usize);
        drop(s);
        self.data.revision.set(self.data.revision.get() + 1);
        self.data.show(true)
    }
    fn Finalize(&self) -> Result<()> {
        let selection = self.data.snapshot.borrow().selected;
        if let Some(index) = selection {
            self.data.pick(index as u32)?;
        }
        Ok(())
    }
    fn Abort(&self) -> Result<()> {
        self.data.close();
        Ok(())
    }
}
