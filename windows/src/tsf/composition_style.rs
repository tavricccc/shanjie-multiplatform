use super::{service::Service_Impl, Count};
use std::cell::Cell;
use windows::{
    core::*,
    Win32::{Foundation::*, System::Variant::VARIANT, UI::TextServices::*},
};

pub(super) const ACTIVE_WORD: GUID = GUID::from_u128(0x6bd25d69_77d7_4ec6_a86a_a608b0b4b395);
const STYLE: TF_DISPLAYATTRIBUTE = TF_DISPLAYATTRIBUTE {
    crText: TF_DA_COLOR {
        r#type: TF_CT_NONE,
        Anonymous: TF_DA_COLOR_0 { cr: COLORREF(0) },
    },
    crBk: TF_DA_COLOR {
        r#type: TF_CT_NONE,
        Anonymous: TF_DA_COLOR_0 { cr: COLORREF(0) },
    },
    lsStyle: TF_LS_SOLID,
    fBoldLine: BOOL(1),
    crLine: TF_DA_COLOR {
        r#type: TF_CT_NONE,
        Anonymous: TF_DA_COLOR_0 { cr: COLORREF(0) },
    },
    bAttr: TF_ATTR_TARGET_CONVERTED,
};

#[implement(ITfDisplayAttributeInfo, Agile = false)]
struct ActiveWord {
    style: Cell<TF_DISPLAYATTRIBUTE>,
    _count: Count,
}
impl ActiveWord {
    fn info() -> ITfDisplayAttributeInfo {
        Self {
            style: Cell::new(STYLE),
            _count: Count::new(),
        }
        .into()
    }
}
impl ITfDisplayAttributeInfo_Impl for ActiveWord_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(ACTIVE_WORD)
    }
    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("善解目前組字詞段"))
    }
    fn GetAttributeInfo(&self, value: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        unsafe {
            *value.as_mut().ok_or(E_POINTER)? = self.style.get();
        }
        Ok(())
    }
    fn SetAttributeInfo(&self, value: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        self.style.set(*unsafe { value.as_ref().ok_or(E_POINTER)? });
        Ok(())
    }
    fn Reset(&self) -> Result<()> {
        self.style.set(STYLE);
        Ok(())
    }
}

#[implement(IEnumTfDisplayAttributeInfo, Agile = false)]
struct Styles {
    consumed: Cell<bool>,
    _count: Count,
}
impl IEnumTfDisplayAttributeInfo_Impl for Styles_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(Styles {
            consumed: Cell::new(self.consumed.get()),
            _count: Count::new(),
        }
        .into())
    }
    fn Next(
        &self,
        count: u32,
        items: *mut Option<ITfDisplayAttributeInfo>,
        fetched: *mut u32,
    ) -> Result<()> {
        if items.is_null() || (fetched.is_null() && count != 1) {
            return Err(E_POINTER.into());
        }
        let n = u32::from(count != 0 && !self.consumed.replace(true));
        unsafe {
            if n == 1 {
                items.write(Some(ActiveWord::info()));
            }
            if !fetched.is_null() {
                fetched.write(n);
            }
        }
        if n == count {
            Ok(())
        } else {
            Err(S_FALSE.into())
        }
    }
    fn Reset(&self) -> Result<()> {
        self.consumed.set(false);
        Ok(())
    }
    fn Skip(&self, count: u32) -> Result<()> {
        if count == 0 || (!self.consumed.replace(true) && count == 1) {
            Ok(())
        } else {
            Err(S_FALSE.into())
        }
    }
}
impl ITfDisplayAttributeProvider_Impl for Service_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(Styles {
            consumed: Cell::new(false),
            _count: Count::new(),
        }
        .into())
    }
    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        (unsafe { guid.as_ref() } == Some(&ACTIVE_WORD))
            .then(ActiveWord::info)
            .ok_or(E_INVALIDARG.into())
    }
}

pub(super) unsafe fn mark(
    context: &ITfContext,
    cookie: u32,
    range: &ITfRange,
    atom: u32,
    [start, end]: [u32; 2],
) -> Result<()> {
    let span = range.Clone()?;
    let mut moved = 0;
    span.Collapse(cookie, TF_ANCHOR_START)?;
    span.ShiftEnd(cookie, end as i32, &mut moved, std::ptr::null())?;
    span.ShiftStart(cookie, start as i32, &mut moved, std::ptr::null())?;
    context
        .GetProperty(&GUID_PROP_ATTRIBUTE)?
        .SetValue(cookie, &span, &VARIANT::from(atom as i32))
}
