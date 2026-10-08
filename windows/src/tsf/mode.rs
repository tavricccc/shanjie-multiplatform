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
#[implement(ITfLangBarItemButton, ITfSource)]
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
