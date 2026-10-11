/// Set Windows smart input policies. Caller commits the old composition before changing options.
#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_set_smart_input(
    engine: *mut ShanjieEngine,
    flags: u32,
) -> i32 {
    if engine.is_null() {
        return 1;
    }
    if flags & !3 != 0 {
        return 2;
    }
    unsafe { &mut (*engine).0 }.set_smart_input(flags & 1 != 0, flags & 2 != 0);
    0
}

#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_is_mixed_pending(engine: *const ShanjieEngine) -> u32 {
    if engine.is_null() { return 0; }
    unsafe { &(*engine).0 }.is_mixed_pending() as u32
}
