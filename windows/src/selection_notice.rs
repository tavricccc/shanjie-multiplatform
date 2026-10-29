//! Brief owned, nonactivating feedback. It never takes the editor's focus.
use windows::{core::*,Win32::{Foundation::*,Graphics::Gdi::*,UI::WindowsAndMessaging::*}};
pub(crate) struct Notice(HWND);
impl Drop for Notice { fn drop(&mut self) { unsafe { let _=DestroyWindow(self.0); } } }
impl Notice {
    pub(crate) unsafe fn open(owner:HWND,theme:u32)->Result<Self> {
        Self::open_at(owner,owner,theme)
    }
    pub(crate) unsafe fn open_at(owner:HWND,source:HWND,theme:u32)->Result<Self> {
        let class=w!("Shanjie.Selection.Notice.v1");
        RegisterClassW(&WNDCLASSW{lpfnWndProc:Some(procedure),hInstance:HINSTANCE(crate::tsf::module().0),lpszClassName:class,..Default::default()});
        let mut rect=RECT::default(); GetWindowRect(source,&mut rect)?;
        let width=390; let height=60;
        let mut monitor=MONITORINFO{cbSize:std::mem::size_of::<MONITORINFO>() as u32,..Default::default()};
        GetMonitorInfoW(MonitorFromWindow(source,MONITOR_DEFAULTTONEAREST),&mut monitor).ok()?;
        let x=rect.left.clamp(monitor.rcWork.left,(monitor.rcWork.right-width).max(monitor.rcWork.left));
        let y=(rect.top+40).clamp(monitor.rcWork.top,(monitor.rcWork.bottom-height).max(monitor.rcWork.top));
        let window=CreateWindowExW(WS_EX_NOACTIVATE|WS_EX_TOOLWINDOW,class,
            w!("此欄位無法取得反白內容，請重新反白後再試。"),WS_POPUP|WS_BORDER,
            if std::env::var_os("SHANJIE_HEADLESS").is_some(){-12000}else{x},y,width,height,Some(owner),None,Some(HINSTANCE(crate::tsf::module().0)),None)?;
        SetWindowLongPtrW(window,GWLP_USERDATA,theme as _);
        SetPropW(window,w!("Shanjie.Notice.Source"),Some(HANDLE(source.0)))?;
        let dpi=windows::Win32::UI::HiDpi::GetDpiForWindow(window).max(96) as i32;
        let width=390*dpi/96;let height=60*dpi/96;
        let x=x.clamp(monitor.rcWork.left,(monitor.rcWork.right-width).max(monitor.rcWork.left));
        let y=y.clamp(monitor.rcWork.top,(monitor.rcWork.bottom-height).max(monitor.rcWork.top));
        SetWindowPos(window,Some(HWND_TOPMOST),if std::env::var_os("SHANJIE_HEADLESS").is_some(){-12000}else{x},y,width,height,SWP_NOACTIVATE)?;
        SetTimer(Some(window),1,4000,None);SetTimer(Some(window),2,100,None);
        let _=ShowWindow(window,SW_SHOWNOACTIVATE);
        Ok(Self(window))
    }
}
unsafe extern "system" fn procedure(window:HWND,message:u32,wp:WPARAM,lp:LPARAM)->LRESULT {
    if message==WM_TIMER {
        let source=HWND(GetPropW(window,w!("Shanjie.Notice.Source")).0);
        if wp.0==1||!IsWindow(Some(source)).as_bool()||GetAncestor(GetForegroundWindow(),GA_ROOT)!=GetAncestor(source,GA_ROOT){
            let _=ShowWindow(window,SW_HIDE);let _=KillTimer(Some(window),1);let _=KillTimer(Some(window),2);
        }
        return LRESULT(0);
    }
    if message==WM_DPICHANGED {let _=ShowWindow(window,SW_HIDE);return LRESULT(0);}
    if message==WM_NCDESTROY {let _=RemovePropW(window,w!("Shanjie.Notice.Source"));}
    if message==WM_MOUSEACTIVATE { return LRESULT(MA_NOACTIVATE as isize); }
    if message==WM_PAINT {
        let mut paint=PAINTSTRUCT::default(); let dc=BeginPaint(window,&mut paint);
        if let Ok(p)=crate::candidate::Palette::new(GetWindowLongPtrW(window,GWLP_USERDATA) as u32) {
            let color=|v:windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F|COLORREF((v.r*255.) as u32|((v.g*255.) as u32)<<8|((v.b*255.) as u32)<<16);
            let brush=CreateSolidBrush(color(p.background)); let mut rect=RECT::default(); let _=GetClientRect(window,&mut rect);
            FillRect(dc,&rect,brush); let _=DeleteObject(brush.into()); SetTextColor(dc,color(p.text)); SetBkMode(dc,TRANSPARENT);
            let dpi=windows::Win32::UI::HiDpi::GetDpiForWindow(window).max(96) as i32;
            let font=CreateFontW(-16*dpi/96,0,0,0,400,0,0,0,DEFAULT_CHARSET,OUT_DEFAULT_PRECIS,CLIP_DEFAULT_PRECIS,CLEARTYPE_QUALITY,DEFAULT_PITCH.0 as u32,w!("Microsoft JhengHei UI"));
            let old=SelectObject(dc,font.into()); rect.left+=12*dpi/96;rect.right-=12*dpi/96;
            let mut text=crate::wide("此欄位無法取得反白內容，請重新反白後再試。"); text.pop();
            DrawTextW(dc,&mut text,&mut rect,DT_LEFT|DT_VCENTER|DT_SINGLELINE|DT_END_ELLIPSIS);
            SelectObject(dc,old);let _=DeleteObject(font.into());
        }
        let _=EndPaint(window,&paint); return LRESULT(0);
    }
    DefWindowProcW(window,message,wp,lp)
}
