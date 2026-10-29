//! Per-session, normal-user selection coordinator. Raw Input observes modifiers only;
//! UI Automation lives in a killable child. No clipboard, document text, or key log.
use std::{cell::{RefCell,Cell},collections::VecDeque,io::{BufRead,BufReader,Write,Read},path::PathBuf,
    process::{Child,Command,Stdio},os::windows::process::CommandExt,sync::mpsc,time::{Duration,Instant}};
use windows::{core::*,Win32::{Foundation::*,System::{Com::*,LibraryLoader::GetModuleHandleW,Threading::*},
    UI::{Accessibility::*,Input::*,WindowsAndMessaging::*}}};
use crate::{selection_text as selected,word_gesture::Gesture,word_popup::Popup};
pub fn initialize_dpi()->Result<()> {unsafe{
    use windows::Win32::UI::HiDpi::*;
    if AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(),DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).as_bool(){return Ok(());}
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
}}
const CLASS:PCWSTR=w!("Shanjie.WordHelper.Coordinator.v1");
const START:u32=WM_APP+211;
const QUIT:u32=WM_APP+212;
thread_local!{static STATE:RefCell<Option<State>>=const{RefCell::new(None)};}
thread_local!{static ROUTE:Cell<Option<(HWND,HWND,u32)>>=const{Cell::new(None)};}
fn reading_key(vk:usize,ctrl:bool,alt:bool,win:bool)->bool{
    !alt&&!win&&(!ctrl||vk==0x41)
        &&(matches!(vk,8|13|27|32|46)||(0x30..=0x5a).contains(&vk)||(0xba..=0xc0).contains(&vk)||(0xdb..=0xdf).contains(&vk)||vk==0xe2)
}
unsafe fn legacy_panel(source:HWND)->bool{
    let mut source_pid=0;GetWindowThreadProcessId(source,Some(&mut source_pid));let mut after=None;
    for _ in 0..32{
        let Ok(window)=FindWindowExW(None,after,w!("Shanjie.AddWord.Popup.v1"),None) else{break;};
        if window.is_invalid(){break;}after=Some(window);
        let mut pid=0;GetWindowThreadProcessId(window,Some(&mut pid));
        if pid==source_pid&&pid!=std::process::id()&&IsWindowVisible(window).as_bool(){return true;}
    }
    false
}
unsafe extern "system" fn route_key(code:i32,message:WPARAM,data:LPARAM)->LRESULT{
    // This hook exists only while a panel is open. It never reads text, calls
    // UIA, edits a dictionary, or waits. All work is posted to the panel.
    if code>=0{
        if let Some((source,popup,pid))=ROUTE.with(Cell::get){
            let root=GetAncestor(GetForegroundWindow(),GA_ROOT);let mut current=0;GetWindowThreadProcessId(root,Some(&mut current));
            if root==source&&current==pid&&IsWindowVisible(popup).as_bool()&&!GetPropW(popup,w!("Shanjie.AddWord.Active.v1")).is_invalid(){
                use windows::Win32::UI::Input::KeyboardAndMouse::*;
                let key=&*(data.0 as *const KBDLLHOOKSTRUCT);
                let held=|vk:VIRTUAL_KEY|GetAsyncKeyState(vk.0 as i32) as u16&0x8000!=0;
                let ctrl=held(VK_CONTROL);let alt=held(VK_MENU);let win=held(VK_LWIN)||held(VK_RWIN);
                if reading_key(key.vkCode as usize,ctrl,alt,win){
                    if message.0==WM_KEYDOWN as usize||message.0==WM_SYSKEYDOWN as usize{
                        let mut translated=shanjie_core::ffi::ShanjieKey{kind:0,ch:0,modifiers:0};
                        let flags=(key.scanCode as isize)<<16;
                        let ch=if crate::bridge::sj_translate(key.vkCode as usize,flags,&mut translated){char::from_u32(translated.ch)}else{None};
                        let vk=if key.vkCode==46{8}else{key.vkCode as usize};
                        let _=PostMessageW(Some(popup),crate::word_popup::ROUTE_KEY,WPARAM(vk|((ctrl as usize)<<16)),LPARAM(ch.map_or(0,|c|c as isize+1)));
                    }
                    return LRESULT(1);
                }
            }
        }
    }
    CallNextHookEx(None,code,message,data)
}
pub fn running()->bool{unsafe{FindWindowW(CLASS,None).is_ok_and(|w|!w.is_invalid()&&!GetPropW(w,w!("Shanjie.WordHelper.Ready.v1")).is_invalid())}}
pub(crate) fn start_resident(data:&std::path::Path){
    if std::env::var_os("SHANJIE_TEST_PROFILE").is_some()||std::env::var_os("SHANJIE_NO_WORD_HELPER").is_some(){return;}
    let executable=data.parent().unwrap_or(data).join("tools/shanjie-word-helper.exe");
    if !executable.is_file(){return;}
    let _=std::thread::Builder::new().name("Shanjie word helper owner".into()).spawn(move||{
        loop{
            if !running(){
                let _=Command::new(&executable).args(["--parent",&std::process::id().to_string()])
                    .creation_flags(0x08000000).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    });
}
#[derive(serde::Serialize,serde::Deserialize)]
struct Message{ready:bool,word:String,rect:[i32;4]}
struct Worker{child:Child,receiver:mpsc::Receiver<Message>,deadline:Instant,delivered:bool}
impl Drop for Worker{fn drop(&mut self){let _=self.child.kill();let _=self.child.wait();}}
impl Worker{
    fn start(source:HWND,helper:HWND)->std::io::Result<Self>{
        let mut child=Command::new(std::env::current_exe()?).args(["--capture",&(source.0 as usize).to_string(),&(helper.0 as usize).to_string()])
            .creation_flags(0x08000000).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
        let pipe=child.stdout.take().unwrap();let (send,receiver)=mpsc::channel();
        std::thread::spawn(move||{
            for line in BufReader::new(pipe.take(262144)).lines(){
                let Ok(line)=line else{break;};if line.len()>8192{break;}
                let Ok(value)=serde_json::from_str(&line) else{break;};if send.send(value).is_err(){break;}
            }
        });
        Ok(Self{child,receiver,deadline:Instant::now()+Duration::from_secs(2),delivered:false})
    }
}
struct State{
    window:HWND,source:HWND,data:PathBuf,prefs:crate::profile::Document,refresh:Instant,
    gesture:Gesture,held:[bool;256],epoch:u64,worker:Option<Worker>,popup:Option<Box<Popup>>,parent:Option<HANDLE>,
    notice:Option<crate::selection_notice::Notice>,
    hook:Option<HHOOK>,
}
impl State{
    fn close(&mut self){
        ROUTE.with(|r|r.set(None));
        if let Some(h)=self.hook.take(){unsafe{let _=UnhookWindowsHookEx(h);}}
        self.worker.take();if let Some(p)=self.popup.take(){p.close();}self.notice.take();self.gesture.reset();
        unsafe{SetTimer(Some(self.window),1,1000,None);}
    }
    unsafe fn failed(&mut self){
        self.close();
        if GetAncestor(GetForegroundWindow(),GA_ROOT)==self.source{
            self.notice=crate::selection_notice::Notice::open_at(self.window,self.source,self.prefs.number("appearance","theme",0)).ok();
        }
    }
    unsafe fn input(&mut self,key:usize,down:bool)->bool{
        if key>=256{return false;}
        let source=GetAncestor(GetForegroundWindow(),GA_ROOT);
        let repeat=down&&self.held[key];self.held[key]=down;
        if self.popup.as_ref().is_some_and(|p|p.contains(source)){self.gesture.reset();return false;}
        if self.hook.is_some()&&source==self.source&&reading_key(key,self.held[0xa2]||self.held[0xa3],self.held[18]||self.held[0xa4]||self.held[0xa5],self.held[0x5b]||self.held[0x5c]){
            self.gesture.reset();return false;
        }
        if (self.worker.is_some()||self.popup.is_some())&&(source!=self.source||(![0xa2,0xa3].contains(&key)&&down)){self.close();}
        if source!=self.source {self.gesture.reset();self.source=source;}
        if key==0x7b&&down&&!repeat&&self.prefs.flag("shortcuts","add_word",true)&&self.prefs.number("shortcuts","add_word_shortcut",0)==1{
            return (self.held[0xa2]||self.held[0xa3])&&(self.held[0xa0]||self.held[0xa1]||self.held[16])
                &&!self.held[18]&&!self.held[0xa4]&&!self.held[0xa5]&&!self.held[0x5b]&&!self.held[0x5c];
        }
        if ![0xa2,0xa3].contains(&key){self.epoch=self.epoch.wrapping_add(1);self.gesture.reset();return false;}
        let p=&self.prefs;
        if !p.flag("shortcuts","add_word",true)||p.number("shortcuts","add_word_shortcut",0)!=0{return false;}
        let side=p.number("shortcuts","add_word_side",0);
        let pure=(side==0||key==if side==1{0xa2}else{0xa3})
            &&!self.held.iter().enumerate().any(|(i,&on)|on&&i!=key&&i!=17);
        self.gesture.observe(key,down,repeat,self.epoch,pure,p.number("shortcuts","add_word_interval",450).clamp(200,1000))
    }
    unsafe fn begin(&mut self){
        self.close();let source=GetAncestor(GetForegroundWindow(),GA_ROOT);
        if source.is_invalid()||source==self.window{return;}
        self.source=source;self.worker=Worker::start(source,self.window).ok();
        SetTimer(Some(self.window),1,80,None);
    }
    unsafe fn mouse(&mut self,buttons:u16){
        if buttons&0x0d55==0{return;}
        self.epoch=self.epoch.wrapping_add(1);self.gesture.reset();
        if self.worker.is_none()&&self.popup.is_none(){return;}
        let mut point=POINT::default();if GetCursorPos(&mut point).is_err(){self.close();return;}
        let target=WindowFromPoint(point);
        if !self.popup.as_ref().is_some_and(|p|p.contains(target)){self.close();}
    }
    unsafe fn poll(&mut self){
        if self.parent.is_some_and(|h|WaitForSingleObject(h,0)!=WAIT_TIMEOUT){let _=PostMessageW(Some(self.window),WM_CLOSE,WPARAM(0),LPARAM(0));return;}
        if self.refresh.elapsed()>Duration::from_secs(1){
            if let Ok(p)=crate::profile::load(&crate::profile::path()){self.prefs=p;}
            self.refresh=Instant::now();
            if !self.prefs.flag("shortcuts","add_word",true){self.close();}
            if let Some(p)=&self.popup{let _=p.set_theme(self.prefs.number("appearance","theme",0));}
        }
        if self.popup.as_ref().is_some_and(|p|!p.active()){self.close();return;}
        // During an upgrade a host may still hold the old TIP. Its existing
        // editable panel wins; never close or send messages to that host panel.
        if (self.worker.is_some()||self.popup.is_some())&&legacy_panel(self.source){self.close();return;}
        let Some(worker)=self.worker.as_mut() else{return;};
        let answer=worker.receiver.try_recv();
        if Instant::now()>worker.deadline{if worker.delivered{self.close();}else{self.failed();}return;}
        if answer.is_ok(){worker.deadline=Instant::now()+Duration::from_secs(2);}
        match answer{
            Ok(m) if m.ready&&!worker.delivered=>{
                worker.delivered=true;
                if GetAncestor(GetForegroundWindow(),GA_ROOT)!=self.source||!IsWindow(Some(self.source)).as_bool(){self.close();return;}
                if selected::checked_word(&m.word.encode_utf16().collect::<Vec<_>>()).is_err(){self.close();return;}
                let rect=RECT{left:m.rect[0],top:m.rect[1],right:m.rect[2],bottom:m.rect[3]};
                let keyboard=self.prefs.number("input","keyboard",0);
                let layout=if keyboard==1{shanjie_core::engine::Layout::Eten}else{shanjie_core::engine::Layout::Standard};
                match Popup::open_helper(self.window,self.source,rect,m.word,self.data.clone(),layout,self.prefs.number("appearance","theme",0)){
                    Ok(p)=>{
                        let mut pid=0;GetWindowThreadProcessId(self.source,Some(&mut pid));
                        match SetWindowsHookExW(WH_KEYBOARD_LL,Some(route_key),Some(HINSTANCE(GetModuleHandleW(None).unwrap().0)),0){
                            Ok(h)=>{ROUTE.with(|r|r.set(Some((self.source,p.window(),pid))));self.hook=Some(h);self.popup=Some(p);},
                            Err(_)=>{p.close();self.failed();}
                        }
                    },Err(_)=>self.close()
                }
            },
            Ok(m) if !m.ready=>self.close(),
            Ok(m) if m.ready=>{if let Some(p)=&self.popup{p.anchor(RECT{left:m.rect[0],top:m.rect[1],right:m.rect[2],bottom:m.rect[3]});}},
            Err(mpsc::TryRecvError::Disconnected)=>if worker.delivered{self.close()}else{self.failed()},
            _=>{}
        }
    }
}
unsafe extern "system" fn procedure(window:HWND,message:u32,wp:WPARAM,lp:LPARAM)->LRESULT{
    match message{
        WM_INPUT=>{
            let mut size=0;let header=std::mem::size_of::<RAWINPUTHEADER>() as u32;
            if GetRawInputData(HRAWINPUT(lp.0 as _),RID_INPUT,None,&mut size,header)==u32::MAX{return LRESULT(0);}
            if size as usize>4096{return LRESULT(0);}
            let mut buffer=vec![0usize;(size as usize).div_ceil(std::mem::size_of::<usize>())];
            if GetRawInputData(HRAWINPUT(lp.0 as _),RID_INPUT,Some(buffer.as_mut_ptr().cast()),&mut size,header)!=u32::MAX{
                let raw=&*(buffer.as_ptr() as *const RAWINPUT);
                if raw.header.dwType==RIM_TYPEKEYBOARD.0{
                    let k=raw.data.keyboard;let key=if k.VKey==17{if k.Flags&RI_KEY_E0 as u16!=0{0xa3}else{0xa2}}else{k.VKey as usize};
                    let fire=STATE.with(|s|s.borrow_mut().as_mut().is_some_and(|s|s.input(key,k.Flags&RI_KEY_BREAK as u16==0)));
                    if fire{let _=PostMessageW(Some(window),START,WPARAM(0),LPARAM(0));}
                }else if raw.header.dwType==RIM_TYPEMOUSE.0{
                    let buttons=raw.data.mouse.Anonymous.Anonymous.usButtonFlags;
                    STATE.with(|s|{if let Some(s)=s.borrow_mut().as_mut(){s.mouse(buttons);}});
                }
            }
        },
        START=>STATE.with(|s|{if let Some(s)=s.borrow_mut().as_mut(){s.begin();}}),
        WM_TIMER=>STATE.with(|s|{if let Some(s)=s.borrow_mut().as_mut(){s.poll();}}),
        WM_CLOSE=>{
            STATE.with(|s|{if let Some(s)=s.borrow_mut().as_mut(){s.close();}});
            let _=PostMessageW(Some(window),QUIT,WPARAM(0),LPARAM(0));return LRESULT(0);
        },
        QUIT=>{PostQuitMessage(0);return LRESULT(0);},
        _=>{}
    }
    DefWindowProcW(window,message,wp,lp)
}
pub fn serve(parent:u32)->Result<()> {unsafe{
    let name=crate::wide(&format!("Local\\Shanjie.WordHelper.{}",crate::broker::user_sid().map_err(|_|Error::from_hresult(E_ACCESSDENIED))?));
    let mutex=CreateMutexW(None,false,PCWSTR(name.as_ptr()))?;
    struct Singleton(HANDLE);impl Drop for Singleton{fn drop(&mut self){unsafe{let _=ReleaseMutex(self.0);let _=CloseHandle(self.0);}}}
    let singleton=Singleton(mutex);
    if ![WAIT_OBJECT_0,WAIT_ABANDONED].contains(&WaitForSingleObject(mutex,0)){return Ok(());}
    if running(){return Ok(());}
    let module=HINSTANCE(GetModuleHandleW(None)?.0);
    RegisterClassW(&WNDCLASSW{lpfnWndProc:Some(procedure),hInstance:module,lpszClassName:CLASS,..Default::default()});
    let window=CreateWindowExW(WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE,CLASS,w!(""),WS_POPUP,-12000,-12000,1,1,None,None,Some(module),None)?;
    let devices=[RAWINPUTDEVICE{usUsagePage:1,usUsage:6,dwFlags:RIDEV_INPUTSINK,hwndTarget:window},
        RAWINPUTDEVICE{usUsagePage:1,usUsage:2,dwFlags:RIDEV_INPUTSINK,hwndTarget:window}];
    RegisterRawInputDevices(&devices,std::mem::size_of::<RAWINPUTDEVICE>() as u32)?;
    let data=std::env::current_exe().map_err(|_|Error::from_hresult(E_FAIL))?.parent().unwrap().parent().unwrap().join("data");
    let handle=if parent==0{None}else{Some(OpenProcess(PROCESS_SYNCHRONIZE,false,parent)?)};
    STATE.with(|s|*s.borrow_mut()=Some(State{window,source:HWND::default(),data,prefs:crate::profile::load(&crate::profile::path()).unwrap(),
        refresh:Instant::now(),gesture:Gesture::default(),held:[false;256],epoch:0,worker:None,popup:None,parent:handle,notice:None,hook:None}));
    SetPropW(window,w!("Shanjie.WordHelper.Ready.v1"),Some(HANDLE(1usize as _)))?;
    SetTimer(Some(window),1,1000,None);
    let mut message=MSG::default();while GetMessageW(&mut message,None,0,0).as_bool(){let _=TranslateMessage(&message);DispatchMessageW(&message);}
    STATE.with(|s|{if let Some(mut s)=s.borrow_mut().take(){s.close();if let Some(h)=s.parent{let _=CloseHandle(h);}}});
    let _=RemovePropW(window,w!("Shanjie.WordHelper.Ready.v1"));let _=DestroyWindow(window);drop(singleton);Ok(())
}}
unsafe fn belongs(automation:&IUIAutomation,walker:&IUIAutomationTreeWalker,element:&IUIAutomationElement,root:&IUIAutomationElement)->Result<bool>{
    let mut current=element.clone();
    for _ in 0..32{
        if automation.CompareElements(&current,root)?.as_bool(){return Ok(true);}
        current=match walker.GetParentElement(&current){Ok(p)=>p,Err(_)=>return Ok(false)};
    }
    Ok(false)
}
pub fn capture(source:usize,helper:usize)->Result<()> {unsafe{
    let source=HWND(source as _);let helper=HWND(helper as _);
    if source.is_invalid()||GetAncestor(GetForegroundWindow(),GA_ROOT)!=source{return Err(E_ABORT.into());}
    CoInitializeEx(None,COINIT_MULTITHREADED).ok()?;
    struct Apartment;impl Drop for Apartment{fn drop(&mut self){unsafe{CoUninitialize()}}}let _apartment=Apartment;
    let automation:IUIAutomation=CoCreateInstance(&CUIAutomation8,None,CLSCTX_INPROC_SERVER)?;
    let settings:IUIAutomation2=automation.cast()?;settings.SetConnectionTimeout(300)?;settings.SetTransactionTimeout(500)?;
    let root=automation.ElementFromHandle(source)?;let walker=automation.ControlViewWalker()?;
    let focused=automation.GetFocusedElement()?;
    if focused.CurrentIsPassword()?.as_bool(){return Err(E_ACCESSDENIED.into());}
    let started=Instant::now();let mut queue=VecDeque::new();
    if belongs(&automation,&walker,&focused,&root)?{queue.push_back((focused,0usize));}
    queue.push_back((root.clone(),0));
    let mut count=0;let mut found=None;
    while let Some((element,depth))=queue.pop_front(){
        count+=1;if count>256||started.elapsed()>Duration::from_millis(1200){break;}
        if !element.CurrentIsOffscreen()?.as_bool()&&!element.CurrentIsPassword()?.as_bool(){
            if let Ok(value)=selected::inspect_readonly(&element){found=Some((element,value));break;}
            if depth<8{
                if let Ok(mut child)=walker.GetFirstChildElement(&element){for _ in 0..64{
                    queue.push_back((child.clone(),depth+1));child=match walker.GetNextSiblingElement(&child){Ok(c)=>c,Err(_)=>break};
                }}
            }
        }
    }
    let Some((element,(pattern,range,word)))=found else{return Err(E_FAIL.into());};
    if !belongs(&automation,&walker,&element,&root)?{return Err(E_ACCESSDENIED.into());}
    let rect=selected::rectangle(&range)?;
    let unchanged=||->Result<bool>{
        if !IsWindow(Some(source)).as_bool(){return Ok(false);}
        let foreground=GetForegroundWindow();
        if GetAncestor(foreground,GA_ROOT)!=source&&GetAncestor(foreground,GA_ROOTOWNER)!=helper{return Ok(false);}
        let current=selected::selected(&pattern)?;
        Ok(!element.CurrentIsPassword()?.as_bool()&&range.Compare(&current)?.as_bool()
            &&selected::checked_word(&current.GetText(65)?)?==word)
    };
    if !unchanged()?{return Err(E_ABORT.into());}
    println!("{}",serde_json::to_string(&Message{ready:true,word:word.clone(),rect:[rect.left,rect.top,rect.right,rect.bottom]}).unwrap());
    let _=std::io::stdout().flush();
    while IsWindow(Some(helper)).as_bool(){
        std::thread::sleep(Duration::from_millis(160));if !unchanged()?{break;}
        let rect=selected::rectangle(&range)?;
        println!("{}",serde_json::to_string(&Message{ready:true,word:String::new(),rect:[rect.left,rect.top,rect.right,rect.bottom]}).unwrap());
        let _=std::io::stdout().flush();
    }
    println!("{}",serde_json::to_string(&Message{ready:false,word:String::new(),rect:[0;4]}).unwrap());Ok(())
}}
/// Explicitly owned, offscreen fixture only; never enumerates the user's desktop.
pub fn capture_fixture(source:usize,owner:u32)->Result<()> {unsafe{
    if std::env::var("SHANJIE_TEST_PROFILE").as_deref()!=Ok("1"){return Err(E_ACCESSDENIED.into());}
    let source=HWND(source as _);let mut pid=0;GetWindowThreadProcessId(source,Some(&mut pid));
    let mut name=[0u16;80];let root=GetAncestor(source,GA_ROOT);let count=GetClassNameW(root,&mut name);
    if owner==0||pid!=owner||pid==std::process::id()||String::from_utf16_lossy(&name[..count as usize])!="Shanjie.WordHelper.OwnedFixture.v1"{return Err(E_ACCESSDENIED.into());}
    CoInitializeEx(None,COINIT_MULTITHREADED).ok()?;
    let result=(||->Result<()>{
        let automation:IUIAutomation=CoCreateInstance(&CUIAutomation8,None,CLSCTX_INPROC_SERVER)?;
        let element=automation.ElementFromHandle(source)?;
        let (_,range,word)=selected::inspect_readonly(&element)?;
        let rect=selected::rectangle(&range).unwrap_or_default();
        println!("{}",serde_json::to_string(&Message{ready:true,word,rect:[rect.left,rect.top,rect.right,rect.bottom]}).unwrap());Ok(())
    })();CoUninitialize();result
}}
/// Owned, offscreen cross-process display check; no desktop enumeration or input.
pub fn display_fixture(source:usize,owner:u32)->Result<()> {unsafe{
    if std::env::var("SHANJIE_TEST_PROFILE").as_deref()!=Ok("1")||std::env::var_os("SHANJIE_HEADLESS").is_none(){return Err(E_ACCESSDENIED.into());}
    let source=HWND(source as _);let mut pid=0;GetWindowThreadProcessId(source,Some(&mut pid));
    let mut name=[0u16;80];let length=GetClassNameW(source,&mut name);
    if owner==0||pid!=owner||pid==std::process::id()||String::from_utf16_lossy(&name[..length as usize])!="Shanjie.WordHelper.OwnedFixture.v1"{return Err(E_ACCESSDENIED.into());}
    let helper=CreateWindowExW(WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE,w!("STATIC"),w!(""),WS_POPUP,-12000,-12000,1,1,None,None,None,None)?;
    for mouse in [true,false]{
        let foreground=GetForegroundWindow();
        let p=Popup::open_helper(helper,source,RECT{left:0,top:0,right:100,bottom:30},"你好".into(),PathBuf::from("missing-fixture"),shanjie_core::engine::Layout::Standard,0)?;
        let w=p.window();let mut rect=RECT::default();GetWindowRect(w,&mut rect)?;
        let dpi=windows::Win32::UI::HiDpi::GetDpiForWindow(w) as i32;
        assert_eq!((rect.right-rect.left,rect.bottom-rect.top),(440*dpi/96,294*dpi/96));
        assert_eq!(GetWindow(w,GW_OWNER)?,helper);assert_eq!(GetForegroundWindow(),foreground);
        assert!(GetWindowLongPtrW(w,GWL_EXSTYLE) as isize&WS_EX_TOPMOST.0 as isize!=0);
        let mut cursor=GetTopWindow(None)?;let mut saw_popup=false;
        while !cursor.is_invalid(){if cursor==w{saw_popup=true;}if cursor==source{assert!(saw_popup,"popup above source in actual z-order");break;}
            cursor=GetWindow(cursor,GW_HWNDNEXT).unwrap_or_default();}
        if mouse{let b=GetDlgItem(Some(w),11)?;SendMessageW(w,WM_COMMAND,Some(WPARAM(11)),Some(LPARAM(b.0 as isize)));}else{p.key(27,None,false);}
        let deadline=Instant::now()+Duration::from_secs(2);
        while IsWindow(Some(w)).as_bool()&&Instant::now()<deadline{
            let mut m=MSG::default();while PeekMessageW(&mut m,None,0,0,PM_REMOVE).as_bool(){DispatchMessageW(&m);}std::thread::sleep(Duration::from_millis(2));}
        assert!(!IsWindow(Some(w)).as_bool());assert!(IsWindow(Some(source)).as_bool());
        println!("PASS cross-process popup: dpi={dpi} width={} height={} above source, no activation, safe cancel",rect.right-rect.left,rect.bottom-rect.top);
    }
    let _=DestroyWindow(helper);Ok(())
}}
#[cfg(test)]mod tests{
    #[test]fn panel_keys_do_not_capture_shortcuts_or_modifiers(){
        for vk in [8,13,27,32,46,0x41,0x33,0xbf]{assert!(super::reading_key(vk,false,false,false));}
        assert!(super::reading_key(0x41,true,false,false));
        for vk in [0x43,0x56,0x58,0x7b]{assert!(!super::reading_key(vk,true,false,false));}
        for vk in [16,17,18,0x5b,0x5c,0xa0,0xa1,0xa2,0xa3,0xa4,0xa5,0x70,0x25]{assert!(!super::reading_key(vk,false,false,false));}
        assert!(!super::reading_key(0x41,false,true,false));assert!(!super::reading_key(13,false,false,true));
    }
    unsafe extern "system" fn fixture_proc(w:HWND,m:u32,a:WPARAM,b:LPARAM)->LRESULT{DefWindowProcW(w,m,a,b)}
    use super::*;
    use windows::Win32::UI::Controls::EM_SETSEL;
    #[test]#[ignore="Run with explicitly built helper on owned offscreen fixtures"]
    fn cross_process_popup_dpi_zorder_and_cancellation(){unsafe{
        use windows::Win32::UI::HiDpi::*;
        let exe=std::env::var_os("SHANJIE_WORD_HELPER_TEST_EXE").unwrap();
        let original=GetThreadDpiAwarenessContext();
        let class=w!("Shanjie.WordHelper.OwnedFixture.v1");let module=HINSTANCE(GetModuleHandleW(None).unwrap().0);
        RegisterClassW(&WNDCLASSW{lpfnWndProc:Some(fixture_proc),hInstance:module,lpszClassName:class,..Default::default()});
        for context in [DPI_AWARENESS_CONTEXT_UNAWARE,DPI_AWARENESS_CONTEXT_SYSTEM_AWARE,DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2]{
            SetThreadDpiAwarenessContext(context);
            let root=CreateWindowExW(WS_EX_NOACTIVATE|WS_EX_TOOLWINDOW,class,w!(""),WS_POPUP|WS_VISIBLE,-12000,-12000,300,100,None,None,Some(module),None).unwrap();
            let mut child=Command::new(&exe).args(["--fixture-display",&(root.0 as usize).to_string(),&std::process::id().to_string()])
                .creation_flags(0x08000000).env("SHANJIE_TEST_PROFILE","1").env("SHANJIE_HEADLESS","1").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
            let deadline=Instant::now()+Duration::from_secs(10);
            while child.try_wait().unwrap().is_none(){
                if Instant::now()>deadline{let _=child.kill();panic!("owned display fixture timed out");}
                let mut m=MSG::default();while PeekMessageW(&mut m,None,0,0,PM_REMOVE).as_bool(){DispatchMessageW(&m);}std::thread::sleep(Duration::from_millis(5));}
            let output=child.wait_with_output().unwrap();assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
            println!("{}",String::from_utf8_lossy(&output.stdout));let _=DestroyWindow(root);
        }
        SetThreadDpiAwarenessContext(original);
    }}
    #[test]#[ignore="Run with explicitly built helper on owned offscreen fixtures"]
    fn cross_process_readonly_selection_password_empty_and_limit(){unsafe{
        let exe=std::env::var_os("SHANJIE_WORD_HELPER_TEST_EXE").expect("explicit fixture helper");
        let class=w!("Shanjie.WordHelper.OwnedFixture.v1");let module=HINSTANCE(GetModuleHandleW(None).unwrap().0);
        RegisterClassW(&WNDCLASSW{lpfnWndProc:Some(fixture_proc),hInstance:module,lpszClassName:class,..Default::default()});
        let root=CreateWindowExW(WS_EX_NOACTIVATE|WS_EX_TOOLWINDOW,class,w!(""),WS_POPUP|WS_VISIBLE,-12000,-12000,300,100,None,None,Some(module),None).unwrap();
        for (style,text,start,end,accepted) in [
            (ES_READONLY as u32,"前回家後".to_owned(),1,3,true),(0,"前回家後".to_owned(),1,3,true),
            (ES_READONLY as u32,"前回家後".to_owned(),1,1,false),
            (ES_PASSWORD as u32,"前回家後".to_owned(),1,3,false),(ES_READONLY as u32,"漢".repeat(33),0,33,false)]{
            let edit=CreateWindowExW(WINDOW_EX_STYLE(0),w!("EDIT"),PCWSTR(crate::wide(&text).as_ptr()),WS_CHILD|WS_VISIBLE|WINDOW_STYLE(style),0,0,290,90,Some(root),None,Some(module),None).unwrap();
            SendMessageW(edit,EM_SETSEL,Some(WPARAM(start)),Some(LPARAM(end)));
            let mut child=Command::new(&exe).args(["--fixture-capture",&(edit.0 as usize).to_string(),&std::process::id().to_string()])
                .creation_flags(0x08000000).env("SHANJIE_TEST_PROFILE","1").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
            let deadline=Instant::now()+Duration::from_secs(8);
            loop{
                if child.try_wait().unwrap().is_some(){break;}
                if Instant::now()>deadline{let _=child.kill();panic!("owned UIA worker timed out");}
                let mut message=MSG::default();while PeekMessageW(&mut message,None,0,0,PM_REMOVE).as_bool(){let _=TranslateMessage(&message);DispatchMessageW(&message);}
                std::thread::sleep(Duration::from_millis(5));
            }
            let output=child.wait_with_output().unwrap();
            assert_eq!(output.status.success(),accepted,"{}",String::from_utf8_lossy(&output.stderr));
            if accepted{let m:Message=serde_json::from_slice(&output.stdout).unwrap();assert_eq!(m.word,"回家");}
            let mut after=vec![0u16;text.encode_utf16().count()+1];let n=GetWindowTextW(edit,&mut after);
            assert_eq!(String::from_utf16_lossy(&after[..n as usize]),text,"source never changes");
            let _=DestroyWindow(edit);
        }
        let _=DestroyWindow(root);
    }}
    #[test]#[ignore="Run with explicitly built helper; terminates only its own child"]
    fn hung_worker_retirement_does_not_stop_other_processes(){
        let exe=std::env::var_os("SHANJIE_WORD_HELPER_TEST_EXE").unwrap();
        let spawn=||Command::new(&exe).arg("--fixture-hang").creation_flags(0x08000000).env("SHANJIE_TEST_PROFILE","1").spawn().unwrap();
        let mut other=spawn();let child=spawn();let (_send,receiver)=mpsc::channel();
        let start=Instant::now();drop(Worker{child,receiver,deadline:Instant::now(),delivered:false});
        assert!(start.elapsed()<Duration::from_secs(3));assert!(other.try_wait().unwrap().is_none());
        other.kill().unwrap();other.wait().unwrap();
    }
}
