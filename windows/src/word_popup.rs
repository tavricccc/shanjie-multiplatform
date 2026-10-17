//! Owned, nonactivating add-word panel. The TSF context retains focus and selection.
use shanjie_core::engine::Layout;
use std::{path::PathBuf, sync::mpsc, time::{Duration,Instant}};
use windows::{core::*, Win32::{Foundation::*, Graphics::Gdi::*,
    UI::{Controls::{DRAWITEMSTRUCT,ODS_SELECTED,ODS_FOCUS},HiDpi::*, WindowsAndMessaging::*}}};
enum Answer {
    Inspected(u64,std::result::Result<crate::word_store::Inspection,String>),
    Saved(std::result::Result<String,String>),
}
#[derive(Clone,Copy,PartialEq)]
struct Colors { background:COLORREF,border:COLORREF,text:COLORREF }
impl Colors {
    fn read(theme:u32)->Result<Self> {
        let p=crate::candidate::Palette::new(theme)?;
        let color=|c:windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F|
            COLORREF((c.r*255.0).round() as u32|((c.g*255.0).round() as u32)<<8|((c.b*255.0).round() as u32)<<16);
        Ok(Self{background:color(p.background),border:color(p.border),text:color(p.text)})
    }
}
pub(crate) struct Popup {
    pub window: HWND,
    owner: HWND,
    reading_control: HWND,
    status: HWND,
    inspection_control:HWND,
    font: HFONT,
    theme:u32,
    colors:Colors,
    background:HBRUSH,
    border:HBRUSH,
    word: String,
    data: PathBuf,
    reading: String,
    layout: Layout,
    replace: bool,
    saving: bool,
    headless: bool,
    revision:u64,
    checked_revision:Option<u64>,
    check_due:Option<Instant>,
    checking:bool,
    save_after_check:bool,
    receiver: mpsc::Receiver<Answer>,
    sender: mpsc::Sender<Answer>,
}
impl Popup {
    pub unsafe fn open(owner: HWND, anchor: RECT, word: String, data: PathBuf, layout: Layout,theme:u32) -> Result<Box<Self>> {
        let (sender,receiver) = mpsc::channel();
        let colors=Colors::read(theme)?;
        let mut popup = Box::new(Self { window: HWND::default(), owner, reading_control: HWND::default(),
            status: HWND::default(),inspection_control:HWND::default(), font: HFONT::default(),theme,colors,
            background:CreateSolidBrush(colors.background),border:CreateSolidBrush(colors.border), word, data, reading: String::new(),
            layout, replace: true, saving: false, headless: std::env::var_os("SHANJIE_HEADLESS").is_some(),
            revision:0,checked_revision:None,check_due:None,checking:false,save_after_check:false,receiver, sender });
        if popup.background.is_invalid()||popup.border.is_invalid(){return Err(Error::from_thread());}
        let module = HINSTANCE(crate::tsf::module().0);
        let class = WNDCLASSW { lpfnWndProc: Some(procedure), hInstance: module,
            hCursor: LoadCursorW(None,IDC_ARROW)?,
            lpszClassName: w!("Shanjie.AddWord.Popup.v1"), ..Default::default() };
        RegisterClassW(&class);
        let dpi = GetDpiForWindow(owner).max(96) as i32;
        let scale = |v:i32| v * dpi / 96;
        let title_height=28*((popup.word.chars().count()+13+23)/24) as i32;
        let reading_height=30*((popup.word.chars().count()*5+23)/24).max(1) as i32;
        let label_y=18+title_height;
        let reading_y=label_y+28;
        let inspection_y=reading_y+reading_height+8;
        let status_y=inspection_y+68;
        let button_y=status_y+64;
        let (width,height) = (scale(440), scale(button_y+50));
        let monitor = MonitorFromRect(&anchor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        GetMonitorInfoW(monitor, &mut info).ok()?;
        let work = info.rcWork;
        let x = if popup.headless { -12000 } else { anchor.left.max(work.left).min((work.right-width).max(work.left)) };
        let y = if popup.headless { -12000 } else if anchor.bottom + height <= work.bottom { anchor.bottom } else { (anchor.top-height).max(work.top) };
        popup.window = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class.lpszClassName, w!("善解：新增詞條"), WS_POPUP | WS_BORDER, x,y,width,height,
            Some(owner),None,Some(module),Some((&mut *popup as *mut Self).cast()))?;
        popup.font = CreateFontW(-scale(16),0,0,0,400,0,0,0,DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32, w!("Microsoft JhengHei UI"));
        let label = |class:PCWSTR, text:&str, id:usize, x,y,w,h,style| -> Result<HWND> {
            let control = CreateWindowExW(WINDOW_EX_STYLE(0),class,PCWSTR(crate::wide(text).as_ptr()),
                WS_CHILD|WS_VISIBLE|WINDOW_STYLE(style),scale(x),scale(y),scale(w),scale(h),
                Some(popup.window),Some(HMENU(id as _)),Some(module),None)?;
            SendMessageW(control,WM_SETFONT,Some(WPARAM(popup.font.0 as usize)),Some(LPARAM(1))); Ok(control)
        };
        label(w!("STATIC"),&format!("加入使用者字典：{}",popup.word),1,14,12,410,title_height,0)?;
        label(w!("STATIC"),"讀音（輸入注音重填；Ctrl+A 清空）",2,14,label_y,410,22,0)?;
        popup.reading_control = label(w!("STATIC"),"正在取得讀音建議…",3,14,reading_y,410,reading_height,WS_BORDER.0)?;
        popup.inspection_control=label(w!("STATIC"),"正在比對內建詞庫與目前模型…",5,14,inspection_y,410,64,0)?;
        popup.status = label(w!("STATIC"),"請確認多音字；空白分節，Enter 加入，Esc 取消。",4,14,status_y,410,46,0)?;
        label(w!("BUTTON"),"加入",10,236,button_y,88,32,BS_OWNERDRAW as u32)?;
        label(w!("BUTTON"),"取消",11,336,button_y,88,32,BS_OWNERDRAW as u32)?;
        let _ = ShowWindow(popup.window,SW_SHOWNOACTIVATE);
        SetTimer(Some(popup.window),1,100,None);
        popup.inspect(true);
        Ok(popup)
    }
    pub fn active(&self) -> bool { unsafe { IsWindowVisible(self.window).as_bool() } }
    pub fn close(&mut self) { unsafe { let _ = ShowWindow(self.window,SW_HIDE); let _ = KillTimer(Some(self.window),1); } }
    fn display(&self) { unsafe { let _ = SetWindowTextW(self.reading_control,PCWSTR(crate::wide(&self.reading).as_ptr())); } }
    fn message(&self, text:&str) { unsafe { let _ = SetWindowTextW(self.status,PCWSTR(crate::wide(text).as_ptr())); } }
    fn reading_changed(&mut self) {
        self.revision=self.revision.wrapping_add(1);self.checked_revision=None;self.checking=false;
        self.check_due=Some(Instant::now()+Duration::from_millis(200));self.save_after_check=false;
        unsafe{let _=SetWindowTextW(self.inspection_control,w!("讀音已變更，正在重新比對…"));let _=SetWindowTextW(GetDlgItem(Some(self.window),10).unwrap_or_default(),w!("加入"));}
    }
    fn inspect(&mut self,suggest:bool) {
        self.checking=true;self.check_due=None;
        let (sender,data,word,revision)=(self.sender.clone(),self.data.clone(),self.word.clone(),self.revision);
        let reading=if suggest{None}else{Some(self.reading.trim_end_matches('-').to_owned())};
        std::thread::spawn(move||{let _=sender.send(Answer::Inspected(revision,crate::broker::word_inspection(&data,&word,reading.as_deref())));});
    }
    fn save(&mut self) {
        if self.saving { return; }
        let reading = self.reading.trim_end_matches('-').to_owned();
        if let Err(e) = crate::profile::dictionary_content(&format!("{reading} {}",self.word)) {
            self.message(&e.to_string()); return;
        }
        if self.checked_revision!=Some(self.revision) {
            self.save_after_check=true;self.message("正在完成比對；已有詞條時會先提示，再由你確認加入。");
            if !self.checking {self.inspect(false);}return;
        }
        self.begin_save(reading);
    }
    fn begin_save(&mut self,reading:String) {
        self.saving = true; self.message("正在儲存…");
        let sender = self.sender.clone(); let data = self.data.clone(); let word = self.word.clone();
        std::thread::spawn(move || { let _ = sender.send(Answer::Saved(crate::broker::word_request(&data,&word,Some(&reading)))); });
    }
    pub fn accepts(vk:usize, ctrl:bool, alt:bool, win:bool) -> bool {
        !alt && !win && (!ctrl || vk == 0x41) && ![0x10,0x11,0x12,0xa0,0xa1,0xa2,0xa3].contains(&vk)
    }
    pub fn key(&mut self, vk:usize, ch:Option<char>, ctrl:bool) {
        if vk == 27 { self.close(); return; }
        if self.saving { return; }
        if vk == 13 { self.save(); return; }
        if ctrl && vk == 0x41 { self.reading.clear(); self.replace = false; self.display();self.reading_changed(); return; }
        if vk == 8 {
            if self.replace { self.reading.clear(); self.replace = false; }
            else { self.reading.pop(); } self.display();self.reading_changed(); return;
        }
        if vk == 32 {
            if self.replace { self.reading.clear(); self.replace = false; }
            if !self.reading.is_empty() && !self.reading.ends_with('-') { self.reading.push('-'); }
        } else if let Some(ch) = ch {
            if self.replace { self.reading.clear(); self.replace = false; }
            if let Some(mark) = ['ˊ','ˇ','ˋ','˙'].into_iter().find(|&mark| self.layout.key_of_tone(mark) == Some(ch)) {
                if !self.reading.is_empty() && !self.reading.ends_with('-') { self.reading.push(mark); }
            } else if let Some((column,symbol)) = self.layout.symbol_of(ch) {
                if self.reading.ends_with(['ˊ','ˇ','ˋ','˙']) { self.reading.push('-'); }
                let at = self.reading.rfind('-').map_or(0,|i|i+1);
                let mut columns = [None;3];
                for c in self.reading[at..].chars() {
                    if let Some((i,_)) = self.layout.key_of_symbol(c).and_then(|k|self.layout.symbol_of(k)) { columns[i]=Some(c); }
                }
                columns[column] = Some(symbol); self.reading.truncate(at);
                self.reading.extend(columns.into_iter().flatten());
            }
        }
        self.reading.truncate(self.reading.char_indices().nth(255).map_or(self.reading.len(),|(i,_)|i));
        self.display();
        self.reading_changed();
    }
    unsafe fn poll(&mut self) {
        let foreground = GetForegroundWindow();
        if !self.headless && !foreground.is_invalid() && GetAncestor(foreground,GA_ROOT) != GetAncestor(self.owner,GA_ROOT) { self.close(); return; }
        if self.check_due.is_some_and(|due|Instant::now()>=due){self.inspect(false);}
        while let Ok(answer) = self.receiver.try_recv() {
            if !self.active() { continue; }
            match answer {
                Answer::Saved(result)=>{self.saving=false;match result{Ok(_)=>self.close(),Err(e)=>self.message(&e)}}
                Answer::Inspected(revision,result) if revision==self.revision=>{
                    self.checking=false;
                    match result {
                        Ok(info)=>{
                            if self.replace {self.reading=info.reading.clone();self.display();}
                            self.checked_revision=Some(revision);
                            let builtin=if info.builtin_exact{"已有同詞同讀音"}else if info.builtin_word{"已有此詞，讀音不同"}else{"未收錄此詞"};
                            let personal=if info.personal_exact{"已有同詞同讀音"}else if info.personal_word{"已有此詞，讀音不同"}else{"尚未加入"};
                            let model=match info.model_word{Some(true)=>"有此詞",Some(false)=>"無完整詞條",None=>"無法讀取，尚未確認"};
                            let text=format!("內建詞庫：{builtin}；個人詞庫：{personal}\n目前模型：{model}（{}{}）",info.model_id,if info.model_fallback{"，內建備援"}else{""});
                            let _=SetWindowTextW(self.inspection_control,PCWSTR(crate::wide(&text).as_ptr()));
                            let exists=info.builtin_word||info.personal_word||info.model_word==Some(true);
                            let _=SetWindowTextW(GetDlgItem(Some(self.window),10).unwrap_or_default(),if exists{w!("仍要加入")}else{w!("加入")});
                            if self.reading.is_empty(){self.message("找不到讀音，請直接輸入注音；空白分節。");}
                            else if exists{self.message("已有收錄；若要指定個人讀音或優先詞條，仍可按「仍要加入」。");}
                            else{self.message("請確認讀音；Enter 加入，Esc 取消。");}
                            let auto=std::mem::take(&mut self.save_after_check);
                            if auto&&!exists {self.begin_save(self.reading.trim_end_matches('-').into());}
                        },
                        Err(e)=>{self.save_after_check=false;self.display();self.message(&e);let _=SetWindowTextW(self.inspection_control,w!("比對未完成；草稿已保留，按加入可重試。"));}
                    }
                },
                _=>{}
            }
        }
    }
    pub fn set_theme(&mut self,theme:u32)->Result<()> {self.theme=theme;unsafe{self.refresh_colors()}}
    unsafe fn refresh_colors(&mut self)->Result<()> {
        let colors=Colors::read(self.theme)?;
        if colors!=self.colors {
            let background=CreateSolidBrush(colors.background);let border=CreateSolidBrush(colors.border);
            if background.is_invalid()||border.is_invalid(){if !background.is_invalid(){let _=DeleteObject(background.into());}if !border.is_invalid(){let _=DeleteObject(border.into());}return Err(Error::from_thread());}
            let _=DeleteObject(self.background.into());let _=DeleteObject(self.border.into());
            self.background=background;self.border=border;self.colors=colors;
            let _=InvalidateRect(Some(self.window),None,true);
            for id in [1,2,3,4,5,10,11]{if let Ok(control)=GetDlgItem(Some(self.window),id){let _=InvalidateRect(Some(control),None,true);}}
        }
        Ok(())
    }
    unsafe fn draw_button(&self,draw:&DRAWITEMSTRUCT) {
        let selected=draw.itemState.0&ODS_SELECTED.0!=0;
        FillRect(draw.hDC,&draw.rcItem,if selected{self.border}else{self.background});FrameRect(draw.hDC,&draw.rcItem,self.border);
        let old_font=SelectObject(draw.hDC,self.font.into());SetBkMode(draw.hDC,TRANSPARENT);SetTextColor(draw.hDC,self.colors.text);
        let mut text=vec![0u16;GetWindowTextLengthW(draw.hwndItem) as usize+1];let n=GetWindowTextW(draw.hwndItem,&mut text);text.truncate(n as usize);
        let mut rect=draw.rcItem;DrawTextW(draw.hDC,&mut text,&mut rect,DT_SINGLELINE|DT_CENTER|DT_VCENTER|DT_NOPREFIX);
        if draw.itemState.0&ODS_FOCUS.0!=0{let mut focus=draw.rcItem;let _=InflateRect(&mut focus,-3,-3);let _=DrawFocusRect(draw.hDC,&focus);}
        SelectObject(draw.hDC,old_font);
    }
}
impl Drop for Popup { fn drop(&mut self) { unsafe {
    let _ = DestroyWindow(self.window); if !self.font.is_invalid() { let _ = DeleteObject(self.font.into()); }
    if !self.background.is_invalid(){let _=DeleteObject(self.background.into());}if !self.border.is_invalid(){let _=DeleteObject(self.border.into());}
} } }
unsafe extern "system" fn procedure(window:HWND, message:u32, wp:WPARAM, lp:LPARAM) -> LRESULT {
    if message == WM_NCCREATE { SetWindowLongPtrW(window,GWLP_USERDATA,(*(lp.0 as *const CREATESTRUCTW)).lpCreateParams as _); }
    let ptr = GetWindowLongPtrW(window,GWLP_USERDATA) as *mut Popup;
    if !ptr.is_null() { match message {
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        WM_CTLCOLORSTATIC => {
            let dc=HDC(wp.0 as _);SetBkColor(dc,(*ptr).colors.background);SetTextColor(dc,(*ptr).colors.text);
            return LRESULT((*ptr).background.0 as isize);
        },
        WM_ERASEBKGND=>{let mut rect=RECT::default();let _=GetClientRect(window,&mut rect);FillRect(HDC(wp.0 as _),&rect,(*ptr).background);return LRESULT(1);},
        WM_DRAWITEM if lp.0!=0=>{(*ptr).draw_button(&*(lp.0 as *const DRAWITEMSTRUCT));return LRESULT(1);},
        WM_SETTINGCHANGE|WM_THEMECHANGED|WM_SYSCOLORCHANGE=>{let _=(*ptr).refresh_colors();return LRESULT(0);},
        WM_TIMER => { (*ptr).poll(); return LRESULT(0); },
        WM_COMMAND => { match wp.0 & 0xffff { 10 => (*ptr).save(), 11 => (*ptr).close(), _ => {} } return LRESULT(0); },
        WM_CLOSE => { (*ptr).close(); return LRESULT(0); },
        WM_NCDESTROY => { SetWindowLongPtrW(window,GWLP_USERDATA,0); },
        _ => {}
    } }
    DefWindowProcW(window,message,wp,lp)
}
