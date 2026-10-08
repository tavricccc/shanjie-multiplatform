use windows::{
    core::{Result, GUID, PCWSTR},
    Win32::{
        Foundation::*,
        System::{Com::*, LibraryLoader::GetModuleFileNameW, Registry::*},
        UI::TextServices::*,
    },
};
pub const SERVICE: GUID = GUID::from_u128(0x8912696e8a314df3a7bc0261f3b6a147);
const STANDARD: GUID = GUID::from_u128(0xac59a613f8134dadba97d221a87a6ace);
const ETEN: GUID = GUID::from_u128(0x338145b4047a4564a0de223dc22c1ae1);
const CATEGORIES: [GUID; 4] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
];
unsafe fn value(path: &str, name: PCWSTR, text: &str) -> Result<()> {
    let mut key = HKEY::default();
    RegCreateKeyExW(
        HKEY_LOCAL_MACHINE,
        PCWSTR(crate::wide(path).as_ptr()),
        Some(0),
        None,
        REG_OPTION_NON_VOLATILE,
        KEY_WRITE,
        None,
        &mut key,
        None,
    )
    .ok()?;
    let wide = crate::wide(text);
    let bytes = std::slice::from_raw_parts(wide.as_ptr().cast(), wide.len() * 2);
    let result = RegSetValueExW(key, name, Some(0), REG_SZ, Some(bytes));
    let _ = RegCloseKey(key);
    result.ok()
}
pub unsafe fn register(module: HMODULE, remove: bool) -> Result<()> {
    let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    let result = (|| -> Result<()> {
        let profiles: ITfInputProcessorProfiles =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
        let categories: ITfCategoryMgr =
            CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
        let path = format!("Software\\Classes\\CLSID\\{}", crate::installer::SERVICE);
        if remove {
            let result = profiles.Unregister(&SERVICE);
            for category in CATEGORIES {
                let _ = categories.UnregisterCategory(&SERVICE, &category, &SERVICE);
            }
            let _ = RegDeleteTreeW(HKEY_LOCAL_MACHINE, PCWSTR(crate::wide(&path).as_ptr()));
            return result;
        }
        let mut buffer = [0u16; 32768];
        let count = GetModuleFileNameW(Some(module), &mut buffer);
        if count == 0 || count as usize == buffer.len() {
            return Err(windows::core::Error::from_thread());
        }
        let file = String::from_utf16(&buffer[..count as usize]).unwrap();
        value(&path, PCWSTR::null(), "善解 Windows")?;
        value(&(path.clone() + "\\InprocServer32"), PCWSTR::null(), &file)?;
        value(
            &(path + "\\InprocServer32"),
            PCWSTR(crate::wide("ThreadingModel").as_ptr()),
            "Apartment",
        )?;
        profiles.Register(&SERVICE)?;
        for (id, description) in [(STANDARD, "善解注音（標準）"), (ETEN, "善解注音（倚天）")]
        {
            profiles.AddLanguageProfile(
                &SERVICE,
                0x0404,
                &id,
                &description.encode_utf16().collect::<Vec<_>>(),
                &buffer[..count as usize],
                0,
            )?;
            profiles.EnableLanguageProfile(&SERVICE, 0x0404, &id, true)?;
        }
        for category in CATEGORIES {
            categories.RegisterCategory(&SERVICE, &category, &SERVICE)?;
        }
        Ok(())
    })();
    if initialized.is_ok() {
        CoUninitialize();
    }
    result
}
#[no_mangle]
pub unsafe extern "C" fn sj_register(module: isize, remove: bool) -> i32 {
    match register(HMODULE(module as _), remove) {
        Ok(()) => 0,
        Err(e) => e.code().0,
    }
}
