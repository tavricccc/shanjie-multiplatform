#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_set_input_options(engine: *mut ShanjieEngine, flags: u32) -> i32 {
    guard(|| {
        if engine.is_null() || flags >> 21 != 0 { return SHANJIE_ERR_INVALID; }
        unsafe { (*engine).0.set_windows_input_options(flags); }
        SHANJIE_OK
    })
}
#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_punctuation(engine: *mut ShanjieEngine, text: *const c_char, out: *mut *mut ShanjieOutput) -> i32 {
    guard(|| {
        if !unsafe { clear_out(out) } || engine.is_null() || text.is_null() { return SHANJIE_ERR_NULL; }
        let Ok(mark) = unsafe { CStr::from_ptr(text) }.to_str() else { return SHANJIE_ERR_INVALID; };
        if mark.chars().count() != 1 { return SHANJIE_ERR_INVALID; }
        match unsafe { (*engine).0.insert_windows_punctuation(mark) } {
            Ok(value) => unsafe { emit(value,out) },
            Err(_) => SHANJIE_ERR_INTERNAL,
        }
    })
}
