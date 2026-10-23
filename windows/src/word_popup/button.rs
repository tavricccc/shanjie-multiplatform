//! Preserve the editor's TSF focus while retaining native owner-drawn buttons.
use windows::{core::Result,Win32::{Foundation::*,UI::{WindowsAndMessaging::*,
    Input::KeyboardAndMouse::*,Shell::{SetWindowSubclass,RemoveWindowSubclass,DefSubclassProc}}}};
const SUBCLASS:usize=1;
pub(super) unsafe fn install(window:HWND)->Result<()> {
    SetWindowSubclass(window,Some(procedure),SUBCLASS,0).ok()
}
unsafe fn inside(window:HWND,position:LPARAM)->bool {
    let point=POINT{x:position.0 as u16 as i16 as i32,y:(position.0>>16) as u16 as i16 as i32};
    let mut rect=RECT::default();
    GetClientRect(window,&mut rect).is_ok() && point.x>=0 && point.y>=0 && point.x<rect.right && point.y<rect.bottom
}
unsafe fn pressed(window:HWND,value:bool) {
    SendMessageW(window,BM_SETSTATE,Some(WPARAM(value as usize)),Some(LPARAM(0)));
}
unsafe fn click(window:HWND) {
    if !IsWindowEnabled(window).as_bool() {return;}
    if let Ok(parent)=GetParent(window) {
        let _=PostMessageW(Some(parent),WM_COMMAND,WPARAM(GetDlgCtrlID(window) as usize),LPARAM(window.0 as isize));
    }
}
unsafe extern "system" fn procedure(window:HWND,message:u32,wp:WPARAM,lp:LPARAM,_:usize,_:usize)->LRESULT {
    match message {
        WM_MOUSEACTIVATE=>return LRESULT(MA_NOACTIVATE as isize),
        WM_LBUTTONDOWN|WM_LBUTTONDBLCLK=>{
            if IsWindowEnabled(window).as_bool(){SetCapture(window);pressed(window,true);}
            return LRESULT(0);
        },
        WM_MOUSEMOVE if GetCapture()==window=>{pressed(window,inside(window,lp));return LRESULT(0);},
        WM_LBUTTONUP=>{
            if GetCapture()==window {let activate=inside(window,lp);let _=ReleaseCapture();pressed(window,false);if activate{click(window);}}
            return LRESULT(0);
        },
        WM_CANCELMODE|WM_CAPTURECHANGED=>{pressed(window,false);},
        BM_CLICK=>{click(window);return LRESULT(0);},
        WM_NCDESTROY=>{let _=RemoveWindowSubclass(window,Some(procedure),SUBCLASS);},
        _=>{}
    }
    DefSubclassProc(window,message,wp,lp)
}
