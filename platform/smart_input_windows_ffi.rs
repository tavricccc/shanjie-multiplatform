/// Preserve the Windows display character independently of the phonetic key.
#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_key_with_latin(engine: *mut ShanjieEngine, key: ShanjieKey, display: u32, out: *mut *mut ShanjieOutput) -> i32 {
    guard(|| {
        if !unsafe { clear_out(out) } || engine.is_null() { return SHANJIE_ERR_NULL; }
        let Some(key) = to_key(key) else { return SHANJIE_ERR_INVALID; };
        match unsafe { (*engine).0.key_with_latin(key, char::from_u32(display).filter(|ch| ch.is_ascii_alphabetic())) } {
            Ok(value) => unsafe { emit(value, out) },
            Err(_) => SHANJIE_ERR_INTERNAL,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_insert_space(engine: *mut ShanjieEngine, out: *mut *mut ShanjieOutput) -> i32 {
    guard(|| {
        if !unsafe { clear_out(out) } || engine.is_null() { return SHANJIE_ERR_NULL; }
        match unsafe { (*engine).0.insert_literal_space() } {
            Ok(value) => unsafe { emit(value, out) },
            Err(_) => SHANJIE_ERR_INTERNAL,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_set_mixed_input(
    engine: *mut ShanjieEngine,
    flags: u32,
) -> i32 {
    if engine.is_null() {
        return 1;
    }
    if flags > 1 {
        return 2;
    }
    unsafe { &mut (*engine).0 }.set_mixed_input(flags != 0);
    0
}

/// 0 = Windows/off policy, 1 = upstream inline English (needs shanjie_engine_load_english).
#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_set_upstream_english(engine: *mut ShanjieEngine, flags: u32) -> i32 {
    if engine.is_null() { return 1; }
    if flags > 1 { return 2; }
    match unsafe { &mut (*engine).0 }.set_upstream_english(flags != 0) {
        Ok(()) => 0,
        Err(_) => SHANJIE_ERR_INTERNAL,
    }
}

#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_is_mixed_hint(engine: *const ShanjieEngine) -> u32 {
    if engine.is_null() { return 0; }
    unsafe { &(*engine).0 }.is_mixed_hint() as u32
}

#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_is_mixed_pending(engine: *const ShanjieEngine) -> u32 {
    if engine.is_null() { return 0; }
    unsafe { &(*engine).0 }.is_mixed_pending() as u32
}

#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_phonetic_pending(engine: *const ShanjieEngine) -> u32 {
    if engine.is_null() { return 0; }
    unsafe { (*engine).0.windows_phonetic_pending() as u32 }
}
