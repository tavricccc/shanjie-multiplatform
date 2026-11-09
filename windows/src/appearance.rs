use windows::{UI::ViewManagement::{UISettings, UIColorType}, Win32::UI::{Accessibility::*, WindowsAndMessaging::*}};
pub(crate) use shanjie_shell::appearance::{Appearance, DEFAULT_ACCENT};

pub(crate) fn system_accent() -> u32 {
    UISettings::new().and_then(|s|s.GetColorValue(UIColorType::Accent))
        .map(|c|(c.R as u32)<<16|(c.G as u32)<<8|c.B as u32).unwrap_or(DEFAULT_ACCENT)
}
pub(crate) fn high_contrast() -> bool {
    let mut value=HIGHCONTRASTW{cbSize:std::mem::size_of::<HIGHCONTRASTW>() as u32,..Default::default()};
    unsafe{SystemParametersInfoW(SPI_GETHIGHCONTRAST,value.cbSize,Some((&mut value as *mut HIGHCONTRASTW).cast()),SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0))}
        .is_ok() && value.dwFlags&HCF_HIGHCONTRASTON!=HIGHCONTRASTW_FLAGS(0)
}
