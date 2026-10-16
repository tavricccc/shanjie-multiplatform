//! Thread-scoped, pass-through chord observation. No text or keystrokes are stored.
use std::cell::Cell;
use windows::{core::Result, Win32::{Foundation::*, System::Threading::GetCurrentThreadId,
    UI::WindowsAndMessaging::*}};
thread_local! {
    static EPOCH: Cell<u64> = const { Cell::new(0) };
    static HOOK: Cell<(HHOOK, u32)> = const { Cell::new((HHOOK(std::ptr::null_mut()), 0)) };
}
pub fn epoch() -> u64 { EPOCH.with(Cell::get) }
pub struct Observer;
impl Observer {
    pub fn new() -> Result<Self> {
        HOOK.with(|slot| unsafe {
            let (mut hook, count) = slot.get();
            if count == 0 { hook = SetWindowsHookExW(WH_GETMESSAGE, Some(observe), None, GetCurrentThreadId())?; }
            slot.set((hook, count + 1)); Ok(Self)
        })
    }
}
impl Drop for Observer {
    fn drop(&mut self) { HOOK.with(|slot| unsafe {
        let (hook, count) = slot.get();
        if count <= 1 { let _ = UnhookWindowsHookEx(hook); slot.set((HHOOK::default(), 0)); }
        else { slot.set((hook, count - 1)); }
    }); }
}
unsafe extern "system" fn observe(code: i32, removed: WPARAM, message: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && removed.0 == PM_REMOVE.0 as usize && message.0 != 0 {
        let key=&*(message.0 as *const MSG);
        if [WM_KEYDOWN,WM_KEYUP,WM_SYSKEYDOWN,WM_SYSKEYUP].contains(&key.message) && !super::keys::shift(key.wParam) {
            EPOCH.with(|value| value.set(value.get().wrapping_add(1)));
        }
    }
    CallNextHookEx(None, code, removed, message)
}
