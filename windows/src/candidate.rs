use crate::{engine::Snapshot, glass::tokens::{FAST_MS, GLIDE, NORMAL_MS, SMOOTH, SPRING}};
use std::time::Instant;
use windows::{
    core::{w, Result, BOOL},
    Win32::{
        Foundation::*,
        Graphics::{
            Direct2D::Common::*, Direct2D::*, DirectWrite::*, Dwm::*, Dxgi::Common::*, Gdi::*,
        },
        System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
        UI::{Accessibility::*, HiDpi::*, WindowsAndMessaging::*},
    },
};
use windows_numerics::{Matrix3x2, Vector2};

fn rgb(value: u32) -> D2D1_COLOR_F {
    rgba(value, 1.0)
}
fn rgba(value: u32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((value >> 16) & 255) as f32 / 255.0,
        g: ((value >> 8) & 255) as f32 / 255.0,
        b: (value & 255) as f32 / 255.0,
        a,
    }
}
/// Pane's colors: a near-opaque glass surface (the candidate window lives in other apps' processes, so it has no
/// system blur), a hairline outline with a light top edge, a soft shadow and the 霧藍 accent.
pub(crate) struct Palette {
    pub(crate) background: D2D1_COLOR_F,
    pub(crate) border: D2D1_COLOR_F,
    pub(crate) text: D2D1_COLOR_F,
    label: D2D1_COLOR_F,
    number: D2D1_COLOR_F,
    accent: D2D1_COLOR_F,
    selected: D2D1_COLOR_F,
    edge: D2D1_COLOR_F,
    mark: D2D1_COLOR_F,
    divider: D2D1_COLOR_F,
    shadow: f32,
}
impl Palette {
    pub(crate) fn new(theme: u32) -> Result<Self> {
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
                    number: system(COLOR_WINDOWTEXT),
                    accent: system(COLOR_HIGHLIGHT),
                    selected: system(COLOR_HIGHLIGHTTEXT),
                    edge: rgba(0, 0.0),
                    mark: rgba(0, 0.0),
                    divider: system(COLOR_WINDOWTEXT),
                    shadow: 0.0,
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
                return Ok(Self {
                    background: rgba(0x1c1c20, 0.97),
                    border: rgba(0xffffff, 0.08),
                    text: rgba(0xffffff, 0.94),
                    label: rgba(0xffffff, 0.64),
                    number: rgba(0xffffff, 0.4),
                    accent: rgb(0x5874c9),
                    selected: rgb(0xffffff),
                    edge: rgba(0xffffff, 0.12),
                    mark: rgba(0x5874c9, 0.3),
                    divider: rgba(0xffffff, 0.08),
                    shadow: 0.42,
                });
            }
            Ok(Self {
                background: rgba(0xf8f8fa, 0.97),
                border: rgba(0x000000, 0.08),
                text: rgba(0x141418, 0.92),
                label: rgba(0x141418, 0.62),
                number: rgba(0x141418, 0.42),
                accent: rgb(0x5874c9),
                selected: rgb(0xffffff),
                edge: rgba(0xffffff, 0.8),
                mark: rgba(0x5874c9, 0.14),
                divider: rgba(0x000000, 0.06),
                shadow: 0.18,
            })
        }
    }
}
/// Room around the panel for its shadow, in DIPs.
const SIDE: f32 = 16.0;
const TOP: f32 = 12.0;
const BOTTOM: f32 = 22.0;
/// Opening: a short fade with a slight spring scale (Pane's panel entrance, scaled down for a popup that opens on
/// every keystroke); moving the selection glides the highlight.
const OPEN_SCALE: f32 = 0.96;
const GLIDE_MS: f32 = 170.0;
const TIMER: usize = 7;

