use crate::engine::Snapshot;
use windows::{
    core::{w, Result},
    Win32::{
        Foundation::*,
        Graphics::{
            Direct2D::Common::*, Direct2D::*, DirectWrite::*, Dwm::*, Dxgi::Common::*, Gdi::*,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{Accessibility::*, HiDpi::*, WindowsAndMessaging::*},
    },
    UI::ViewManagement::{UIColorType, UISettings},
};
use windows_numerics::Vector2;

fn rgb(value: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((value >> 16) & 255) as f32 / 255.0,
        g: ((value >> 8) & 255) as f32 / 255.0,
        b: (value & 255) as f32 / 255.0,
        a: 1.0,
    }
}
struct Palette {
    background: D2D1_COLOR_F,
    border: D2D1_COLOR_F,
    text: D2D1_COLOR_F,
    label: D2D1_COLOR_F,
    accent: D2D1_COLOR_F,
    selected: D2D1_COLOR_F,
}
impl Palette {
    fn new(theme: u32) -> Result<Self> {
        unsafe {
            let mut contrast = HIGHCONTRASTW {
                cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
                ..Default::default()
            };
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                contrast.cbSize,
                Some((&mut contrast as *mut HIGHCONTRASTW).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )?;
            if contrast.dwFlags & HCF_HIGHCONTRASTON != HIGHCONTRASTW_FLAGS(0) {
                let system = |role| {
                    let c = GetSysColor(role);
                    rgb(((c & 255) << 16) | (c & 0xff00) | ((c >> 16) & 255))
                };
                return Ok(Self {
                    background: system(COLOR_WINDOW),
                    border: system(COLOR_WINDOWTEXT),
                    text: system(COLOR_WINDOWTEXT),
                    label: system(COLOR_WINDOWTEXT),
                    accent: system(COLOR_HIGHLIGHT),
                    selected: system(COLOR_HIGHLIGHTTEXT),
                });
            }
            let dark = theme == 2
                || (theme == 0
                    && UISettings::new()?.GetColorValue(UIColorType::Foreground)?.R > 127);
            Ok(Self {
                background: rgb(if dark { 0x343443 } else { 0xf2f2f5 }),
                border: rgb(if dark { 0x787884 } else { 0xb5b5bd }),
                text: rgb(if dark { 0xf5f5f7 } else { 0x222225 }),
                label: rgb(if dark { 0xb9b9c5 } else { 0x65656e }),
                accent: rgb(0x007aff),
                selected: rgb(0xffffff),
            })
        }
    }
}
struct Item {
    word: IDWriteTextLayout,
    label: IDWriteTextLayout,
    width: f32,
    tag: f32,
    word_height: f32,
    label_height: f32,
}
pub struct Host {
    pub window: HWND,
    callback: HWND,
    pub sequence: i32,
    state: Snapshot,
    theme: u32,
    size: u32,
    scale: f32,
    row: f32,
    _factory: ID2D1Factory,
    text_factory: IDWriteFactory,
    target: ID2D1DCRenderTarget,
    brush: ID2D1SolidColorBrush,
    palette: Palette,
    items: Vec<Item>,
}
impl Host {
    pub fn start(callback: HWND) -> Result<Box<Self>> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let text_factory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                ..Default::default()
            };
            let target = factory.CreateDCRenderTarget(&properties)?;
            let brush = target.CreateSolidColorBrush(&rgb(0), None)?;
            let mut host = Box::new(Self {
                window: HWND::default(),
                callback,
                sequence: 0,
                state: Snapshot::default(),
                theme: 0,
                size: 18,
                scale: 1.0,
                row: 28.0,
                _factory: factory,
                text_factory,
                target,
                brush,
                palette: Palette::new(0)?,
                items: Vec::new(),
            });
            let module = GetModuleHandleW(None)?;
            let wc = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: HINSTANCE(module.0),
                lpszClassName: w!("ShanjieRustCandidates"),
                ..Default::default()
            };
            RegisterClassW(&wc);
            let awareness =
                SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let created = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                wc.lpszClassName,
                w!("善解候選"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(HINSTANCE(module.0)),
                Some((&mut *host as *mut Self).cast()),
            );
            SetThreadDpiAwarenessContext(awareness);
            host.window = created?;
            let disabled = 1i32;
            DwmSetWindowAttribute(
                host.window,
                DWMWA_TRANSITIONS_FORCEDISABLED,
                (&disabled as *const i32).cast(),
                4,
            )?;
            Ok(host)
        }
    }
    pub fn hide(&mut self) {
        self.sequence = self.sequence.wrapping_add(1);
        unsafe {
            let _ = ShowWindow(self.window, SW_HIDE);
        }
    }
    pub fn shown(&self) -> bool {
        unsafe { IsWindowVisible(self.window).as_bool() }
    }
    fn measure(&mut self, work_width: f32) -> Result<f32> {
        unsafe {
            let text = self.text_factory.CreateTextFormat(
                w!("Microsoft JhengHei UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                self.size as f32,
                w!("zh-TW"),
            )?;
            let label = self.text_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                (self.size as f32 * 0.6).max(11.0),
                w!("zh-TW"),
            )?;
            text.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            label.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let ellipsis = self.text_factory.CreateEllipsisTrimmingSign(&text)?;
            self.items.clear();
            self.row = 26.0;
            for (i, value) in self.state.candidates.iter().enumerate() {
                let tag = if self.state.selected.is_none() {
                    String::new()
                } else {
                    format!("{}{}", if self.state.associated { "⇧" } else { "" }, i + 1)
                };
                let word = self.text_factory.CreateTextLayout(
                    &value.encode_utf16().collect::<Vec<_>>(),
                    &text,
                    10000.0,
                    1000.0,
                )?;
                let number = self.text_factory.CreateTextLayout(
                    &tag.encode_utf16().collect::<Vec<_>>(),
                    &label,
                    1000.0,
                    1000.0,
                )?;
                let mut wm = DWRITE_TEXT_METRICS::default();
                let mut nm = DWRITE_TEXT_METRICS::default();
                word.GetMetrics(&mut wm)?;
                number.GetMetrics(&mut nm)?;
                let tag_width = if tag.is_empty() {
                    0.0
                } else {
                    nm.widthIncludingTrailingWhitespace + 6.0
                };
                self.row = self.row.max(wm.height + 6.0);
                self.items.push(Item {
                    word,
                    label: number,
                    width: wm.widthIncludingTrailingWhitespace + tag_width + 20.0,
                    tag: tag_width,
                    word_height: wm.height,
                    label_height: nm.height,
                });
            }
            let available = (work_width - 8.0).max(1.0);
            let natural: f32 = self.items.iter().map(|i| i.width).sum();
            let ratio = (available / natural).min(1.0);
            for item in &mut self.items {
                item.width = (item.width * ratio * self.scale).floor() / self.scale;
                item.word
                    .SetMaxWidth((item.width - item.tag - 20.0).max(1.0))?;
                item.word.SetTrimming(
                    &DWRITE_TRIMMING {
                        granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                        ..Default::default()
                    },
                    &ellipsis,
                )?;
            }
            Ok(self.items.iter().map(|i| i.width).sum::<f32>() + 8.0)
        }
    }
    pub fn publish(
        &mut self,
        state: Snapshot,
        theme: u32,
        size: u32,
        caret: RECT,
        visible: bool,
    ) -> Result<()> {
        unsafe {
            self.sequence = self.sequence.wrapping_add(1);
            if !visible || state.candidates.is_empty() {
                self.hide();
                return Ok(());
            }
            self.state = state;
            self.theme = theme;
            self.size = size;
            self.palette = Palette::new(theme)?;
            let mut monitor = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            GetMonitorInfoW(
                MonitorFromRect(&caret, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            )
            .ok()?;
            if let Ok(width) = std::env::var("SHANJIE_VERIFY_WORK_WIDTH") {
                monitor.rcWork.right = monitor.rcWork.left + width.parse::<i32>().unwrap();
            }
            let awareness =
                SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let result = (|| -> Result<()> {
                SetWindowPos(
                    self.window,
                    None,
                    caret.left,
                    caret.bottom,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
                )?;
                self.scale = GetDpiForWindow(self.window) as f32 / 96.0;
                let width = (self
                    .measure((monitor.rcWork.right - monitor.rcWork.left) as f32 / self.scale)?
                    * self.scale)
                    .ceil() as i32;
                let height = ((self.row + 4.0) * self.scale).ceil() as i32;
                let mut x = caret.left.clamp(
                    monitor.rcWork.left,
                    (monitor.rcWork.right - width).max(monitor.rcWork.left),
                );
                let gap = (6.0 * self.scale).round() as i32;
                let mut y = caret.bottom + gap;
                if y + height > monitor.rcWork.bottom {
                    y = caret.top - height - gap;
                }
                y = y.max(monitor.rcWork.top);
                if std::env::var_os("SHANJIE_VERIFY_UI").is_some() {
                    x = -12000;
                    y = -12000;
                }
                let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, height, height);
                SetWindowRgn(self.window, Some(region), false);
                SetWindowPos(
                    self.window,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )?;
                let _ = InvalidateRect(Some(self.window), None, false);
                let _ = UpdateWindow(self.window);
                Ok(())
            })();
            SetThreadDpiAwarenessContext(awareness);
            result
        }
    }
    unsafe fn paint(&self, dc: HDC) -> Result<()> {
        let mut bounds = RECT::default();
        GetClientRect(self.window, &mut bounds)?;
        self.target.BindDC(dc, &bounds)?;
        self.target.SetDpi(96.0 * self.scale, 96.0 * self.scale);
        self.target.BeginDraw();
        self.target.Clear(Some(&self.palette.background));
        let width = bounds.right as f32 / self.scale;
        let height = bounds.bottom as f32 / self.scale;
        let rounded = |l, t, r, b, radius| D2D1_ROUNDED_RECT {
            rect: D2D_RECT_F {
                left: l,
                top: t,
                right: r,
                bottom: b,
            },
            radiusX: radius,
            radiusY: radius,
        };
        self.brush.SetColor(&self.palette.border);
        self.target.DrawRoundedRectangle(
            &rounded(0.5, 0.5, width - 0.5, height - 0.5, (height - 1.0) / 2.0),
            &self.brush,
            1.0,
            None,
        );
        let mut x = 4.0;
        for (i, item) in self.items.iter().enumerate() {
            let selected = self.state.selected == Some(i);
            if selected {
                self.brush.SetColor(&self.palette.accent);
                self.target.FillRoundedRectangle(
                    &rounded(x, 2.0, x + item.width, 2.0 + self.row, self.row / 2.0),
                    &self.brush,
                );
            }
            self.target.PushAxisAlignedClip(
                &D2D_RECT_F {
                    left: x,
                    top: 2.0,
                    right: x + item.width,
                    bottom: 2.0 + self.row,
                },
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            );
            self.brush.SetColor(if selected {
                &self.palette.selected
            } else {
                &self.palette.label
            });
            self.target.DrawTextLayout(
                Vector2 {
                    X: x + 10.0,
                    Y: 2.0 + (self.row - item.label_height) / 2.0,
                },
                &item.label,
                &self.brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
            self.brush.SetColor(if selected {
                &self.palette.selected
            } else {
                &self.palette.text
            });
            self.target.DrawTextLayout(
                Vector2 {
                    X: x + 10.0 + item.tag,
                    Y: 2.0 + (self.row - item.word_height) / 2.0,
                },
                &item.word,
                &self.brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            );
            self.target.PopAxisAlignedClip();
            x += item.width;
        }
        self.target.EndDraw(None, None)
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
        }
    }
}
unsafe extern "system" fn procedure(window: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let mut ptr = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut Host;
    if message == WM_NCCREATE {
        ptr = (*(lp.0 as *const CREATESTRUCTW)).lpCreateParams.cast();
        SetWindowLongPtrW(window, GWLP_USERDATA, ptr as isize);
    }
    if message == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    if message == WM_ERASEBKGND {
        return LRESULT(1);
    }
    if !ptr.is_null() {
        let host = &mut *ptr;
        if message == WM_PAINT || message == WM_PRINTCLIENT {
            let mut ps = PAINTSTRUCT::default();
            let dc = if message == WM_PAINT {
                BeginPaint(window, &mut ps)
            } else {
                HDC(wp.0 as _)
            };
            let _ = host.paint(dc);
            if message == WM_PAINT {
                let _ = EndPaint(window, &ps);
            }
            return LRESULT(0);
        }
        if message == WM_LBUTTONUP {
            let x = (lp.0 as u16 as i16) as f32 / host.scale;
            let y = ((lp.0 >> 16) as u16 as i16) as f32 / host.scale;
            let mut left = 4.0;
            for (i, item) in host.items.iter().enumerate() {
                if x >= left && x < left + item.width && y >= 2.0 && y < 2.0 + host.row {
                    let _ = PostMessageW(
                        Some(host.callback),
                        WM_APP + 42,
                        WPARAM(i),
                        LPARAM(host.sequence as isize),
                    );
                    break;
                }
                left += item.width;
            }
            return LRESULT(0);
        }
        if [
            WM_SETTINGCHANGE,
            WM_THEMECHANGED,
            WM_DWMCOLORIZATIONCOLORCHANGED,
        ]
        .contains(&message)
        {
            if let Ok(p) = Palette::new(host.theme) {
                host.palette = p;
            }
            let _ = InvalidateRect(Some(window), None, false);
            return LRESULT(0);
        }
    }
    DefWindowProcW(window, message, wp, lp)
}
