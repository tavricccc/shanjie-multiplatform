use super::{service::Data, Count};
use std::{
    cell::{Cell, RefCell},
    rc::Weak,
};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        UI::{TextServices::*, WindowsAndMessaging::*},
    },
};
pub(crate) unsafe fn icon(chinese: bool) -> Result<HICON> {
    let id = if chinese { 101 } else { 102 };
    let handle = LoadImageW(
        Some(HINSTANCE(super::module().0)),
        PCWSTR(id as *const u16),
        IMAGE_ICON,
        GetSystemMetrics(SM_CXSMICON),
        GetSystemMetrics(SM_CYSMICON),
        IMAGE_FLAGS(0),
    )?;
    Ok(HICON(handle.0))
}
#[no_mangle]
pub unsafe extern "C" fn sj_mode_icon(chinese: bool) -> isize {
    icon(chinese).map_or(0, |icon| icon.0 as isize)
}
// This object holds apartment-owned Rc/RefCell state; COM must dispatch to its STA.
#[implement(ITfLangBarItemButton, ITfSource, Agile = false)]
pub struct Button {
    owner: Weak<Data>,
    shown: Cell<bool>,
    sink: RefCell<Option<ITfLangBarItemSink>>,
    _count: Count,
}
pub fn new(owner: Weak<Data>) -> ITfLangBarItemButton {
    Button {
        owner,
        shown: Cell::new(true),
        sink: RefCell::new(None),
        _count: Count::new(),
    }
    .into()
}
pub fn refresh(button: &ITfLangBarItemButton) {
    unsafe {
        let object: &Button = button.as_impl();
        if let Some(sink) = object.sink.borrow().as_ref() {
            let _ = sink.OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_STATUS);
        }
    }
}
impl ITfLangBarItem_Impl for Button_Impl {
    fn GetInfo(&self, out: *mut TF_LANGBARITEMINFO) -> Result<()> {
        unsafe {
            *out = TF_LANGBARITEMINFO {
                clsidService: crate::registration::SERVICE,
                guidItem: GUID_LBI_INPUTMODE,
                dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY,
                ..Default::default()
            };
            let text: Vec<_> = "善解中英切換".encode_utf16().collect();
            (&mut (*out).szDescription)[..text.len()].copy_from_slice(&text);
        }
        Ok(())
    }
    fn GetStatus(&self) -> Result<u32> {
        Ok(if self.shown.get() {
            0
        } else {
            TF_LBI_STATUS_HIDDEN
        })
    }
    fn Show(&self, value: BOOL) -> Result<()> {
        self.shown.set(value.as_bool());
        if let Some(sink) = self.sink.borrow().as_ref() {
            unsafe {
                sink.OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_STATUS)?;
            }
        }
        Ok(())
    }
    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok("善解：點一下切換中英；右鍵開啟設定".into())
    }
}
impl ITfLangBarItemButton_Impl for Button_Impl {
    fn OnClick(&self, click: TfLBIClick, _: &POINT, _: *const RECT) -> Result<()> {
        if let Some(owner) = self.owner.upgrade() {
            if click == TF_LBI_CLK_RIGHT {
                owner.settings();
            } else {
                owner.toggle(None)?;
            }
        }
        Ok(())
    }
    fn InitMenu(&self, menu: Ref<ITfMenu>) -> Result<()> {
        unsafe {
            menu.ok()?.AddMenuItem(
                1,
                0,
                Default::default(),
                Default::default(),
                &"善解設定".encode_utf16().collect::<Vec<_>>(),
                std::ptr::null_mut(),
            )
        }
    }
    fn OnMenuSelect(&self, id: u32) -> Result<()> {
        if id == 1 {
            if let Some(owner) = self.owner.upgrade() {
                owner.settings();
            }
        }
        Ok(())
    }
    fn GetText(&self) -> Result<BSTR> {
        Ok(if self.owner.upgrade().is_some_and(|o| o.chinese()) {
            "中"
        } else {
            "英"
        }
        .into())
    }
    fn GetIcon(&self) -> Result<HICON> {
        unsafe { icon(self.owner.upgrade().is_some_and(|o| o.chinese())) }
    }
}
impl ITfSource_Impl for Button_Impl {
    fn AdviseSink(&self, id: *const GUID, sink: Ref<IUnknown>) -> Result<u32> {
        unsafe {
            if *id != ITfLangBarItemSink::IID {
                return Err(E_NOINTERFACE.into());
            }
            *self.sink.borrow_mut() = Some(sink.ok()?.cast()?);
        }
        Ok(1)
    }
    fn UnadviseSink(&self, cookie: u32) -> Result<()> {
        if cookie != 1 {
            return Err(E_INVALIDARG.into());
        }
        self.sink.borrow_mut().take();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::{AtomicU32, Ordering}, Arc};
    use windows::Win32::System::{Com::*, Threading::GetCurrentThreadId};

    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoMarshalInterThreadInterfaceInStream(iid: *const GUID, object: *mut std::ffi::c_void, stream: *mut *mut std::ffi::c_void) -> HRESULT;
        fn CoGetInterfaceAndReleaseStream(stream: *mut std::ffi::c_void, iid: *const GUID, object: *mut *mut std::ffi::c_void) -> HRESULT;
    }

    // These recorders are deliberately agile and contain only thread-safe state.
    // A call into one records the thread on which the production object ran.
    #[implement(ITfLangBarItemSink)]
    struct ButtonObserver(Arc<AtomicU32>);
    impl ITfLangBarItemSink_Impl for ButtonObserver_Impl {
        fn OnUpdate(&self, _: u32) -> Result<()> {
            self.0.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
            Ok(())
        }
    }

    #[implement(ITfEditRecord)]
    struct EditRecord(Arc<AtomicU32>);
    impl ITfEditRecord_Impl for EditRecord_Impl {
        fn GetSelectionStatus(&self) -> Result<BOOL> {
            self.0.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
            Ok(BOOL(0))
        }
        fn GetTextAndPropertyUpdates(&self, _: GET_TEXT_AND_PROPERTY_UPDATES_FLAGS, _: *const *const GUID, _: u32) -> Result<IEnumTfRanges> {
            Err(E_NOTIMPL.into())
        }
    }

    #[test]
    fn marshaled_button_and_service_callbacks_stay_on_owner_thread() {
        std::thread::spawn(|| unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
            let owner_thread = GetCurrentThreadId();
            let button_thread = Arc::new(AtomicU32::new(0));
            let edit_thread = Arc::new(AtomicU32::new(0));
            let button = new(Weak::new());
            let source: ITfSource = button.cast().unwrap();
            let observer: ITfLangBarItemSink = ButtonObserver(button_thread.clone()).into();
            let cookie = source.AdviseSink(&ITfLangBarItemSink::IID, &observer).unwrap();
            let service = super::super::service::new();
            let edits: ITfTextEditSink = service.cast().unwrap();
            let mut button_stream = std::ptr::null_mut();
            CoMarshalInterThreadInterfaceInStream(&ITfLangBarItemButton::IID, button.as_raw(), &mut button_stream).ok().unwrap();
            let mut edit_stream = std::ptr::null_mut();
            CoMarshalInterThreadInterfaceInStream(&ITfTextEditSink::IID, edits.as_raw(), &mut edit_stream).ok().unwrap();
            let streams = (button_stream as usize, edit_stream as usize);
            let edit_observation = edit_thread.clone();
            let (send, receive) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
                let result = (|| -> Result<()> {
                    let mut raw = std::ptr::null_mut();
                    CoGetInterfaceAndReleaseStream(streams.0 as _, &ITfLangBarItemButton::IID, &mut raw).ok()?;
                    let proxy = ITfLangBarItemButton::from_raw(raw);
                    proxy.Show(true)?;
                    let mut raw = std::ptr::null_mut();
                    CoGetInterfaceAndReleaseStream(streams.1 as _, &ITfTextEditSink::IID, &mut raw).ok()?;
                    let proxy = ITfTextEditSink::from_raw(raw);
                    let record: ITfEditRecord = EditRecord(edit_observation).into();
                    proxy.OnEndEdit(None, 0, &record)?;
                    Ok(())
                })();
                CoUninitialize();
                send.send(result.map_err(|error| error.code().0)).unwrap();
            });
            let started = std::time::Instant::now();
            let result = loop {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                if let Ok(result) = receive.try_recv() { break result; }
                assert!(started.elapsed() < std::time::Duration::from_secs(5), "cross-apartment callbacks timed out");
                std::thread::sleep(std::time::Duration::from_millis(1));
            };
            worker.join().unwrap();
            result.unwrap();
            let observed = (button_thread.load(Ordering::SeqCst), edit_thread.load(Ordering::SeqCst));
            eprintln!("COM callback threads: owner={owner_thread}, button={}, service={}", observed.0, observed.1);
            source.UnadviseSink(cookie).unwrap();
            drop(observer);
            drop(source);
            drop(button);
            drop(edits);
            drop(service);
            CoUninitialize();
            assert_eq!(observed, (owner_thread, owner_thread), "COM dispatched apartment-owned state onto another thread");
        }).join().unwrap();
    }
}
