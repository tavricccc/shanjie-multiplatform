//! Selected-text fallback for TSF bridges. All UIA objects stay on one MTA
//! worker; no clipboard, document range or unbounded text read is used.
use std::{sync::{Arc,atomic::{AtomicBool,Ordering},mpsc},time::{Duration,Instant}};
use windows::{core::*,Win32::{Foundation::*,System::{Com::*,Ole::*,Variant::VT_R8},
    UI::{Accessibility::*,WindowsAndMessaging::*}}};
static BUSY:AtomicBool=AtomicBool::new(false);
pub(crate) enum Event { Ready(String,RECT), Invalid }
pub(crate) struct Lookup {
    pub receiver:mpsc::Receiver<Event>,
    pub deadline:Instant,
    pub delivered:bool,
    cancel:Arc<AtomicBool>,
}
impl Drop for Lookup { fn drop(&mut self) { self.cancel.store(true,Ordering::Release); } }
struct Busy;
impl Drop for Busy { fn drop(&mut self) { BUSY.store(false,Ordering::Release); } }
struct Apartment;
impl Drop for Apartment { fn drop(&mut self) { unsafe { CoUninitialize(); } } }
struct Array(*mut SAFEARRAY);
impl Drop for Array { fn drop(&mut self) { unsafe { let _=SafeArrayDestroy(self.0); } } }
pub(crate) fn checked_word(text:&[u16])->Result<String> {
    if text.len()>=65 { return Err(E_INVALIDARG.into()); }
    let word=String::from_utf16(text).map_err(|_|Error::from_hresult(E_INVALIDARG))?;
    crate::word_store::validate_word(&word).map_err(|_|Error::from_hresult(E_INVALIDARG))?;
    Ok(word)
}
pub(crate) unsafe fn rectangle(selected:&IUIAutomationTextRange)->Result<RECT> {
    let array=Array(selected.GetBoundingRectangles()?);
    if array.0.is_null() || SafeArrayGetDim(array.0)!=1 || SafeArrayGetVartype(array.0)?!=VT_R8 { return Err(E_FAIL.into()); }
    let size=(SafeArrayGetUBound(array.0,1)?-SafeArrayGetLBound(array.0,1)?+1) as usize;
    if size<4 || size>512 || size%4!=0 { return Err(E_FAIL.into()); }
    let mut ptr=std::ptr::null_mut(); SafeArrayAccessData(array.0,&mut ptr)?;
    let values=std::slice::from_raw_parts(ptr as *const f64,size);
    let found=values.chunks_exact(4).find(|v|v.iter().all(|x|x.is_finite() && x.abs()<1_000_000.0) && v[2]>0.0 && v[3]>0.0)
        .map(|v|RECT{left:v[0].floor() as i32,top:v[1].floor() as i32,right:(v[0]+v[2]).ceil() as i32,bottom:(v[1]+v[3]).ceil() as i32});
    SafeArrayUnaccessData(array.0)?;
    found.ok_or(E_FAIL.into())
}
pub(crate) unsafe fn selected(pattern:&IUIAutomationTextPattern)->Result<IUIAutomationTextRange> {
    let ranges=pattern.GetSelection()?;
    if ranges.Length()?!=1 { return Err(E_FAIL.into()); }
    ranges.GetElement(0)
}
unsafe fn inspect(element:&IUIAutomationElement)->Result<(IUIAutomationTextPattern,IUIAutomationTextRange,String)> {
    let (pattern,range,word)=inspect_readonly(element)?;
    if bool::try_from(&range.GetAttributeValue(UIA_IsReadOnlyAttributeId)?)? {return Err(E_ACCESSDENIED.into());}
    Ok((pattern,range,word))
}
pub(crate) unsafe fn inspect_readonly(element:&IUIAutomationElement)->Result<(IUIAutomationTextPattern,IUIAutomationTextRange,String)> {
    if element.CurrentIsPassword()?.as_bool() {return Err(E_ACCESSDENIED.into());}
    let pattern:IUIAutomationTextPattern=element.GetCurrentPatternAs(UIA_TextPatternId)?;
    let range=selected(&pattern)?;
    let word=checked_word(&range.GetText(65)?)?;
    Ok((pattern,range,word))
}
unsafe fn same_selection(pattern:&IUIAutomationTextPattern,range:&IUIAutomationTextRange,word:&str)->Result<bool> {
    let current=selected(pattern)?;
    Ok(range.Compare(&current)?.as_bool() && checked_word(&current.GetText(65)?)?==word
        && !bool::try_from(&current.GetAttributeValue(UIA_IsReadOnlyAttributeId)?)?)
}
unsafe fn run(owner:usize,cancel:&AtomicBool,sender:&mpsc::Sender<Event>)->Result<()> {
    CoInitializeEx(None,COINIT_MULTITHREADED).ok()?; let _apartment=Apartment;
    let automation:IUIAutomation=CoCreateInstance(&CUIAutomation8,None,CLSCTX_INPROC_SERVER)?;
    let settings:IUIAutomation2=automation.cast()?;
    settings.SetConnectionTimeout(300)?; settings.SetTransactionTimeout(500)?;
    let element=automation.GetFocusedElement()?;
    if element.CurrentProcessId()? as u32!=std::process::id() || element.CurrentIsPassword()?.as_bool() { return Err(E_ACCESSDENIED.into()); }
    let native=element.CurrentNativeWindowHandle()?;
    if !native.is_invalid() && GetAncestor(native,GA_ROOT)!=GetAncestor(HWND(owner as _),GA_ROOT) { return Err(E_ACCESSDENIED.into()); }
    let (pattern,range,word)=inspect(&element)?;
    let rect=rectangle(&range)?;
    let unchanged=||->Result<bool> {
        let focused=automation.GetFocusedElement()?;
        if !automation.CompareElements(&element,&focused)?.as_bool() || focused.CurrentIsPassword()?.as_bool() { return Ok(false); }
        same_selection(&pattern,&range,&word)
    };
    if cancel.load(Ordering::Acquire) || !unchanged()? { return Err(E_FAIL.into()); }
    sender.send(Event::Ready(word.clone(),rect)).map_err(|_|Error::from_hresult(E_ABORT))?;
    while !cancel.load(Ordering::Acquire) {
        std::thread::sleep(Duration::from_millis(120));
        if cancel.load(Ordering::Acquire) { break; }
        if !unchanged()? { return Err(E_ABORT.into()); }
    }
    Ok(())
}
pub(crate) fn start(owner:HWND)->Option<Lookup> {
    if BUSY.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_err() { return None; }
    let (sender,receiver)=mpsc::channel(); let cancel=Arc::new(AtomicBool::new(false));
    let stop=cancel.clone(); let owner=owner.0 as usize;
    // Hold the DLL for the entire worker lifetime, including a slow provider.
    let count=crate::tsf::Count::new();
    if std::thread::Builder::new().name("Shanjie selected text".into()).spawn(move|| {
        let _count=count; let _busy=Busy;
        if unsafe {run(owner,&stop,&sender)}.is_err() && !stop.load(Ordering::Acquire) { let _=sender.send(Event::Invalid); }
    }).is_err() { BUSY.store(false,Ordering::Release); return None; }
    Some(Lookup{receiver,deadline:Instant::now()+Duration::from_millis(1500),delivered:false,cancel})
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn only_short_selected_han_is_accepted() {
        for value in ["頭髮","與","漢" ] { assert!(checked_word(&value.encode_utf16().collect::<Vec<_>>()).is_ok()); }
        for value in ["","private text","漢\n字","123","漢 字"] { assert!(checked_word(&value.encode_utf16().collect::<Vec<_>>()).is_err()); }
        assert!(checked_word(&"漢".repeat(33).encode_utf16().collect::<Vec<_>>()).is_err());
        assert!(checked_word(&[0xd800]).is_err());
    }
    #[test] fn real_native_edit_selection_and_protection_on_mta() { unsafe {
        // This fixture is never focused/activated. UIA is addressed ONLY to its
        // owned HWND; no desktop selection or current user's input is inspected.
        use windows::Win32::{System::LibraryLoader::GetModuleHandleW,UI::Controls::EM_SETSEL};
        CoInitializeEx(None,COINIT_APARTMENTTHREADED).ok().unwrap(); let _apartment=Apartment;
        let module=GetModuleHandleW(None).unwrap();
        for (label,style,start,end,expected) in [
            ("selected",0,1,3,Some("回家")),("empty",0,1,1,None),
            ("readonly",ES_READONLY as u32,1,3,None),("password",ES_PASSWORD as u32,1,3,None),
        ] {
            let hwnd=CreateWindowExW(WS_EX_NOACTIVATE|WS_EX_TOOLWINDOW,w!("EDIT"),w!("前回家後"),
                WS_POPUP|WS_VISIBLE|WINDOW_STYLE(style),-12000,-12000,300,50,None,None,Some(HINSTANCE(module.0)),None).unwrap();
            SendMessageW(hwnd,EM_SETSEL,Some(WPARAM(start)),Some(LPARAM(end)));
            let address=hwnd.0 as usize; let (send,recv)=mpsc::channel();
            let worker=std::thread::spawn(move|| {
                let result=(||->Result<String> {
                    CoInitializeEx(None,COINIT_MULTITHREADED).ok()?;let _apartment=Apartment;
                    let automation:IUIAutomation=CoCreateInstance(&CUIAutomation8,None,CLSCTX_INPROC_SERVER)?;
                    let settings:IUIAutomation2=automation.cast()?;
                    settings.SetConnectionTimeout(300)?;settings.SetTransactionTimeout(500)?;
                    let element=automation.ElementFromHandle(HWND(address as _))?;
                    let (pattern,range,word)=inspect(&element)?;
                    assert!(same_selection(&pattern,&range,&word)?);
                    SendMessageW(HWND(address as _),EM_SETSEL,Some(WPARAM(0)),Some(LPARAM(1)));
                    assert!(!same_selection(&pattern,&range,&word)?);
                    Ok(word)
                })();
                let _=send.send(result.map_err(|e|e.code().0));
            });
            let until=Instant::now()+Duration::from_secs(8);
            let result=loop {
                match recv.try_recv() {
                    Ok(value)=>break value,
                    Err(mpsc::TryRecvError::Empty)=>{},
                    Err(e)=>panic!("{label}: {e}"),
                }
                assert!(Instant::now()<until,"{label}: owned UIA fixture timed out");
                let mut message=MSG::default();
                while PeekMessageW(&mut message,None,0,0,PM_REMOVE).as_bool() {let _=TranslateMessage(&message);DispatchMessageW(&message);}
                std::thread::sleep(Duration::from_millis(5));
            };
            worker.join().unwrap();let _=DestroyWindow(hwnd);
            if let Some(expected)=expected {assert_eq!(result.as_deref(),Ok(expected),"{label}");}
            else {assert!(result.is_err(),"{label}: protected or empty selection accepted");}
        }
    } }
}
