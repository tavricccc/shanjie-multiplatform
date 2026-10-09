use crate::engine::Snapshot;
use windows::{
    core::{w, Result},
    Win32::{
        Foundation::*,
        Graphics::{
            Direct2D::Common::*, Direct2D::*, DirectWrite::*, Dwm::*, Dxgi::Common::*, Gdi::*,
        },
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
    glass: bool,
    tint: u32,
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
                    glass: false,
                    tint: 0,
                    text: system(COLOR_WINDOWTEXT),
                    label: system(COLOR_WINDOWTEXT),
                    accent: system(COLOR_HIGHLIGHT),
                    selected: system(COLOR_HIGHLIGHTTEXT),
                });
            }
            let settings = UISettings::new()?;
            let glass = settings.AdvancedEffectsEnabled()?;
            let dark = theme == 2
                || (theme == 0 && settings.GetColorValue(UIColorType::Foreground)?.R > 127);
            let tint = if dark { 0x343443 } else { 0xffffff };
            let mut background = rgb(tint);
            background.a = if glass { 0.82 } else { 1.0 };
            Ok(Self {
                background,
                glass,
                tint,
                text: rgb(if dark { 0xf5f5f7 } else { 0x222225 }),
                label: rgb(if dark { 0xb9b9c5 } else { 0x65656e }),
                accent: rgb(0x006ee6),
                selected: rgb(0xffffff),
            })
        }
    }
}
struct Item {
    word: IDWriteTextLayout,
    label: IDWriteTextLayout,
    width: f32,
    natural: f32,
    tag: f32,
    word_height: f32,
    label_height: f32,
}
pub struct Host {
    pub window: HWND,
    backdrop: crate::backdrop::Backdrop,
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
    start: usize,
    end: usize,
    work_width: f32,
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
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                ..Default::default()
            };
            let target = factory.CreateDCRenderTarget(&properties)?;
            let brush = target.CreateSolidColorBrush(&rgb(0), None)?;
            let mut host = Box::new(Self {
                window: HWND::default(),
                backdrop: crate::backdrop::Backdrop::new()?,
                callback,
                sequence: 0,
                state: Snapshot::default(),
                theme: 0,
                size: 16,
                scale: 1.0,
                row: 28.0,
                _factory: factory,
                text_factory,
                target,
                brush,
                palette: Palette::new(0)?,
                items: Vec::new(),
                start: 0,
                end: 0,
                work_width: 0.0,
            });
            let module = crate::tsf::module();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: HINSTANCE(module.0),
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                lpszClassName: w!("ShanjieRustCandidates"),
                ..Default::default()
            };
            RegisterClassW(&wc);
            let awareness =
                SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let created = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                wc.lpszClassName,
                w!("善解候選"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                Some(host.backdrop.window()),
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
            self.backdrop.hide();
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
                (self.size as f32 * 9.0 / 16.0).max(8.0),
                w!("zh-TW"),
            )?;
            text.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            label.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let ellipsis = self.text_factory.CreateEllipsisTrimmingSign(&text)?;
            self.items.clear();
            let ratio = self.size as f32 / 16.0;
            self.row = 24.0 * ratio;
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
                    nm.widthIncludingTrailingWhitespace + 6.0 * ratio
                };
                self.row = self.row.max(wm.height + 2.0 * ratio);
                self.items.push(Item {
                    word,
                    label: number,
                    width: wm.widthIncludingTrailingWhitespace + tag_width + 11.0 * ratio,
                    natural: wm.widthIncludingTrailingWhitespace + tag_width + 11.0 * ratio,
                    tag: tag_width,
                    word_height: wm.height,
                    label_height: nm.height,
                });
            }
            self.work_width = work_width;
            let width = self.fit_view(self.state.selected);
            for item in &mut self.items {
                item.word
                    .SetMaxWidth((item.width - item.tag - 11.0 * ratio).max(1.0))?;
                item.word.SetTrimming(
                    &DWRITE_TRIMMING {
                        granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                        ..Default::default()
                    },
                    &ellipsis,
                )?;
            }
            Ok(width)
        }
    }
    fn fit_view(&mut self, selected: Option<usize>) -> f32 {
        let ratio = self.size as f32 / 16.0;
        let count = self.items.len();
        self.start = self.start.min(count.saturating_sub(1));
        if let Some(selected) = selected {
            if selected < self.start {
                self.start = selected;
            }
        }
        loop {
            for item in &mut self.items {
                item.width = item.natural;
            }
            let left = if self.start > 0 { 14.0 * ratio } else { 0.0 };
            let natural = self.items[self.start..]
                .iter()
                .map(|i| i.natural)
                .sum::<f32>()
                + 2.0 * ratio * (count - self.start).saturating_sub(1) as f32;
            let right = if natural + left + 6.0 * ratio > self.work_width {
                14.0 * ratio
            } else {
                0.0
            };
            let available = (self.work_width - left - right - 6.0 * ratio).max(1.0);
            let mut used = 0.0;
            self.end = self.start;
            for i in self.start..count {
                let gap = if i == self.start { 0.0 } else { 2.0 * ratio };
                let width = self.items[i].natural.min(available);
                if self.end > self.start && used + gap + width > available {
                    break;
                }
                self.items[i].width = width;
                used += gap + width;
                self.end = i + 1;
            }
            if selected.is_some_and(|i| i >= self.end) && self.start + 1 < count {
                self.start += 1;
                continue;
            }
            return (used + left + if self.end < count { 14.0 * ratio } else { 0.0 } + 6.0 * ratio)
                .min(self.work_width);
        }
    }
    unsafe fn scroll(&mut self, next: bool) -> Result<()> {
        let start = if next {
            self.end.min(self.items.len() - 1)
        } else {
            self.start.saturating_sub((self.end - self.start).max(1))
        };
        if start == self.start {
            return Ok(());
        }
        self.start = start;
        self.sequence = self.sequence.wrapping_add(1);
        let width = self.fit_view(None);
        let ratio = self.size as f32 / 16.0;
        for item in &self.items {
            item.word
                .SetMaxWidth((item.width - item.tag - 11.0 * ratio).max(1.0))?;
        }
        let mut rect = RECT::default();
        GetWindowRect(self.window, &mut rect)?;
        let mut monitor = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        GetMonitorInfoW(
            MonitorFromWindow(self.window, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        )
        .ok()?;
        let pixels = (width * self.scale).ceil() as i32;
        let x = if std::env::var_os("SHANJIE_VERIFY_UI").is_some() {
            rect.left
        } else {
            rect.left.clamp(
                monitor.rcWork.left,
                (monitor.rcWork.right - pixels).max(monitor.rcWork.left),
            )
        };
        SetWindowPos(
            self.window,
            None,
            x,
            rect.top,
            pixels,
            ((self.row + 6.0 * self.size as f32 / 16.0) * self.scale).ceil() as i32,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )?;
        self.display()
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
            if self.state.candidates != state.candidates || self.state.first != state.first {
                self.start = 0;
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
                let height =
                    ((self.row + 6.0 * self.size as f32 / 16.0) * self.scale).ceil() as i32;
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
                SetWindowPos(
                    self.window,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )?;
                self.display()?;
                Ok(())
            })();
            SetThreadDpiAwarenessContext(awareness);
            result
        }
    }
    unsafe fn display(&self) -> Result<()> {
        let mut rect = RECT::default();
        GetWindowRect(self.window, &mut rect)?;
        self.backdrop
            .show(self.window, rect, self.palette.glass, self.palette.tint)?;
        let buffer = Buffer::new(rect.right - rect.left, rect.bottom - rect.top)?;
        self.paint(buffer.dc)?;
        let position = POINT {
            x: rect.left,
            y: rect.top,
        };
        let size = SIZE {
            cx: rect.right - rect.left,
            cy: rect.bottom - rect.top,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        UpdateLayeredWindow(
            self.window,
            None,
            Some(&position),
            Some(&size),
            Some(buffer.dc),
            Some(&POINT::default()),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        )?;
        let _ = ValidateRect(Some(self.window), None);
        Ok(())
    }
    unsafe fn paint(&self, dc: HDC) -> Result<()> {
        let mut bounds = RECT::default();
        GetClientRect(self.window, &mut bounds)?;
        self.target.BindDC(dc, &bounds)?;
        self.target.SetDpi(96.0 * self.scale, 96.0 * self.scale);
        self.target.BeginDraw();
        self.target.Clear(Some(&D2D1_COLOR_F::default()));
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
        self.brush.SetColor(&self.palette.background);
        self.target
            .FillRoundedRectangle(&rounded(0.0, 0.0, width, height, height / 2.0), &self.brush);
        let ratio = self.size as f32 / 16.0;
        let mut x = (3.0 + if self.start > 0 { 14.0 } else { 0.0 }) * ratio;
        let y = 3.0 * ratio;
        for (i, item) in self
            .items
            .iter()
            .enumerate()
            .take(self.end)
            .skip(self.start)
        {
            let selected = self.state.selected == Some(i);
            if selected {
                self.brush.SetColor(&self.palette.accent);
                self.target.FillRoundedRectangle(
                    &rounded(x, y, x + item.width, y + self.row, self.row / 2.0),
                    &self.brush,
                );
            }
            self.target.PushAxisAlignedClip(
                &D2D_RECT_F {
                    left: x,
                    top: y,
                    right: x + item.width,
                    bottom: y + self.row,
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
                    X: x + 4.0 * ratio,
                    Y: y + (self.row - item.label_height) / 2.0,
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
                    X: x + 4.0 * ratio + item.tag,
                    Y: y + (self.row - item.word_height) / 2.0,
                },
                &item.word,
                &self.brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            );
            self.target.PopAxisAlignedClip();
            x += item.width + 2.0 * ratio;
        }
        self.brush.SetColor(&self.palette.text);
        for (present, center, direction) in [
            (self.start > 0, 10.0 * ratio, -1.0),
            (self.end < self.items.len(), width - 10.0 * ratio, 1.0),
        ] {
            if present {
                let middle = height / 2.0;
                self.target.DrawLine(
                    Vector2 {
                        X: center - direction * 2.0 * ratio,
                        Y: middle - 4.0 * ratio,
                    },
                    Vector2 {
                        X: center + direction * 2.0 * ratio,
                        Y: middle,
                    },
                    &self.brush,
                    1.5 * ratio,
                    None,
                );
                self.target.DrawLine(
                    Vector2 {
                        X: center + direction * 2.0 * ratio,
                        Y: middle,
                    },
                    Vector2 {
                        X: center - direction * 2.0 * ratio,
                        Y: middle + 4.0 * ratio,
                    },
                    &self.brush,
                    1.5 * ratio,
                    None,
                );
            }
        }
        self.target.EndDraw(None, None)
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
            let _ = UnregisterClassW(
                w!("ShanjieRustCandidates"),
                Some(HINSTANCE(crate::tsf::module().0)),
            );
        }
    }
}
unsafe extern "system" fn procedure(window: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let mut ptr = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut Host;
    if message == WM_NCCREATE {
        ptr = (*(lp.0 as *const CREATESTRUCTW)).lpCreateParams.cast();
        SetWindowLongPtrW(window, GWLP_USERDATA, ptr as _);
    }
    if message == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    if message == WM_ERASEBKGND {
        return LRESULT(1);
    }
    if message == WM_PAINT {
        let mut ps = PAINTSTRUCT::default();
        BeginPaint(window, &mut ps);
        let _ = EndPaint(window, &ps);
        return LRESULT(0);
    }
    if !ptr.is_null() {
        if message == WM_PRINTCLIENT {
            let _ = (*ptr).paint(HDC(wp.0 as _));
            return LRESULT(0);
        }
        if message == WM_LBUTTONUP {
            let host = &mut *ptr;
            let x = (lp.0 as u16 as i16) as f32 / host.scale;
            let y = ((lp.0 >> 16) as u16 as i16) as f32 / host.scale;
            let ratio = host.size as f32 / 16.0;
            let mut bounds = RECT::default();
            let _ = GetClientRect(window, &mut bounds);
            if host.start > 0 && x < 17.0 * ratio {
                let _ = host.scroll(false);
                return LRESULT(0);
            }
            if host.end < host.items.len() && x > bounds.right as f32 / host.scale - 17.0 * ratio {
                let _ = host.scroll(true);
                return LRESULT(0);
            }
            let mut left = (3.0 + if host.start > 0 { 14.0 } else { 0.0 }) * ratio;
            for (i, item) in host
                .items
                .iter()
                .enumerate()
                .take(host.end)
                .skip(host.start)
            {
                if x >= left
                    && x < left + item.width
                    && y >= 3.0 * ratio
                    && y < 3.0 * ratio + host.row
                {
                    let _ = PostMessageW(
                        Some(host.callback),
                        WM_APP + 42,
                        WPARAM(i),
                        LPARAM(host.sequence as isize),
                    );
                    break;
                }
                left += item.width + 2.0 * ratio;
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
            let host = &mut *ptr;
            if let Ok(p) = Palette::new(host.theme) {
                host.palette = p;
            }
            if host.shown() {
                let _ = host.display();
            }
            return LRESULT(0);
        }
    }
    DefWindowProcW(window, message, wp, lp)
}
struct Buffer {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
}
impl Buffer {
    unsafe fn new(width: i32, height: i32) -> Result<Self> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = std::ptr::null_mut();
        let bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut pixels, None, 0)?;
        let dc = CreateCompatibleDC(None);
        let old = SelectObject(dc, bitmap.into());
        Ok(Self { dc, bitmap, old })
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteDC(self.dc);
            let _ = DeleteObject(self.bitmap.into());
        }
    }
}
