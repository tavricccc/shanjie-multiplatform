//! Per-session, normal-user selection coordinator. Raw Input observes modifiers only;
//! UI Automation lives in a killable child. No clipboard, document text, or key log.
use std::{cell::RefCell,collections::VecDeque,io::{BufRead,BufReader,Write,Read},path::PathBuf,
    process::{Child,Command,Stdio},os::windows::process::CommandExt,sync::mpsc,time::{Duration,Instant}};
use windows::{core::*,Win32::{Foundation::*,System::{Com::*,LibraryLoader::GetModuleHandleW,Threading::*},
    UI::{Accessibility::*,Input::*,WindowsAndMessaging::*}}};
use crate::{selection_text as selected,word_gesture::Gesture,word_popup::Popup};
const CLASS:PCWSTR=w!("Shanjie.WordHelper.Coordinator.v1");
const START:u32=WM_APP+211;
thread_local!{static STATE:RefCell<Option<State>>=const{RefCell::new(None)};}
pub fn running()->bool{unsafe{FindWindowW(CLASS,None).is_ok_and(|w|!w.is_invalid())}}
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
}
impl State{
    fn close(&mut self){self.worker.take();if let Some(p)=self.popup.take(){p.close();}self.notice.take();self.gesture.reset();}
    unsafe fn failed(&mut self){
        self.close();
        if GetAncestor(GetForegroundWindow(),GA_ROOT)==self.source{
            self.notice=crate::selection_notice::Notice::open_at(self.window,self.source,self.prefs.number("appearance","theme",0)).ok();
        }
    }
    unsafe fn input(&mut self,key:usize,down:bool)->bool{
        if key>=256{return false;}
        let source=GetAncestor(GetForegroundWindow(),GA_ROOT);
        if source!=self.source {self.gesture.reset();self.source=source;}
        let repeat=down&&self.held[key];self.held[key]=down;
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
    }
    unsafe fn poll(&mut self){
        if self.parent.is_some_and(|h|WaitForSingleObject(h,0)!=WAIT_TIMEOUT){PostQuitMessage(0);return;}
        if self.refresh.elapsed()>Duration::from_secs(1){
            if let Ok(p)=crate::profile::load(&crate::profile::path()){self.prefs=p;}
            self.refresh=Instant::now();
            if !self.prefs.flag("shortcuts","add_word",true){self.close();}
            if let Some(p)=&self.popup{let _=p.set_theme(self.prefs.number("appearance","theme",0));}
        }
        if self.popup.as_ref().is_some_and(|p|!p.active()){self.close();return;}
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
                    Ok(p)=>self.popup=Some(p),Err(_)=>self.close()
                }
            },
            Ok(m) if !m.ready=>self.close(),
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
                }
            }
        },
        START=>STATE.with(|s|{if let Some(s)=s.borrow_mut().as_mut(){s.begin();}}),
        WM_TIMER=>STATE.with(|s|{if let Some(s)=s.borrow_mut().as_mut(){s.poll();}}),
        WM_CLOSE=>{PostQuitMessage(0);return LRESULT(0);},
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
    let devices=[RAWINPUTDEVICE{usUsagePage:1,usUsage:6,dwFlags:RIDEV_INPUTSINK,hwndTarget:window}];
    RegisterRawInputDevices(&devices,std::mem::size_of::<RAWINPUTDEVICE>() as u32)?;
    let data=std::env::current_exe().map_err(|_|Error::from_hresult(E_FAIL))?.parent().unwrap().parent().unwrap().join("data");
    let handle=if parent==0{None}else{Some(OpenProcess(PROCESS_SYNCHRONIZE,false,parent)?)};
    STATE.with(|s|*s.borrow_mut()=Some(State{window,source:HWND::default(),data,prefs:crate::profile::load(&crate::profile::path()).unwrap(),
        refresh:Instant::now(),gesture:Gesture::default(),held:[false;256],epoch:0,worker:None,popup:None,parent:handle,notice:None}));
    SetTimer(Some(window),1,80,None);
    let mut message=MSG::default();while GetMessageW(&mut message,None,0,0).as_bool(){let _=TranslateMessage(&message);DispatchMessageW(&message);}
    STATE.with(|s|{if let Some(mut s)=s.borrow_mut().take(){s.close();if let Some(h)=s.parent{let _=CloseHandle(h);}}});
    let _=DestroyWindow(window);drop(singleton);Ok(())
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
        println!("{}",serde_json::to_string(&Message{ready:true,word:String::new(),rect:[0;4]}).unwrap());
        let _=std::io::stdout().flush();
    }
    println!("{}",serde_json::to_string(&Message{ready:false,word:String::new(),rect:[0;4]}).unwrap());Ok(())
}}
