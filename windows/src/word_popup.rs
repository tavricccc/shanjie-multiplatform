//! Owned, nonactivating add-word panel. The TSF context retains focus and selection.
use shanjie_core::engine::Layout;
use std::{path::PathBuf, sync::mpsc};
use windows::{core::*, Win32::{Foundation::*, Graphics::Gdi::*,
    UI::{HiDpi::*, WindowsAndMessaging::*}}};
type Answer = (bool, std::result::Result<String,String>);
pub(crate) struct Popup {
    pub window: HWND,
    owner: HWND,
    reading_control: HWND,
    status: HWND,
    font: HFONT,
    word: String,
    data: PathBuf,
    reading: String,
    layout: Layout,
    replace: bool,
    saving: bool,
    headless: bool,
    receiver: mpsc::Receiver<Answer>,
    sender: mpsc::Sender<Answer>,
}
impl Popup {
    pub unsafe fn open(owner: HWND, anchor: RECT, word: String, data: PathBuf, layout: Layout) -> Result<Box<Self>> {
        let (sender,receiver) = mpsc::channel();
        let mut popup = Box::new(Self { window: HWND::default(), owner, reading_control: HWND::default(),
            status: HWND::default(), font: HFONT::default(), word, data, reading: String::new(),
            layout, replace: true, saving: false, headless: std::env::var_os("SHANJIE_HEADLESS").is_some(), receiver, sender });
        let module = HINSTANCE(crate::tsf::module().0);
        let class = WNDCLASSW { lpfnWndProc: Some(procedure), hInstance: module,
            hCursor: LoadCursorW(None,IDC_ARROW)?, hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as usize as _),
            lpszClassName: w!("Shanjie.AddWord.Popup.v1"), ..Default::default() };
        RegisterClassW(&class);
        let dpi = GetDpiForWindow(owner).max(96) as i32;
        let scale = |v:i32| v * dpi / 96;
        let title_height=28*((popup.word.chars().count()+13+23)/24) as i32;
        let reading_height=30*((popup.word.chars().count()*5+23)/24).max(1) as i32;
        let label_y=18+title_height;
        let reading_y=label_y+28;
        let status_y=reading_y+reading_height+8;
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
        popup.status = label(w!("STATIC"),"請確認多音字；空白分節，Enter 加入，Esc 取消。",4,14,status_y,410,46,0)?;
        label(w!("BUTTON"),"加入",10,236,button_y,88,32,BS_PUSHBUTTON as u32)?;
        label(w!("BUTTON"),"取消",11,336,button_y,88,32,BS_PUSHBUTTON as u32)?;
        let _ = ShowWindow(popup.window,SW_SHOWNOACTIVATE);
        SetTimer(Some(popup.window),1,100,None);
        let sender = popup.sender.clone(); let data = popup.data.clone(); let word = popup.word.clone();
        std::thread::spawn(move || { let _ = sender.send((false,crate::broker::word_request(&data,&word,None))); });
        Ok(popup)
    }
    pub fn active(&self) -> bool { unsafe { IsWindowVisible(self.window).as_bool() } }
    pub fn close(&mut self) { unsafe { let _ = ShowWindow(self.window,SW_HIDE); let _ = KillTimer(Some(self.window),1); } }
    fn display(&self) { unsafe { let _ = SetWindowTextW(self.reading_control,PCWSTR(crate::wide(&self.reading).as_ptr())); } }
    fn message(&self, text:&str) { unsafe { let _ = SetWindowTextW(self.status,PCWSTR(crate::wide(text).as_ptr())); } }
    fn save(&mut self) {
        if self.saving { return; }
        let reading = self.reading.trim_end_matches('-').to_owned();
        if let Err(e) = crate::profile::dictionary_content(&format!("{reading} {}",self.word)) {
            self.message(&e.to_string()); return;
        }
        self.saving = true; self.message("正在儲存…");
        let sender = self.sender.clone(); let data = self.data.clone(); let word = self.word.clone();
        std::thread::spawn(move || { let _ = sender.send((true,crate::broker::word_request(&data,&word,Some(&reading)))); });
    }
    pub fn accepts(vk:usize, ctrl:bool, alt:bool, win:bool) -> bool {
        !alt && !win && (!ctrl || vk == 0x41) && ![0x10,0x11,0x12,0xa0,0xa1,0xa2,0xa3].contains(&vk)
    }
    pub fn key(&mut self, vk:usize, ch:Option<char>, ctrl:bool) {
        if vk == 27 { self.close(); return; }
        if self.saving { return; }
        if vk == 13 { self.save(); return; }
        if ctrl && vk == 0x41 { self.reading.clear(); self.replace = false; self.display(); return; }
        if vk == 8 {
            if self.replace { self.reading.clear(); self.replace = false; }
            else { self.reading.pop(); } self.display(); return;
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
    }
    unsafe fn poll(&mut self) {
        let foreground = GetForegroundWindow();
        if !self.headless && !foreground.is_invalid() && GetAncestor(foreground,GA_ROOT) != GetAncestor(self.owner,GA_ROOT) { self.close(); return; }
        while let Ok((saved,result)) = self.receiver.try_recv() {
            if !self.active() { continue; }
            if saved { self.saving = false; match result { Ok(_) => self.close(), Err(e) => self.message(&e) } }
            else if self.replace { match result {
                Ok(reading) => { self.reading=reading; self.display(); if self.reading.is_empty() { self.message("找不到讀音，請直接輸入注音；空白分節。 "); } },
                Err(e) => { self.display(); self.message(&e); }
            } }
        }
    }
}
impl Drop for Popup { fn drop(&mut self) { unsafe {
    let _ = DestroyWindow(self.window); if !self.font.is_invalid() { let _ = DeleteObject(self.font.into()); }
} } }
unsafe extern "system" fn procedure(window:HWND, message:u32, wp:WPARAM, lp:LPARAM) -> LRESULT {
    if message == WM_NCCREATE { SetWindowLongPtrW(window,GWLP_USERDATA,(*(lp.0 as *const CREATESTRUCTW)).lpCreateParams as _); }
    let ptr = GetWindowLongPtrW(window,GWLP_USERDATA) as *mut Popup;
    if !ptr.is_null() { match message {
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        WM_CTLCOLORSTATIC => {
            let dc=HDC(wp.0 as _); SetBkColor(dc,COLORREF(GetSysColor(COLOR_WINDOW))); SetTextColor(dc,COLORREF(GetSysColor(COLOR_WINDOWTEXT)));
            return LRESULT(GetSysColorBrush(COLOR_WINDOW).0 as isize);
        },
        WM_TIMER => { (*ptr).poll(); return LRESULT(0); },
        WM_COMMAND => { match wp.0 & 0xffff { 10 => (*ptr).save(), 11 => (*ptr).close(), _ => {} } return LRESULT(0); },
        WM_CLOSE => { (*ptr).close(); return LRESULT(0); },
        WM_NCDESTROY => { SetWindowLongPtrW(window,GWLP_USERDATA,0); },
        _ => {}
    } }
    DefWindowProcW(window,message,wp,lp)
}
