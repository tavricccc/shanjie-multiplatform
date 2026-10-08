//! Narrow UTF-8 C ABI for the TSF and WinUI platform bindings. Owners stay in Rust.
use crate::{
    candidate::Host,
    engine::{Engine, Snapshot},
    input::{self, Associated, Pinyin, Special},
    profile::{self, Document},
};
use shanjie_core::ffi::*;
use std::{
    cell::RefCell,
    ffi::{c_char, CStr, CString},
    path::Path,
    ptr,
};
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::Input::KeyboardAndMouse::*,
};
thread_local! {static ERROR:RefCell<CString>=RefCell::new(CString::default());}
pub(crate) fn failure(e: impl ToString) {
    ERROR.with(|v| *v.borrow_mut() = CString::new(e.to_string()).unwrap());
}
unsafe fn text<'a>(p: *const c_char) -> &'a str {
    CStr::from_ptr(p).to_str().unwrap()
}
#[no_mangle]
pub extern "C" fn sj_error() -> *const c_char {
    ERROR.with(|v| v.borrow().as_ptr())
}
#[no_mangle]
pub unsafe extern "C" fn sj_string_free(p: *mut c_char) {
    if !p.is_null() {
        drop(CString::from_raw(p));
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_profile_load_text() -> *mut c_char {
    match profile::load(&profile::path()) {
        Ok(d) => CString::new(d.content()).unwrap().into_raw(),
        Err(e) => {
            failure(e);
            ptr::null_mut()
        }
    }
}
#[no_mangle]
pub extern "C" fn sj_profile_stamp() -> u64 {
    use std::os::windows::fs::MetadataExt;
    std::fs::metadata(profile::path())
        .map(|m| m.last_write_time())
        .unwrap_or(0)
}
#[no_mangle]
pub unsafe extern "C" fn sj_document_json(content: *const c_char) -> *mut c_char {
    match Document::parse(text(content)) {
        Ok(d) => CString::new(serde_json::to_string(&d.0).unwrap())
            .unwrap()
            .into_raw(),
        Err(e) => {
            failure(e);
            ptr::null_mut()
        }
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_profile_save(content: *const c_char) -> bool {
    match profile::save(&profile::path(), text(content)) {
        Ok(()) => true,
        Err(e) => {
            failure(e);
            false
        }
    }
}
#[no_mangle]
pub extern "C" fn sj_clear_learning() -> bool {
    match profile::clear_learning() {
        Ok(()) => true,
        Err(e) => {
            failure(e);
            false
        }
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_profile_preset(content: *const c_char, windows: bool) -> *mut c_char {
    match profile::preset(text(content), windows) {
        Ok(s) => CString::new(s).unwrap().into_raw(),
        Err(e) => {
            failure(e);
            ptr::null_mut()
        }
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_dictionary_save(
    content: *const c_char,
    folder: *const c_char,
) -> *mut c_char {
    match profile::save_dictionary(text(content), Path::new(text(folder))) {
        Ok(s) => CString::new(s).unwrap().into_raw(),
        Err(e) => {
            failure(e);
            ptr::null_mut()
        }
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_document_open(content: *const c_char) -> *mut Document {
    match Document::parse(text(content)) {
        Ok(d) => Box::into_raw(Box::new(d)),
        Err(e) => {
            failure(e);
            ptr::null_mut()
        }
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_document_free(p: *mut Document) {
    drop(Box::from_raw(p));
}
#[no_mangle]
pub unsafe extern "C" fn sj_document_get(
    p: *const Document,
    s: *const c_char,
    k: *const c_char,
    default: *const c_char,
) -> *mut c_char {
    CString::new((*p).get(text(s), text(k), text(default)))
        .unwrap()
        .into_raw()
}

#[repr(C)]
pub struct Frame {
    pub handled: u32,
    pub associated: u32,
    pub prediction: u32,
    pub commit: *const c_char,
    pub preedit: *const c_char,
    pub cursor: u32,
    pub columns: u32,
    pub first: u32,
    pub total: u32,
    pub selected: i32,
    pub count: u32,
    pub candidates: *const *const c_char,
}
#[repr(C)]
struct OwnedFrame {
    raw: Frame,
    _strings: Vec<CString>,
    _pointers: Vec<*const c_char>,
}
fn frame(s: Snapshot) -> *mut Frame {
    let strings: Vec<_> = std::iter::once(s.commit)
        .chain(Some(s.preedit))
        .chain(s.candidates)
        .map(|v| CString::new(v).unwrap())
        .collect();
    let pointers: Vec<_> = strings[2..].iter().map(|s| s.as_ptr()).collect();
    let raw = Frame {
        handled: s.handled as u32,
        associated: s.associated as u32,
        prediction: s.prediction as u32,
        commit: strings[0].as_ptr(),
        preedit: strings[1].as_ptr(),
        cursor: s.cursor,
        columns: s.columns,
        first: s.first,
        total: s.total,
        selected: s.selected.map_or(-1, |i| i as i32),
        count: pointers.len() as u32,
        candidates: pointers.as_ptr(),
    };
    Box::into_raw(Box::new(OwnedFrame {
        raw,
        _strings: strings,
        _pointers: pointers,
    }))
    .cast()
}
unsafe fn snapshot(r: *const Frame) -> Snapshot {
    let r = &*r;
    Snapshot {
        handled: r.handled != 0,
        associated: r.associated != 0,
        prediction: r.prediction != 0,
        commit: text(r.commit).into(),
        preedit: text(r.preedit).into(),
        cursor: r.cursor,
        columns: r.columns,
        first: r.first,
        total: r.total,
        selected: usize::try_from(r.selected).ok(),
        candidates: (0..r.count as usize)
            .map(|i| text(*r.candidates.add(i)).into())
            .collect(),
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_frame_free(p: *mut Frame) {
    if !p.is_null() {
        drop(Box::from_raw(p.cast::<OwnedFrame>()));
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_engine_new(
    data: *const c_char,
    layout: u32,
    ranking: u32,
    flags: u32,
    dictionary: *const c_char,
) -> *mut Engine {
    Engine::open(
        Path::new(text(data)),
        layout,
        ranking,
        flags & 1 != 0,
        flags & 2 != 0,
        flags & 4 != 0,
        text(dictionary),
        flags & 8 != 0,
    )
    .map_or(ptr::null_mut(), |e| Box::into_raw(Box::new(e)))
}
#[no_mangle]
pub unsafe extern "C" fn sj_engine_free(p: *mut Engine) {
    if !p.is_null() {
        drop(Box::from_raw(p));
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_engine_action(
    p: *mut Engine,
    action: u32,
    key: ShanjieKey,
    value: u32,
    string: *const c_char,
) -> *mut Frame {
    if p.is_null() {
        return ptr::null_mut();
    }
    let e = &mut *p;
    let out = match action {
        0 => e.key(key),
        1 => e.operation(|h, o| shanjie_engine_pick(h, value, o)),
        2 => e.reset(value != 0),
        3 => {
            let mut o = e.operation(|h, o| shanjie_windows_associate(h, string, o));
            if let Some(s) = o.as_mut() {
                s.associated = true;
            }
            o
        }
        4 => e.operation(|h, o| shanjie_windows_reading(h, 1, o)),
        5 => e.operation(|h, o| shanjie_engine_set_prediction(h, value, o)),
        6 => e.operation(|h, o| shanjie_engine_set_demote(h, value, o)),
        7 => e.operation(|h, o| shanjie_windows_punctuation(h, string, o)),
        8 => e.operation(|h, o| shanjie_windows_cycle_candidate(h, o)),
        _ => None,
    };
    out.map_or(ptr::null_mut(), frame)
}
#[no_mangle]
pub unsafe extern "C" fn sj_engine_context(p: *mut Engine, value: *const c_char) {
    shanjie_engine_set_left_context((*p).handle, value);
}
#[no_mangle]
pub unsafe extern "C" fn sj_engine_options(p: *mut Engine, value: u32) -> bool {
    !p.is_null() && shanjie_windows_set_input_options((*p).handle, value) == 0
}
#[no_mangle]
pub unsafe extern "C" fn sj_translate(vk: usize, flags: isize, out: *mut ShanjieKey) -> bool {
    let state = |k| GetKeyState(k) as u16 & 0x8000 != 0;
    let mut k = ShanjieKey {
        kind: 0,
        ch: 0,
        modifiers: state(VK_SHIFT.0 as i32) as u32
            | ((state(VK_CONTROL.0 as i32) as u32) << 1)
            | ((state(VK_MENU.0 as i32) as u32) << 2)
            | (((state(VK_LWIN.0 as i32) || state(VK_RWIN.0 as i32)) as u32) << 3)
            | (((GetKeyState(VK_CAPITAL.0 as i32) & 1 != 0) as u32) << 4),
    };
    k.kind = match vk as u16 {
        32 => 2,
        13 => 3,
        8 => 4,
        46 => 5,
        27 => 6,
        37 => 7,
        39 => 8,
        38 => 9,
        40 => 10,
        36 => 11,
        35 => 12,
        9 => 13,
        _ => {
            let keyboard = [0u8; 256];
            let mut chars = [0u16; 8];
            let count = ToUnicodeEx(
                vk as u32,
                ((flags >> 16) & 255) as u32,
                &keyboard,
                &mut chars,
                4,
                Some(GetKeyboardLayout(0)),
            );
            if count != 1 || !(32..=126).contains(&chars[0]) {
                return false;
            }
            k.ch = chars[0] as u32;
            1
        }
    };
    *out = k;
    true
}
#[no_mangle]
pub unsafe extern "C" fn sj_candidate_key(
    k: ShanjieKey,
    s: *const Frame,
    windows: bool,
) -> ShanjieKey {
    input::candidate_key(k, &snapshot(s), windows)
}
#[no_mangle]
pub extern "C" fn sj_punctuation(k: ShanjieKey) -> *const c_char {
    match input::punctuation(k) {
        "" => c"".as_ptr(),
        s => {
            ERROR.with(|v| *v.borrow_mut() = CString::new(s).unwrap());
            sj_error()
        }
    }
}
#[no_mangle]
pub extern "C" fn sj_is_punctuation(k: ShanjieKey) -> bool {
    input::is_punctuation(k)
}
#[no_mangle]
pub unsafe extern "C" fn sj_pending(s: *const Frame) -> bool {
    input::pending(&snapshot(s))
}

#[no_mangle]
pub extern "C" fn sj_special_new() -> *mut Special {
    Box::into_raw(Box::new(Special::default()))
}
#[no_mangle]
pub unsafe extern "C" fn sj_special_free(p: *mut Special) {
    drop(Box::from_raw(p));
}
#[no_mangle]
pub unsafe extern "C" fn sj_special_mode(p: *const Special) -> u32 {
    (*p).mode
}
#[no_mangle]
pub unsafe extern "C" fn sj_special_clear(p: *mut Special) {
    (*p).clear();
}
#[no_mangle]
pub unsafe extern "C" fn sj_special_starts(k: ShanjieKey, profile: *const c_char) -> bool {
    Special::starts(k, &Document::parse(text(profile)).unwrap())
}
#[no_mangle]
pub unsafe extern "C" fn sj_special_key(
    p: *mut Special,
    k: ShanjieKey,
    profile: *const c_char,
) -> *mut Frame {
    frame((*p).key(k, &Document::parse(text(profile)).unwrap()))
}
#[no_mangle]
pub unsafe extern "C" fn sj_special_pick(p: *mut Special, index: u32) -> *mut Frame {
    frame((*p).pick(index as usize))
}
#[no_mangle]
pub extern "C" fn sj_associated_new() -> *mut Associated {
    Box::into_raw(Box::new(Associated::default()))
}
#[no_mangle]
pub unsafe extern "C" fn sj_associated_free(p: *mut Associated) {
    drop(Box::from_raw(p));
}
#[no_mangle]
pub unsafe extern "C" fn sj_associated_active(p: *const Associated) -> bool {
    (*p).active()
}
#[no_mangle]
pub unsafe extern "C" fn sj_associated_clear(p: *mut Associated) {
    (*p).clear();
}
#[no_mangle]
pub unsafe extern "C" fn sj_associated_open(
    p: *mut Associated,
    words: *const Frame,
    base: *const Frame,
) {
    (*p).open(snapshot(words).candidates, snapshot(base));
}
#[no_mangle]
pub unsafe extern "C" fn sj_associated_view(p: *const Associated) -> *mut Frame {
    frame((*p).view())
}
#[no_mangle]
pub unsafe extern "C" fn sj_associated_handles(p: *const Associated, k: ShanjieKey) -> bool {
    (*p).handles(k)
}
#[no_mangle]
pub unsafe extern "C" fn sj_associated_key(p: *mut Associated, k: ShanjieKey) -> *mut Frame {
    frame((*p).key(k))
}
#[no_mangle]
pub unsafe extern "C" fn sj_associated_pick(p: *mut Associated, index: u32) -> *mut Frame {
    frame((*p).pick(index as usize))
}
#[no_mangle]
pub extern "C" fn sj_pinyin_new() -> *mut Pinyin {
    Box::into_raw(Box::new(Pinyin::default()))
}
#[no_mangle]
pub unsafe extern "C" fn sj_pinyin_free(p: *mut Pinyin) {
    drop(Box::from_raw(p));
}
#[no_mangle]
pub unsafe extern "C" fn sj_pinyin_load(p: *mut Pinyin, data: *const c_char, taiwan: bool) -> bool {
    match Pinyin::load(Path::new(text(data)), taiwan) {
        Ok(value) => {
            *p = value;
            true
        }
        Err(e) => {
            failure(e);
            false
        }
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_pinyin_enabled(p: *const Pinyin) -> bool {
    (*p).enabled()
}
#[no_mangle]
pub unsafe extern "C" fn sj_pinyin_active(p: *const Pinyin) -> bool {
    !(&*p).pending.is_empty()
}
#[no_mangle]
pub unsafe extern "C" fn sj_pinyin_handles(p: *const Pinyin, k: ShanjieKey) -> bool {
    (*p).handles(k)
}
#[no_mangle]
pub unsafe extern "C" fn sj_pinyin_clear(p: *mut Pinyin) {
    (*p).clear();
}
#[no_mangle]
pub unsafe extern "C" fn sj_pinyin_key(
    p: *mut Pinyin,
    k: ShanjieKey,
    e: *mut Engine,
    s: *const Frame,
    ascii: bool,
    space: bool,
) -> *mut Frame {
    (*p).key(k, &mut *e, snapshot(s), ascii, space)
        .map_or(ptr::null_mut(), frame)
}

#[no_mangle]
pub extern "C" fn sj_host_new(callback: isize) -> *mut Host {
    match Host::start(HWND(callback as _)) {
        Ok(h) => Box::into_raw(h),
        Err(e) => {
            failure(e);
            ptr::null_mut()
        }
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_host_free(p: *mut Host) {
    if !p.is_null() {
        drop(Box::from_raw(p));
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_host_publish(
    p: *mut Host,
    s: *const Frame,
    theme: u32,
    size: u32,
    caret: RECT,
    visible: bool,
) {
    if let Err(e) = (*p).publish(snapshot(s), theme, size, caret, visible) {
        (*p).hide();
        failure(e);
    }
}
#[no_mangle]
pub unsafe extern "C" fn sj_host_hide(p: *mut Host) {
    (*p).hide();
}
#[no_mangle]
pub unsafe extern "C" fn sj_host_shown(p: *const Host) -> bool {
    (*p).shown()
}
#[no_mangle]
pub unsafe extern "C" fn sj_host_window(p: *const Host) -> isize {
    (*p).window.0 as isize
}
#[no_mangle]
pub unsafe extern "C" fn sj_host_sequence(p: *const Host) -> i32 {
    (*p).sequence
}
