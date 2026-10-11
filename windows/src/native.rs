use windows::{
    core::{w, Result},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
};
pub struct ControlChannel {
    window: HWND,
    callback: unsafe extern "C" fn(*mut std::ffi::c_void, bool),
    context: *mut std::ffi::c_void,
    message: u32,
    restart: u32,
}
impl ControlChannel {
    pub unsafe fn new(
        callback: unsafe extern "C" fn(*mut std::ffi::c_void, bool),
        context: *mut std::ffi::c_void,
    ) -> Result<Box<Self>> {
        let mut w = Box::new(Self {
            window: HWND::default(),
            callback,
            context,
            message: RegisterWindowMessageW(w!("Shanjie.ProfileChanged.v1")),
            restart: RegisterWindowMessageW(w!("Shanjie.Restart.v1")),
        });
        let module = crate::tsf::module();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(watcher_proc),
            hInstance: HINSTANCE(module.0),
            lpszClassName: w!("ShanjieRustProfileControlChannel"),
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
}
impl Drop for ControlChannel {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
            let _ = UnregisterClassW(
                w!("ShanjieRustProfileControlChannel"),
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
    if !p.is_null() && (message == (*p).message || message == (*p).restart) {
        ((*p).callback)((*p).context, message == (*p).restart);
        return LRESULT(0);
    }
    DefWindowProcW(window, message, wp, lp)
}

fn icon(size: u32, chinese: bool) -> tiny_skia::Pixmap {
    use tiny_skia::*;
    let mut pixmap = Pixmap::new(size, size).unwrap();
    let mut path = PathBuilder::new();
    let k = 2.209139;
    path.move_to(5.0, 1.0);
    path.line_to(19.0, 1.0);
    path.cubic_to(19.0 + k, 1.0, 23.0, 5.0 - k, 23.0, 5.0);
    path.line_to(23.0, 19.0);
    path.cubic_to(23.0, 19.0 + k, 19.0 + k, 23.0, 19.0, 23.0);
    path.line_to(5.0, 23.0);
    path.cubic_to(5.0 - k, 23.0, 1.0, 19.0 + k, 1.0, 19.0);
    path.line_to(1.0, 5.0);
    path.cubic_to(1.0, 5.0 - k, 5.0 - k, 1.0, 5.0, 1.0);
    path.close();
    let mut paint = Paint::default();
    paint.set_color_rgba8(
        if chinese { 0x24 } else { 0x47 },
        if chinese { 0x61 } else { 0x55 },
        if chinese { 0x78 } else { 0x69 },
        255,
    );
    let scale = Transform::from_scale(size as f32 / 24.0, size as f32 / 24.0);
    pixmap.fill_path(
        &path.finish().unwrap(),
        &paint,
        FillRule::Winding,
        scale,
        None,
    );
    let mut path = PathBuilder::new();
    if chinese {
        path.move_to(12.0, 4.5);
        path.line_to(12.0, 19.5);
        path.move_to(5.5, 8.0);
        path.line_to(18.5, 8.0);
        path.line_to(18.5, 16.0);
        path.line_to(5.5, 16.0);
        path.close();
    } else {
        path.move_to(6.5, 18.0);
        path.line_to(12.0, 5.5);
        path.line_to(17.5, 18.0);
        path.move_to(8.25, 14.0);
        path.line_to(15.75, 14.0);
    }
    paint.set_color_rgba8(255, 255, 255, 255);
    pixmap.stroke_path(
        &path.finish().unwrap(),
        &paint,
        &Stroke {
            width: 2.2,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        },
        scale,
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