/// Cubic Bézier easing (CSS `cubic-bezier`), `x` in 0..1.
fn ease(curve: [f32; 4], x: f32) -> f32 {
    let [x1, y1, x2, y2] = curve;
    let at = |a: f32, b: f32, t: f32| 3.0 * a * (1.0 - t) * (1.0 - t) * t + 3.0 * b * (1.0 - t) * t * t + t * t * t;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if at(x1, x2, mid) < x { lo = mid } else { hi = mid }
    }
    at(y1, y2, (lo + hi) / 2.0)
}
/// Eased progress of an animation that started at `start`, or `None` once it has ended.
fn progress(start: Option<Instant>, ms: f32, curve: [f32; 4]) -> Option<f32> {
    let t = start?.elapsed().as_secs_f32() * 1000.0 / ms;
    (t < 1.0).then(|| ease(curve, t.max(0.0)))
}
/// Whether Windows' animation effects are on (and this is not a capture run, which needs the final frame).
fn animated() -> bool {
    let mut on = BOOL(1);
    unsafe {
        let _ = SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some((&mut on as *mut BOOL).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    }
    on.as_bool() && std::env::var_os("SHANJIE_VERIFY_UI").is_none()
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
    /// The Zhuyin (or the character being chosen) inside the reading row: left and right edge.
    reading_mark: Option<(f32, f32)>,
    /// The panel's size in DIPs; the window is larger by the shadow margin.
    panel: (f32, f32),
    /// The panel opens above the caret, so the entrance scales from its bottom edge.
    above: bool,
    opened: Option<Instant>,
    /// Where the highlight glides from (x, y, width in panel DIPs) and when it started.
    glide: Option<(Instant, (f32, f32, f32))>,
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
                reading_mark: None,
                panel: (0.0, 0.0),
                above: false,
                opened: None,
                glide: None,
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
        self.opened = None;
        self.glide = None;
        unsafe {
            let _ = KillTimer(Some(self.window), TIMER);
            let _ = ShowWindow(self.window, SW_HIDE);
        }
    }
    pub fn shown(&self) -> bool {
        unsafe { IsWindowVisible(self.window).as_bool() }
    }
    fn measure(&mut self, work_width: f32) -> Result<f32> {
        unsafe {
            let text = self.text_factory.CreateTextFormat(
                &crate::font::family(),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                self.size as f32,
                w!("zh-TW"),
            )?;
            let label = self.text_factory.CreateTextFormat(
                &crate::font::label_family(),
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
            self.reading_mark = None;
            if !reading.is_empty() {
                let format = self.text_factory.CreateTextFormat(
                    &crate::font::family(),
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
                    let edge = |position: usize| -> Result<f32> {
                        let (mut x, mut y) = (0.0, 0.0);
                        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
                        layout.HitTestTextPosition(position as u32, false, &mut x, &mut y, &mut metrics)?;
                        Ok(x)
                    };
                    self.reading_mark = Some((edge(start)?, edge(start + length)?.min(self.reading_width)));
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
        5.0 * self.size as f32 / 16.0 + self.reading_height
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
        self.top() + rows + 5.0 * self.size as f32 / 16.0
    }
    fn fit_view(&mut self, selected: Option<usize>) -> f32 {
        let ratio = self.size as f32 / 16.0;
        let count = self.items.len();
        if self.vertical {
            // A page holds at most nine candidates, so every row is shown; rows share one width.
            self.start = 0;
            self.end = count;
            // Rows keep a minimum width so a list of single characters is not a thin strip.
            let widest = self.items.iter().map(|i| i.natural).fold(112.0 * ratio, f32::max);
            let width = widest.min((self.work_width - 10.0 * ratio).max(1.0));
            for item in &mut self.items {
                item.width = width;
            }
            return (width + 10.0 * ratio).min(self.work_width);
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
            let right = if natural + left + 10.0 * ratio > self.work_width {
                14.0 * ratio
            } else {
                0.0
            };
            let available = (self.work_width - left - right - 10.0 * ratio).max(1.0);
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
            return (used + left + if self.end < count { 14.0 * ratio } else { 0.0 } + 10.0 * ratio)
                .min(self.work_width);
        }
    }
    /// Left edge, top and width of candidate `i` inside the panel.
    fn slot(&self, i: usize) -> (f32, f32, f32) {
        let ratio = self.size as f32 / 16.0;
        let mut x = (5.0 + if self.start > 0 { 14.0 } else { 0.0 }) * ratio;
        if !self.vertical {
            for item in &self.items[self.start..i] {
                x += item.width + 2.0 * ratio;
            }
        }
        (x, self.row_top(i), self.items[i].width)
    }
    /// The highlight where it is drawn now: gliding toward the selected candidate or resting on it.
    fn highlight(&self) -> Option<(f32, f32, f32)> {
        let selected = self.state.selected.filter(|&i| i >= self.start && i < self.end)?;
        let to = self.slot(selected);
        let Some((start, from)) = self.glide else { return Some(to) };
        let Some(t) = progress(Some(start), GLIDE_MS, GLIDE) else { return Some(to) };
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Some((mix(from.0, to.0), mix(from.1, to.1), mix(from.2, to.2)))
    }
    fn moving(&self) -> bool {
        progress(self.opened, NORMAL_MS as f32, SPRING).is_some()
            || progress(self.glide.map(|g| g.0), GLIDE_MS, GLIDE).is_some()
    }
    unsafe fn animate(&self) {
        if self.moving() {
            SetTimer(Some(self.window), TIMER, 8, None);
        }
    }
    /// The window rectangle (x, y, width, height) for a panel whose top-left is `x, y`, in physical pixels.
    fn frame(&self, x: i32, y: i32) -> (i32, i32, i32, i32) {
        let (side, top) = ((SIDE * self.scale).round() as i32, (TOP * self.scale).round() as i32);
        let width = (self.panel.0 * self.scale).ceil() as i32 + 2 * side;
        let height = ((self.panel.1 + TOP + BOTTOM) * self.scale).ceil() as i32;
        (x - side, y - top, width, height)
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
        self.glide = None;
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
        self.panel = (width, self.height());
        let pixels = (width * self.scale).ceil() as i32;
        let left = rect.left + (SIDE * self.scale).round() as i32;
        let top = rect.top + (TOP * self.scale).round() as i32;
        let x = if std::env::var_os("SHANJIE_VERIFY_UI").is_some() {
            left
        } else {
            left.clamp(
                monitor.rcWork.left,
                (monitor.rcWork.right - pixels).max(monitor.rcWork.left),
            )
        };
        let (x, y, w, h) = self.frame(x, top);
        SetWindowPos(self.window, None, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE)?;
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
            let shown = self.shown();
            // Moving the selection inside the same page glides the highlight from where it is drawn now.
            let same = shown && self.state.candidates == state.candidates && self.state.first == state.first && self.size == size;
            let from = if same && self.state.selected != state.selected { self.highlight() } else { None };
            let page = self.start;
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
                let measured = self.measure((monitor.rcWork.right - monitor.rcWork.left) as f32 / self.scale)?;
                self.panel = (measured, self.height());
                let width = (measured * self.scale).ceil() as i32;
                let height = (self.panel.1 * self.scale).ceil() as i32;
                let mut x = caret.left.clamp(
                    monitor.rcWork.left,
                    (monitor.rcWork.right - width).max(monitor.rcWork.left),
                );
                let gap = (6.0 * self.scale).round() as i32;
                let mut y = caret.bottom + gap;
                self.above = y + height > monitor.rcWork.bottom;
                if self.above {
                    y = caret.top - height - gap;
                }
                y = y.max(monitor.rcWork.top);
                if std::env::var_os("SHANJIE_VERIFY_UI").is_some() {
                    x = -12000;
                    y = -12000;
                }
                let motion = animated();
                if !shown {
                    self.opened = motion.then(Instant::now);
                    self.glide = None;
                } else {
                    self.glide = from.filter(|_| motion && self.start == page).map(|f| (Instant::now(), f));
                }
                let (x, y, w, h) = self.frame(x, y);
                SetWindowPos(
                    self.window,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    w,
                    h,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )?;
                self.display()?;
                self.animate();
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
        let fade = progress(self.opened, FAST_MS as f32, SMOOTH).unwrap_or(1.0);
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: (255.0 * fade).round().clamp(0.0, 255.0) as u8,
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
        let (width, height) = self.panel;
        // The entrance scales the panel from its edge next to the caret.
        let grow = progress(self.opened, NORMAL_MS as f32, SPRING).map_or(1.0, |t| OPEN_SCALE + (1.0 - OPEN_SCALE) * t);
        let (cx, cy) = (width / 2.0, if self.above { height } else { 0.0 });
        self.target.SetTransform(&Matrix3x2 {
            M11: grow,
            M12: 0.0,
            M21: 0.0,
            M22: grow,
            M31: SIDE + cx - cx * grow,
            M32: TOP + cy - cy * grow,
        });
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
        // Pane's panel corners in both layouts (no pill): 14 for a list, 12 for the single row.
        let radius = if self.vertical { 14.0 } else { 12.0 } * ratio;
        // Soft shadow: stacked, widening rounded rectangles under the panel, offset downward.
        if self.palette.shadow > 0.0 {
            // Each layer is wider and fainter, so the edges fade out like a blur.
            const LAYERS: usize = 12;
            for i in 0..LAYERS {
                let f = (i + 1) as f32 / LAYERS as f32;
                let spread = 14.0 * f;
                self.brush.SetColor(&rgba(0, self.palette.shadow * 0.09 * (1.0 - f)));
                self.target.FillRoundedRectangle(
                    &rounded(-spread, 3.0 - spread * 0.7, width + spread, height + 3.0 + spread, radius + spread),
                    &self.brush,
                );
            }
            self.brush.SetColor(&rgba(0, self.palette.shadow * 0.25));
            self.target.FillRoundedRectangle(&rounded(-0.5, 0.0, width + 0.5, height + 1.0, radius + 0.5), &self.brush);
        }
        self.brush.SetColor(&self.palette.background);
        self.target
            .FillRoundedRectangle(&rounded(0.0, 0.0, width, height, radius), &self.brush);
        self.brush.SetColor(&self.palette.border);
        self.target.DrawRoundedRectangle(
            &rounded(0.25, 0.25, width - 0.25, height - 0.25, radius - 0.25),
            &self.brush,
            0.5,
            None,
        );
        // Pane's light top edge: a highlight along the upper part of the outline.
        if self.palette.edge.a > 0.0 {
            self.target.PushAxisAlignedClip(
                &D2D_RECT_F { left: 0.0, top: 0.0, right: width, bottom: (height / 2.0).min(radius) },
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            );
            self.brush.SetColor(&self.palette.edge);
            self.target.DrawRoundedRectangle(
                &rounded(1.0, 1.0, width - 1.0, height - 1.0, radius - 1.0),
                &self.brush,
                1.0,
                None,
            );
            self.target.PopAxisAlignedClip();
        }
        if let Some(reading) = &self.reading {
            let (left, top) = (14.0 * ratio, 6.0 * ratio);
            // The Zhuyin being typed (or the character being chosen) sits on a faint accent pill.
            if let Some((a, b)) = self.reading_mark.filter(|&(a, b)| b > a) {
                self.brush.SetColor(&self.palette.mark);
                let h = self.reading_height - 6.0 * ratio;
                self.target.FillRoundedRectangle(
                    &rounded(left + a - 3.0 * ratio, top, left + b + 3.0 * ratio, top + h, 6.0 * ratio),
                    &self.brush,
                );
            }
            self.brush.SetColor(&self.palette.label);
            self.target.DrawTextLayout(
                Vector2 { X: left, Y: top },
                reading,
                &self.brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            );
            if !self.items.is_empty() {
                let y = self.top() - 1.5 * ratio;
                self.brush.SetColor(&self.palette.divider);
                self.target.DrawLine(
                    Vector2 { X: 12.0 * ratio, Y: y },
                    Vector2 { X: width - 12.0 * ratio, Y: y },
                    &self.brush,
                    1.0 / self.scale,
                    None,
                );
            }
        }
        // Pane's item corners, concentric with the panel.
        let corner = (self.row * 0.36).min(9.0 * ratio);
        if let Some((x, y, w)) = self.highlight() {
            self.brush.SetColor(&self.palette.accent);
            self.target.FillRoundedRectangle(&rounded(x, y, x + w, y + self.row, corner), &self.brush);
        }
        for i in self.start..self.end {
            let item = &self.items[i];
            let selected = self.state.selected == Some(i);
            let (x, y, _) = self.slot(i);
            self.target.PushAxisAlignedClip(
                &D2D_RECT_F {
                    left: x,
                    top: y,
                    right: x + item.width,
                    bottom: y + self.row,
                },
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            );
            self.brush.SetColor(&if selected {
                D2D1_COLOR_F { a: self.palette.selected.a * 0.8, ..self.palette.selected }
            } else {
                self.palette.number
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
        }
        self.brush.SetColor(&self.palette.label);
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
        self.target.SetTransform(&Matrix3x2::identity());
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
            let x = (lp.0 as u16 as i16) as f32 / host.scale - SIDE;
            let y = ((lp.0 >> 16) as u16 as i16) as f32 / host.scale - TOP;
            let ratio = host.size as f32 / 16.0;
            if host.start > 0 && x < 17.0 * ratio {
                let _ = host.scroll(false);
                return LRESULT(0);
            }
            if host.end < host.items.len() && x > host.panel.0 - 17.0 * ratio {
                let _ = host.scroll(true);
                return LRESULT(0);
            }
            for i in host.start..host.end {
                let (left, top, width) = host.slot(i);
                if x >= left
                    && x < left + width
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
            }
            return LRESULT(0);
        }
        if message == WM_TIMER && wp.0 == TIMER {
            let host = &mut *ptr;
            let _ = host.display();
            if !host.moving() {
                host.opened = None;
                host.glide = None;
                let _ = KillTimer(Some(window), TIMER);
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
