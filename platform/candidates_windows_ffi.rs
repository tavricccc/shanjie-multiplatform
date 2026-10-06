/// Windows extension: 0 preserves upstream's left anchor; 1 selects to the right.
/// Call before composition starts, using the same live-handle contract as core ABI.
#[no_mangle]
pub unsafe extern "C" fn shanjie_windows_set_candidate_anchor(engine: *mut ShanjieEngine, right: u32) -> i32 {
    guard(|| {
        if engine.is_null() { return SHANJIE_ERR_NULL; }
        if right > 1 { return SHANJIE_ERR_INVALID; }
        unsafe { (*engine).0.set_candidate_anchor(right != 0); }
        SHANJIE_OK
    })
}
