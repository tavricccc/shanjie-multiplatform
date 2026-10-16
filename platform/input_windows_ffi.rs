#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_reading(engine: *mut ShanjieEngine, commit: u32, out: *mut *mut ShanjieOutput) -> i32 {
    guard(|| {
        if !unsafe { clear_out(out) } || engine.is_null() || commit > 1 { return SHANJIE_ERR_NULL; }
        let engine = unsafe { &mut (*engine).0 };
        let output = if commit == 1 { engine.commit_candidate_reading() } else {
            let reading = engine.candidate_reading();
            Output { handled: true, commit: String::new(), cursor_utf16: reading.encode_utf16().count() as u32, preedit: reading,
                candidates: Vec::new(), selected: None, columns: 0, first: 0, total: 0, vertical: 0, english_run_utf16_start: 0, english_run_utf16_len: 0 }
        };
        unsafe { emit(output,out) }
    })
}
#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_is_prediction_row(engine: *mut ShanjieEngine) -> u32 {
    if engine.is_null() { return 0; }
    unsafe { (*engine).0.is_prediction_row() as u32 }
}
#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_set_candidate_charset(engine: *mut ShanjieEngine, big5: u32) -> i32 {
    guard(|| {
        if engine.is_null() || big5 > 1 { return SHANJIE_ERR_NULL; }
        unsafe { (*engine).0.set_candidate_charset(big5 == 1); } SHANJIE_OK
    })
}
#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_associate(engine: *mut ShanjieEngine, prefix: *const c_char, out: *mut *mut ShanjieOutput) -> i32 {
    guard(|| {
        if !unsafe { clear_out(out) } || engine.is_null() || prefix.is_null() { return SHANJIE_ERR_NULL; }
        let Ok(prefix) = unsafe { CStr::from_ptr(prefix) }.to_str() else { return SHANJIE_ERR_NULL; };
        unsafe { emit((*engine).0.associated_words(prefix),out) }
    })
}
#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_engine_new(data: *const c_char, personal: *const c_char, layout: u32, out: *mut *mut ShanjieEngine) -> i32 {
    guard(|| {
        if !unsafe { clear_out(out) } || data.is_null() { return SHANJIE_ERR_NULL; }
        let Ok(data) = unsafe { CStr::from_ptr(data) }.to_str() else { return SHANJIE_ERR_NULL; };
        let layout = match layout { 0 => Layout::Standard, 1 => Layout::Eten, _ => return SHANJIE_ERR_NULL };
        let personal = if personal.is_null() { None } else {
            let Ok(text) = unsafe { CStr::from_ptr(personal) }.to_str() else { return SHANJIE_ERR_NULL; };
            Some(Path::new(text))
        };
        match Engine::new_with_personal(Path::new(data),layout,personal) {
            Ok(engine) => { unsafe { *out = Box::into_raw(Box::new(ShanjieEngine(engine))); } SHANJIE_OK },
            Err(_) => SHANJIE_ERR_LOAD,
        }
    })
}
