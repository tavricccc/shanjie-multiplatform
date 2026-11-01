//! The font every Shanjie surface uses: Noto Sans when it is installed, otherwise Windows' Traditional Chinese UI font.

use std::{ffi::CString, sync::OnceLock};
use windows::{
    core::{BOOL, HSTRING},
    Win32::Graphics::DirectWrite::*,
};

/// Noto Sans families in order of preference: Traditional Chinese first, then the Latin-only family (DirectWrite and
/// WinUI fall back to the system font for Han characters).
const PREFERRED: [&str; 5] = ["Noto Sans TC", "Noto Sans CJK TC", "Noto Sans HK", "Noto Sans CJK HK", "Noto Sans"];
const FALLBACK: &str = "Microsoft JhengHei UI";

/// The installed Noto Sans family, if any. Looked up once per process.
pub fn noto() -> Option<&'static str> {
    static FOUND: OnceLock<Option<&'static str>> = OnceLock::new();
    *FOUND.get_or_init(|| unsafe {
        let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()?;
        let mut fonts = None;
        factory.GetSystemFontCollection(&mut fonts, false).ok()?;
        let fonts = fonts?;
        PREFERRED.into_iter().find(|name| {
            let (mut index, mut exists) = (0, BOOL(0));
            fonts.FindFamilyName(&HSTRING::from(*name), &mut index, &mut exists).is_ok() && exists.as_bool()
        })
    })
}

/// The family for Chinese text.
pub fn family() -> HSTRING {
    HSTRING::from(noto().unwrap_or(FALLBACK))
}

/// The family for digits and Latin labels.
pub fn label_family() -> HSTRING {
    HSTRING::from(noto().unwrap_or("Segoe UI"))
}

/// The Noto Sans family for Shanjie.UI's WinUI windows (UTF-8), or null to keep the system default.
#[no_mangle]
pub extern "C" fn sj_ui_font() -> *const std::ffi::c_char {
    static NAME: OnceLock<Option<CString>> = OnceLock::new();
    NAME.get_or_init(|| noto().and_then(|n| CString::new(n).ok()))
        .as_ref()
        .map_or(std::ptr::null(), |n| n.as_ptr())
}

#[cfg(test)]
mod tests {
    #[test]
    fn family_is_noto_or_the_fallback() {
        let name = super::family().to_string();
        assert!(name.starts_with("Noto Sans") || name == super::FALLBACK);
    }
}
