/// Set Windows smart input policies. Caller commits the old composition before changing options.
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
