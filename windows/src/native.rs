use windows::{
    core::{w, PCWSTR, Result},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
};
pub struct ControlChannel {
    window: HWND,
    callback: unsafe extern "C" fn(*mut std::ffi::c_void, u32),
    context: *mut std::ffi::c_void,
    message: u32,
    restart: u32,
    legacy_message: u32,
    legacy_restart: u32,
}
impl ControlChannel {
    pub unsafe fn new(
        callback: unsafe extern "C" fn(*mut std::ffi::c_void, u32),
        context: *mut std::ffi::c_void,
    ) -> Result<Box<Self>> {
        let mut w = Box::new(Self {
            window: HWND::default(),
            callback,
            context,
            message: RegisterWindowMessageW(PCWSTR(crate::wide(crate::profile::CHANGED_MESSAGE).as_ptr())),
            restart: RegisterWindowMessageW(PCWSTR(crate::wide(crate::profile::RESTART_MESSAGE).as_ptr())),
            legacy_message: RegisterWindowMessageW(w!("Shanjie.ProfileChanged.v1")),
            legacy_restart: RegisterWindowMessageW(w!("Shanjie.Restart.v1")),
        });
        let module = crate::tsf::module();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(watcher_proc),
            hInstance: HINSTANCE(module.0),
            lpszClassName: w!("ShanjieRustProfileControlChannel.v2"),
            ..Default::default()
        };
        RegisterClassW(&wc);
        w.window = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            wc.lpszClassName,
            w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(wc.hInstance),
            Some((&mut *w as *mut Self).cast()),
        )?;
        Ok(w)
    }
    pub fn retry(&self) {
        unsafe {
            SetTimer(Some(self.window), 42, 50, None);
        }
    }
    pub fn add_word(&self) { unsafe { let _ = PostMessageW(Some(self.window),WM_APP+91,WPARAM(0),LPARAM(0)); } }
    pub fn stop_retry(&self) {
        unsafe {
            let _ = KillTimer(Some(self.window), 42);
        }
    }
}
impl Drop for ControlChannel {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
            let _ = UnregisterClassW(
                w!("ShanjieRustProfileControlChannel.v2"),
                Some(HINSTANCE(crate::tsf::module().0)),
            );
        }
    }
}
unsafe extern "system" fn watcher_proc(
    window: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    let mut p = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut ControlChannel;
    if message == WM_NCCREATE {
        p = (*(lp.0 as *const CREATESTRUCTW)).lpCreateParams.cast();
        SetWindowLongPtrW(window, GWLP_USERDATA, p as _);
    }
    if !p.is_null()
        && [(*p).message, (*p).restart, (*p).legacy_message, (*p).legacy_restart].contains(&message)
    {
        ((*p).callback)((*p).context, [(*p).restart, (*p).legacy_restart].contains(&message) as u32);
        return LRESULT(0);
    }
    if !p.is_null() && message == WM_TIMER && wp.0 == 42 {
        let _ = KillTimer(Some(window), 42);
        ((*p).callback)((*p).context, 2);
        return LRESULT(0);
    }
    if !p.is_null() && message == WM_APP+91 {
        ((*p).callback)((*p).context,4); return LRESULT(0);
    }
    DefWindowProcW(window, message, wp, lp)
}

