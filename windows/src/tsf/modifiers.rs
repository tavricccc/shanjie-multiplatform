//! Thread-scoped, pass-through chord observation. No text or keystrokes are stored.
use std::cell::Cell;
use windows::{core::Result, Win32::{Foundation::*, System::Threading::GetCurrentThreadId,
    UI::{WindowsAndMessaging::*, Input::KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VSC_TO_VK_EX}}}};
#[derive(Clone, Copy, Default)]
struct Activity { shift: u64, control: u64 }
#[derive(Clone, Copy, PartialEq, Debug)]
enum Modifier { Shift, Control, Other }
fn classify(key: usize, flags: isize) -> Modifier {
    let key = if key == 0xe5 {
        let scan = ((flags as usize >> 16) & 0xff) as u32;
        if scan == 0 { return Modifier::Other; }
        unsafe { MapVirtualKeyW(scan | if flags as usize & (1 << 24) != 0 { 0xe000 } else { 0 }, MAPVK_VSC_TO_VK_EX) as usize }
    } else { key };
    match key { 0x10 | 0xa0 | 0xa1 => Modifier::Shift, 0x11 | 0xa2 | 0xa3 => Modifier::Control, _ => Modifier::Other }
}
impl Activity {
    fn record(&mut self, modifier: Modifier) {
        if modifier != Modifier::Shift { self.shift = self.shift.wrapping_add(1); }
        if modifier != Modifier::Control { self.control = self.control.wrapping_add(1); }
    }
}
thread_local! {
    static ACTIVITY: Cell<Activity> = const { Cell::new(Activity { shift: 0, control: 0 }) };
    static HOOK: Cell<(HHOOK, u32)> = const { Cell::new((HHOOK(std::ptr::null_mut()), 0)) };
}
pub fn epoch() -> u64 { ACTIVITY.with(|v| v.get().shift) }
pub fn control_epoch() -> u64 { ACTIVITY.with(|v| v.get().control) }
#[cfg(feature="input-diagnostics")]
pub(super) fn is_shift_message(key: WPARAM, flags: LPARAM) -> bool { classify(key.0, flags.0) == Modifier::Shift }
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
        if [WM_KEYDOWN,WM_KEYUP,WM_SYSKEYDOWN,WM_SYSKEYUP].contains(&key.message) {
            super::diagnostics::message(key.wParam,key.lParam,[WM_KEYDOWN,WM_SYSKEYDOWN].contains(&key.message),
                [WM_SYSKEYDOWN,WM_SYSKEYUP].contains(&key.message));
        }
        if [WM_KEYDOWN,WM_KEYUP,WM_SYSKEYDOWN,WM_SYSKEYUP].contains(&key.message) {
            ACTIVITY.with(|value| { let mut next=value.get(); next.record(classify(key.wParam.0,key.lParam.0)); value.set(next); });
        }
    }
    CallNextHookEx(None, code, removed, message)
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn proxy_modifiers_keep_chord_protection() {
        assert_eq!(classify(0xe5,0x2a << 16),Modifier::Shift);
        assert_eq!(classify(0xe5,0x36 << 16),Modifier::Shift);
        assert_eq!(classify(0xe5,0x1d << 16),Modifier::Control);
        assert_eq!(classify(0xe5,(0x1d << 16) | (1 << 24)),Modifier::Control);
        for flags in [0,0x1c << 16,0x1e << 16,0x4d << 16] { assert_eq!(classify(0xe5,flags),Modifier::Other); }
        let mut activity=Activity::default();
        for _ in 0..4 { activity.record(Modifier::Shift); }
        assert_eq!(activity.shift,0);
        activity.record(Modifier::Control);
        assert_eq!(activity.shift,1);
        assert_eq!(activity.control,4);
        activity.record(Modifier::Other);
        assert_eq!((activity.shift,activity.control),(2,5));
    }
}
