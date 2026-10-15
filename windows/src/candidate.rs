use crate::engine::Snapshot;
use windows::{
    core::{w, Result},
    Win32::{
        Foundation::*,
        Graphics::{
            Direct2D::Common::*, Direct2D::*, DirectWrite::*, Dwm::*, Dxgi::Common::*, Gdi::*,
        },
        System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
        UI::{Accessibility::*, HiDpi::*, WindowsAndMessaging::*},
    },
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
            let mut light = 1u32;
            if theme == 0 {
                let mut size = 4;
                // An absent personalization value is Windows' default light appearance.
                let _ = RegGetValueW(HKEY_CURRENT_USER,
                    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                    w!("AppsUseLightTheme"), RRF_RT_REG_DWORD, None,
                    Some((&mut light as *mut u32).cast()), Some(&mut size));
            }
            if theme == 2 || (theme == 0 && light == 0) {
                return Ok(Self { background: rgb(0x202020), border: rgb(0x505055),
                    text: rgb(0xf5f5f7), label: rgb(0xb8b8c2), accent: rgb(0x006ee6), selected: rgb(0xffffff) });
            }
            Ok(Self {
                background: rgb(0xffffff),
                border: rgb(0xd3d3d8),
                text: rgb(0x222225),
                label: rgb(0x65656e),
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
    reading: Option<IDWriteTextLayout>,
    reading_width: f32,
    reading_height: f32,
    start: usize,
    end: usize,
    work_width: f32,
    /// Microsoft IME style: one candidate per row instead of the horizontal pill.
    pub vertical: bool,
    /// The candidate target is the character right of the cursor (profile `anchor=right`).
    pub anchor_right: bool,
}
impl Host {
    pub fn start(callback: HWND, owner: Option<HWND>) -> Result<Box<Self>> {
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
                reading: None,
                reading_width: 0.0,
                reading_height: 0.0,
                start: 0,
                end: 0,
                work_width: 0.0,
                vertical: false,
                anchor_right: true,
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
                owner,
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
                (self.size as f32 * 9.0 / 16.0).max(8.0),
                w!("zh-TW"),
            )?;
            text.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            label.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let ellipsis = self.text_factory.CreateEllipsisTrimmingSign(&text)?;
            self.items.clear();
            let ratio = self.size as f32 / 16.0;
            self.row = 24.0 * ratio;
            let candidates: &[String] = if self.state.reading_hint { &[] } else { &self.state.candidates };
            for (i, value) in candidates.iter().enumerate() {
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
            // The row shows the whole sentence with the character being chosen (or the Zhuyin being typed,
            // swapped in for its Latin keys) in the accent colour.
            let units: Vec<u16> = self.state.preedit.encode_utf16().collect();
            let cursor = (self.state.cursor as usize).min(units.len());
            let mut mark: Option<(usize, usize)> = None;
            let mut text = units.clone();
            if self.state.reading_hint {
                let hint: Vec<u16> = self.state.candidates.first().map(|h| h.encode_utf16().collect()).unwrap_or_default();
                let raw: Vec<u16> = self.state.candidates.get(1).map(|h| h.encode_utf16().collect()).unwrap_or_default();
                let end = (raw.len()..=cursor).rev().find(|&e| units[e - raw.len()..e] == raw[..]);
                if let (Some(end), false) = (end, hint.is_empty()) {
                    let start = end - raw.len();
                    text.splice(start..end, hint.iter().copied());
                    mark = Some((start, hint.len()));
                } else {
                    text = hint.clone();
                    mark = Some((0, hint.len()));
                }
            } else if self.anchor_right && cursor < units.len() {
                // Right anchor: the character after the cursor is the one being chosen.
                let len = if (0xd800..0xdc00).contains(&units[cursor]) { 2 } else { 1 };
                mark = Some((cursor, len.min(units.len() - cursor)));
            } else if !self.anchor_right && !units.is_empty() && cursor < units.len() {
                let mut start = cursor.saturating_sub(1);
                if start > 0 && (0xdc00..0xe000).contains(&units[start]) {
                    start -= 1;
                }
                let len = if (0xd800..0xdc00).contains(&units[start]) { 2 } else { 1 };
                mark = Some((start, len.min(units.len() - start)));
            }
            let reading = String::from_utf16_lossy(&text);
            self.reading = None;
            self.reading_width = 0.0;
            self.reading_height = 0.0;
            if !reading.is_empty() {
                let format = self.text_factory.CreateTextFormat(
                    w!("Microsoft JhengHei UI"),
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    (self.size as f32 * 0.8).max(9.0),
                    w!("zh-TW"),
                )?;
                format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                let layout = self.text_factory.CreateTextLayout(
                    &text,
                    &format,
                    10000.0,
                    1000.0,
                )?;
                let mut m = DWRITE_TEXT_METRICS::default();
                layout.GetMetrics(&mut m)?;
                self.reading_width = m
                    .widthIncludingTrailingWhitespace
                    .min((work_width - 28.0 * ratio).max(1.0));
                self.reading_height = m.height + 6.0 * ratio;
                layout.SetMaxWidth(self.reading_width.max(1.0))?;
                layout.SetTrimming(
                    &DWRITE_TRIMMING {
                        granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                        ..Default::default()
                    },
                    &self.text_factory.CreateEllipsisTrimmingSign(&format)?,
                )?;
                if let Some((start, length)) = mark.filter(|&(_, n)| n > 0) {
                    let blue = self.target.CreateSolidColorBrush(&self.palette.accent, None)?;
                    let _ = layout.SetDrawingEffect(
                        &blue,
                        DWRITE_TEXT_RANGE { startPosition: start as u32, length: length as u32 },
                    );
                }
                self.reading = Some(layout);
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
            Ok(width.max(self.reading_width + 28.0 * ratio).min(work_width))
        }
    }
    fn top(&self) -> f32 {
        3.0 * self.size as f32 / 16.0 + self.reading_height
    }
    fn row_top(&self, index: usize) -> f32 {
        if self.vertical {
            self.top() + (index - self.start) as f32 * (self.row + 2.0 * self.size as f32 / 16.0)
        } else {
            self.top()
        }
    }
    fn height(&self) -> f32 {
        let lines = if self.vertical { self.end - self.start } else { 1 };
        let rows = if self.items.is_empty() {
            0.0
        } else {
            lines as f32 * self.row + lines.saturating_sub(1) as f32 * 2.0 * self.size as f32 / 16.0
        };
        self.top() + rows + 3.0 * self.size as f32 / 16.0
    }
    fn fit_view(&mut self, selected: Option<usize>) -> f32 {
        let ratio = self.size as f32 / 16.0;
        let count = self.items.len();
        if self.vertical {
            // A page holds at most nine candidates, so every row is shown; rows share one width.
            self.start = 0;
            self.end = count;
            let widest = self.items.iter().map(|i| i.natural).fold(0.0, f32::max);
            let width = widest.min((self.work_width - 6.0 * ratio).max(1.0));
            for item in &mut self.items {
                item.width = width;
            }
            return (width + 6.0 * ratio).min(self.work_width);
        }
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
        if self.items.is_empty() {
            return Ok(());
        }
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
        let ratio = self.size as f32 / 16.0;
        let width = self
            .fit_view(None)
            .max(self.reading_width + 28.0 * ratio)
            .min(self.work_width);
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
            (self.height() * self.scale).ceil() as i32,
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
            if !visible || (state.candidates.is_empty() && state.preedit.is_empty()) {
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
                let height = (self.height() * self.scale).ceil() as i32;
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
        let ratio = self.size as f32 / 16.0;
        // Large corners: one row stays a full pill; two rows keep the same generous radius.
        let radius = if self.vertical {
            12.0 * ratio
        } else {
            (height / 2.0).min(20.0 * ratio)
        };
        self.brush.SetColor(&self.palette.background);
        self.target
            .FillRoundedRectangle(&rounded(0.0, 0.0, width, height, radius), &self.brush);
        self.brush.SetColor(&self.palette.border);
        self.target.DrawRoundedRectangle(
            &rounded(
                0.25,
                0.25,
                width - 0.25,
                height - 0.25,
                radius - 0.25,
            ),
            &self.brush,
            0.5,
            None,
        );
        if let Some(reading) = &self.reading {
            self.brush.SetColor(&self.palette.label);
            self.target.DrawTextLayout(
                Vector2 {
                    X: 14.0 * ratio,
                    Y: 6.0 * ratio,
                },
                reading,
                &self.brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            );
        }
        let mut x = (3.0 + if self.start > 0 { 14.0 } else { 0.0 }) * ratio;
        for (i, item) in self
            .items
            .iter()
            .enumerate()
            .take(self.end)
            .skip(self.start)
        {
            let selected = self.state.selected == Some(i);
            let y = self.row_top(i);
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
            if !self.vertical {
                x += item.width + 2.0 * ratio;
            }
        }
        self.brush.SetColor(&self.palette.text);
        for (present, center, direction) in [
            (self.start > 0, 10.0 * ratio, -1.0),
            (self.end < self.items.len(), width - 10.0 * ratio, 1.0),
        ] {
            if present {
                let middle = self.top() + self.row / 2.0;
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
            if host.state.reading_hint { return LRESULT(0); }
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
                let top = host.row_top(i);
                if x >= left
                    && x < left + item.width
                    && y >= top
                    && y < top + host.row
                {
                    let _ = PostMessageW(
                        Some(host.callback),
                        WM_APP + 42,
                        WPARAM(i),
                        LPARAM(host.sequence as isize),
                    );
                    break;
                }
                if !host.vertical {
                    left += item.width + 2.0 * ratio;
                }
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