fn icon(size: u32, chinese: bool) -> tiny_skia::Pixmap {
    use tiny_skia::*;
    // Master artwork from scripts/make-icons.ps1, scaled to the DPI-sized hint.
    let master = Pixmap::decode_png(if chinese {
        include_bytes!("../../assets/mode/chinese.png")
    } else {
        include_bytes!("../../assets/mode/english.png")
    })
    .unwrap();
    let mut pixmap = Pixmap::new(size, size).unwrap();
    let scale = size as f32 / master.width() as f32;
    pixmap.draw_pixmap(
        0,
        0,
        master.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..Default::default()
        },
        Transform::from_scale(scale, scale),
        None,
    );
    pixmap
}
pub struct Indicator {
    window: HWND,
    bitmap: HBITMAP,
}
impl Default for Indicator {
    fn default() -> Self {
        Self {
            window: HWND::default(),
            bitmap: HBITMAP::default(),
        }
    }
}
impl Indicator {
    pub unsafe fn hide(&mut self) {
        if !self.window.is_invalid() {
            let _ = KillTimer(Some(self.window), 1);
            let _ = ShowWindow(self.window, SW_HIDE);
        }
    }
    pub unsafe fn show(&mut self, caret: RECT, mut dpi: u32, chinese: bool) -> Result<()> {
        if std::env::var_os("SHANJIE_HEADLESS").is_some() {
            return Ok(());
        }
        let awareness = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let result = (|| -> Result<()> {
            if self.window.is_invalid() {
                let module = crate::tsf::module();
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(indicator_proc),
                    hInstance: HINSTANCE(module.0),
                    lpszClassName: w!("ShanjieRustModeIndicator"),
                    ..Default::default()
                };
                RegisterClassW(&wc);
                self.window = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                    wc.lpszClassName,
                    w!(""),
                    WS_POPUP,
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    Some(wc.hInstance),
                    Some((self as *mut Self).cast()),
                )?;
            }
            SetWindowPos(
                self.window,
                None,
                caret.left,
                caret.bottom,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
            )?;
            if std::env::var_os("SHANJIE_VERIFY_UI").is_none() {
                dpi = GetDpiForWindow(self.window);
            }
            let size = (28 * dpi + 48) / 96;
            let gap = (6 * dpi + 48) / 96;
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(self.bitmap.into());
            }
            let pixmap = icon(size, chinese);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size as i32,
                    biHeight: -(size as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixels = std::ptr::null_mut();
            self.bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut pixels, None, 0)?;
            let bytes = std::slice::from_raw_parts_mut(pixels.cast::<u8>(), pixmap.data().len());
            for (source, target) in pixmap.data().chunks_exact(4).zip(bytes.chunks_exact_mut(4)) {
                target.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
            }
            let mut monitor = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            GetMonitorInfoW(
                MonitorFromRect(&caret, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            )
            .ok()?;
            let mut position = POINT {
                x: caret
                    .left
                    .clamp(monitor.rcWork.left, monitor.rcWork.right - size as i32),
                y: caret.bottom + gap as i32,
            };
            if position.y + size as i32 > monitor.rcWork.bottom {
                position.y = caret.top - size as i32 - gap as i32;
            }
            if std::env::var_os("SHANJIE_VERIFY_UI").is_some() {
                position = POINT {
                    x: -12000,
                    y: -12000,
                };
            }
            let dc = CreateCompatibleDC(None);
            let old = SelectObject(dc, self.bitmap.into());
            let source = POINT::default();
            let dimensions = SIZE {
                cx: size as i32,
                cy: size as i32,
            };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let result = UpdateLayeredWindow(
                self.window,
                None,
                Some(&position),
                Some(&dimensions),
                Some(dc),
                Some(&source),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
            SelectObject(dc, old);
            let _ = DeleteDC(dc);
            result?;
            let _ = ShowWindow(self.window, SW_SHOWNOACTIVATE);
            SetTimer(Some(self.window), 1, 1400, None);
            Ok(())
        })();
        SetThreadDpiAwarenessContext(awareness);
        result
    }
}
impl Drop for Indicator {
    fn drop(&mut self) {
        unsafe {
            if !self.window.is_invalid() {
                let _ = DestroyWindow(self.window);
            }
            let _ = UnregisterClassW(
                w!("ShanjieRustModeIndicator"),
                Some(HINSTANCE(crate::tsf::module().0)),
            );
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(self.bitmap.into());
            }
        }
    }
}
unsafe extern "system" fn indicator_proc(
    window: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    let mut p = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut Indicator;
    if message == WM_NCCREATE {
        p = (*(lp.0 as *const CREATESTRUCTW)).lpCreateParams.cast();
        SetWindowLongPtrW(window, GWLP_USERDATA, p as _);
    }
    if message == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    if !p.is_null() {
        if message == WM_TIMER {
            (*p).hide();
            return LRESULT(0);
        }
        if message == WM_PRINTCLIENT && !(*p).bitmap.is_invalid() {
            let dc = CreateCompatibleDC(None);
            let old = SelectObject(dc, (*p).bitmap.into());
            let mut bitmap = BITMAP::default();
            GetObjectW(
                (*p).bitmap.into(),
                std::mem::size_of::<BITMAP>() as i32,
                Some((&mut bitmap as *mut BITMAP).cast()),
            );
            let _ = BitBlt(
                HDC(wp.0 as _),
                0,
                0,
                bitmap.bmWidth,
                bitmap.bmHeight,
                Some(dc),
                0,
                0,
                SRCCOPY,
            );
            SelectObject(dc, old);
            let _ = DeleteDC(dc);
            return LRESULT(0);
        }
    }
    DefWindowProcW(window, message, wp, lp)
}
#[no_mangle]
pub extern "C" fn sj_indicator_new() -> *mut Indicator {
    Box::into_raw(Box::new(Indicator::default()))
}
#[no_mangle]
pub unsafe extern "C" fn sj_indicator_free(p: *mut Indicator) {
    drop(Box::from_raw(p));
}
#[no_mangle]
pub unsafe extern "C" fn sj_indicator_hide(p: *mut Indicator) {
    (*p).hide();
}
#[no_mangle]
pub unsafe extern "C" fn sj_indicator_show(
    p: *mut Indicator,
    caret: RECT,
    dpi: u32,
    chinese: bool,
) {
    let _ = (*p).show(caret, dpi, chinese);
}
