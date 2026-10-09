//! Native background acrylic. The separate, unpainted window leaves DWM's
//! backdrop visible below the layered text/tint surface without taking focus.
use windows::{
    core::{w, Error, Result, PCSTR},
    Win32::{
        Foundation::*,
        Graphics::{Dwm::*, Gdi::*},
        System::LibraryLoader::{GetModuleHandleW, GetProcAddress},
        UI::{Controls::MARGINS, HiDpi::*, WindowsAndMessaging::*},
    },
};

#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    tint: u32,
    animation: u32,
}
#[repr(C)]
struct Attribute {
    kind: u32,
    data: *const AccentPolicy,
    size: usize,
}
type SetAttribute = unsafe extern "system" fn(HWND, *const Attribute) -> windows::core::BOOL;

pub struct Backdrop {
    window: HWND,
    set_attribute: SetAttribute,
    tint: std::cell::Cell<Option<u32>>,
    size: std::cell::Cell<(i32, i32)>,
}
impl Backdrop {
    pub unsafe fn new() -> Result<Self> {
        // User32 exports this API without a Windows SDK import library.
        let user32 = GetModuleHandleW(w!("user32.dll"))?;
        let address = GetProcAddress(
            user32,
            PCSTR(c"SetWindowCompositionAttribute".as_ptr().cast()),
        )
        .ok_or_else(|| Error::new(E_NOTIMPL, "Windows 缺少候選毛玻璃合成 API。"))?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: HINSTANCE(crate::tsf::module().0),
            lpszClassName: w!("ShanjieRustCandidateBackdrop"),
            ..Default::default()
        };
        RegisterClassW(&class);
        let awareness = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let created = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            class.lpszClassName,
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(class.hInstance),
            None,
        );
        SetThreadDpiAwarenessContext(awareness);
        let window = created?;
        let backdrop = Self {
            window,
            set_attribute: std::mem::transmute(address),
            tint: std::cell::Cell::new(None),
            size: std::cell::Cell::new((0, 0)),
        };
        let disabled = 1i32;
        DwmSetWindowAttribute(
            window,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            (&disabled as *const i32).cast(),
            4,
        )?;
        let frame = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        DwmExtendFrameIntoClientArea(window, &frame)?;
        Ok(backdrop)
    }
    pub fn window(&self) -> HWND {
        self.window
    }
    pub unsafe fn show(&self, content: HWND, bounds: RECT, glass: bool, rgb: u32) -> Result<()> {
        if !glass {
            self.hide();
            return Ok(());
        }
        // ACCENT_ENABLE_ACRYLICBLURBEHIND, WCA_ACCENT_POLICY. No border flags.
        // The text window supplies the visible tint; use a minimal native tint.
        let policy = AccentPolicy {
            state: 4,
            flags: 0,
            tint: 0x01000000 | ((rgb & 255) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 255),
            animation: 0,
        };
        let attribute = Attribute {
            kind: 19,
            data: &policy,
            size: std::mem::size_of::<AccentPolicy>(),
        };
        if self.tint.get() != Some(rgb) {
            if !(self.set_attribute)(self.window, &attribute).as_bool() {
                return Err(Error::new(E_FAIL, "Windows 無法套用候選毛玻璃。"));
            }
            self.tint.set(Some(rgb));
        }
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        if self.size.get() != (width, height) {
            let region = CreateRoundRectRgn(1, 1, width, height, height - 2, height - 2);
            if region.is_invalid() {
                return Err(Error::from_thread());
            }
            if SetWindowRgn(self.window, Some(region), false) == 0 {
                let _ = DeleteObject(region.into());
                return Err(Error::from_thread());
            }
            self.size.set((width, height));
        }
        // Windows owns the region after SetWindowRgn. The owned content popup
        // always stays above this material window, including after resizing.
        SetWindowPos(
            self.window,
            Some(content),
            bounds.left,
            bounds.top,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )?;
        Ok(())
    }
    pub unsafe fn hide(&self) {
        let _ = ShowWindow(self.window, SW_HIDE);
    }
}
impl Drop for Backdrop {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
            let _ = UnregisterClassW(
                w!("ShanjieRustCandidateBackdrop"),
                Some(HINSTANCE(crate::tsf::module().0)),
            );
        }
    }
}
unsafe extern "system" fn procedure(window: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match message {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let dc = BeginPaint(window, &mut paint);
            let rect = paint.rcPaint;
            let _ = PatBlt(
                dc,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                BLACKNESS,
            );
            let _ = EndPaint(window, &paint);
            LRESULT(0)
        }
        _ => DefWindowProcW(window, message, wp, lp),
    }
}
