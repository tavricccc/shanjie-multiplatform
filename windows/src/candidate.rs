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
/// Colors: a near-opaque glass surface (the candidate window lives in other apps' processes, so it has no
/// system blur), a hairline outline with a light top edge, a soft shadow and the 霧藍 accent.
pub(crate) struct Palette {
    pub(crate) background: D2D1_COLOR_F,
    pub(crate) border: D2D1_COLOR_F,
    pub(crate) text: D2D1_COLOR_F,
    label: D2D1_COLOR_F,
    number: D2D1_COLOR_F,
    pub(crate) accent: D2D1_COLOR_F,
    pub(crate) selected: D2D1_COLOR_F,
    pub(crate) hint: D2D1_COLOR_F,
    edge: D2D1_COLOR_F,
    mark: D2D1_COLOR_F,
    divider: D2D1_COLOR_F,
    shadow: f32,
}
impl Palette {
    pub(crate) fn new(theme: u32) -> Result<Self> {
        Self::with(theme, crate::appearance::Appearance::default())
    }
    pub(crate) fn with(theme: u32, appearance: crate::appearance::Appearance) -> Result<Self> {
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
                    hint: system(COLOR_WINDOWTEXT),
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
            let accent = appearance.accent(if matches!(appearance.accent,shanjie_shell::appearance::Accent::Windows){crate::appearance::system_accent()}else{crate::appearance::DEFAULT_ACCENT});
            let selected = appearance.selected_text(accent);
            if theme == 2 || (theme == 0 && light == 0) {
                return Ok(Self {
                    background: if appearance.pure_black { rgb(0) } else { rgba(0x1c1c20, 0.97) },
                    border: rgba(0xffffff, 0.08),
                    text: rgba(0xffffff, 0.94),
                    label: rgba(0xffffff, 0.64),
                    number: rgba(0xffffff, 0.4),
                    accent: rgb(accent),
                    selected: rgb(selected),
                    hint: rgb(shanjie_shell::appearance::text_accent(accent,if appearance.pure_black{0}else{0x1c1c20})),
                    edge: rgba(0xffffff, 0.12),
                    mark: rgba(accent, 0.3),
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
                accent: rgb(accent),
                selected: rgb(selected),
                hint: rgb(shanjie_shell::appearance::text_accent(accent,0xf8f8fa)),
                edge: rgba(0xffffff, 0.8),
                mark: rgba(accent, 0.14),
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
/// Opening: a short fade with a slight spring scale (panel entrance, scaled down for a popup that opens on
/// every keystroke); moving the selection glides the highlight.
const OPEN_SCALE: f32 = 0.96;
const GLIDE_MS: f32 = 170.0;
/// A panel already on screen glides to its new size and place (longer candidates, a second row, the caret moving).
const MORPH_MS: f32 = 180.0;
const TIMER: usize = 7;
/// The reading row's inset from the panel edge, in DIPs at the 16px base size.
const READING_SIDE: f32 = 12.0;
const READING_TOP: f32 = 4.0;

/// From a one-line layout's top to the middle of its glyphs. A CJK font's line box leaves more room above the
/// characters than below (Noto Sans TC: ascent 1.16 em, the ideographic box 0.88 em above the baseline and
/// 0.12 em below), so centring the line box leaves the text visibly off; centre the ideographic box instead.
/// UTF-16 length of the first `chars` characters of `units`, clamped to the text that is there.
fn mark_units(units: &[u16], chars: usize) -> usize {
    let mut at = 0;
    for _ in 0..chars {
        match units.get(at) {
            Some(0xd800..=0xdbff) => at = (at + 2).min(units.len()),
            Some(_) => at += 1,
            None => break,
        }
    }
    at
}

#[cfg(test)]
mod mark_tests {
    use super::mark_units;

    #[test]
    fn mark_covers_the_selected_candidate() {
        let text: Vec<u16> = "你好測試".encode_utf16().collect();
        assert_eq!(mark_units(&text, 1), 1);
        assert_eq!(mark_units(&text, 2), 2);
        assert_eq!(mark_units(&text, 9), 4, "clamped to the text");
        let astral: Vec<u16> = "𠀀好".encode_utf16().collect();
        assert_eq!(mark_units(&astral, 1), 2);
        assert_eq!(mark_units(&astral, 2), 3);
    }
}

fn middle(layout: &IDWriteTextLayout, size: f32) -> f32 {
    let mut line = [DWRITE_LINE_METRICS::default()];
    let mut count = 0;
    match unsafe { layout.GetLineMetrics(Some(&mut line), &mut count) } {
        Ok(()) if count > 0 => line[0].baseline - 0.38 * size,
        _ => 0.62 * size,
    }
}
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
/// `SHANJIE_VERIFY_MOTION` is the harness asking to time a glide whatever the desktop's setting is.
fn animated(policy: shanjie_shell::appearance::Animation) -> bool {
    if std::env::var_os("SHANJIE_VERIFY_MOTION").is_some() {
        return true;
    }
    let mut on = BOOL(1);
    unsafe {
        let _ = SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some((&mut on as *mut BOOL).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    }
    policy.enabled(on.as_bool()) && std::env::var_os("SHANJIE_VERIFY_UI").is_none()
}
struct Item {
    word: IDWriteTextLayout,
    label: IDWriteTextLayout,
    width: f32,
    natural: f32,
    tag: f32,
    /// From the layout's top to the middle of its glyphs.
    word_middle: f32,
    label_middle: f32,
}
pub struct Host {
    pub window: HWND,
    callback: HWND,
    pub sequence: i32,
    state: Snapshot,
    theme: u32,
    pub(crate) appearance: crate::appearance::Appearance,
    hint_brush: ID2D1SolidColorBrush,
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
    reading_middle: f32,
    /// The panel's size in DIPs; the window is larger by the shadow margin.
    panel: (f32, f32),
    /// The panel opens above the caret, so the entrance scales from its bottom edge.
    above: bool,
    opened: Option<Instant>,
    /// Where the highlight glides from (x, y, width in panel DIPs) and when it started.
    glide: Option<(Instant, (f32, f32, f32))>,
    /// The panel's top-left on screen in physical pixels (where it rests).
    origin: (i32, i32),
    /// Where the panel glides from: its size in DIPs and its top-left, and when it started.
    morph: Option<(Instant, (f32, f32), (i32, i32))>,
    start: usize,
    end: usize,
    work_width: f32,
    /// Room for the panel on the roomier side of the caret, in DIPs; the grid shows only the rows that fit.
    work_height: f32,
    /// The candidate target is the character right of the cursor (profile `anchor=right`).
    pub anchor_right: bool,
}
/// The visible grid items: whole rows, moved only as far as needed to keep the selection inside.
fn grid_window(count: usize, selected: Option<usize>, start: usize, max_rows: usize) -> (usize, usize) {
    let columns = crate::input::GRID_COLUMNS;
    let total = count.div_ceil(columns);
    let rows = max_rows.clamp(1, total.max(1));
    let mut top = (start / columns).min(total.saturating_sub(rows));
    if let Some(row) = selected.map(|i| i / columns) {
        if row < top { top = row } else if row >= top + rows { top = row + 1 - rows }
    }
    (top * columns, ((top + rows) * columns).min(count))
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
            let hint_brush = target.CreateSolidColorBrush(&rgb(crate::appearance::DEFAULT_ACCENT), None)?;
            let mut host = Box::new(Self {
                window: HWND::default(),
                callback,
                sequence: 0,
                state: Snapshot::default(),
                theme: 0,
                appearance: crate::appearance::Appearance::default(),
                size: 16,
                scale: 1.0,
                row: 28.0,
                _factory: factory,
                text_factory,
                target,
                brush,
                hint_brush,
                palette: Palette::new(0)?,
                items: Vec::new(),
                reading: None,
                reading_width: 0.0,
                reading_height: 0.0,
                reading_mark: None,
                reading_middle: 0.0,
                panel: (0.0, 0.0),
                above: false,
                opened: None,
                glide: None,
                origin: (0, 0),
                morph: None,
                start: 0,
                end: 0,
                work_width: 0.0,
                work_height: 0.0,
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
        self.morph = None;
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
                    let number = if self.state.columns > 0 { (self.state.first as usize + i) % 9 + 1 } else { i + 1 };
                    format!("{}{}", if self.state.associated { "⇧" } else { "" }, number)
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
                let (word_middle, label_middle) =
                    (middle(&word, self.size as f32), middle(&number, (self.size as f32 * 9.0 / 16.0).max(8.0)));
                self.items.push(Item {
                    word,
                    label: number,
                    width: wm.widthIncludingTrailingWhitespace + tag_width + 11.0 * ratio,
                    natural: wm.widthIncludingTrailingWhitespace + tag_width + 11.0 * ratio,
                    tag: tag_width,
                    word_middle,
                    label_middle,
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
                // Right anchor: the text after the cursor that the selected candidate replaces (one character per
                // syllable, so as many characters as the candidate has; one when nothing is selected).
                let chars = if self.state.associated {
                    1
                } else {
                    self.state.selected.and_then(|i| self.state.candidates.get(i)).map_or(1, |c| c.chars().count().max(1))
                };
                mark = Some((cursor, mark_units(&units[cursor..], chars)));
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
                // Alone (still typing, nothing to choose yet) the row is the whole panel and takes the
                // candidates' size; above a candidate row it is smaller.
                let reading_size = if self.items.is_empty() { self.size as f32 } else { (self.size as f32 * 0.8).max(9.0) };
                let format = self.text_factory.CreateTextFormat(
                    &crate::font::family(),
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    reading_size,
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
                    .min((work_width - 2.0 * READING_SIDE * ratio).max(1.0));
                self.reading_height = m.height + 4.0 * ratio;
                self.reading_middle = middle(&layout, reading_size);
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
                    let _ = layout.SetDrawingEffect(
                        &self.hint_brush,
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
            Ok(width.max(self.reading_width + 2.0 * READING_SIDE * ratio).min(work_width))
        }
    }
    /// Where the candidate row starts: below the reading row and its divider when there is one.
    fn top(&self) -> f32 {
        let ratio = self.size as f32 / 16.0;
        if self.reading.is_none() {
            5.0 * ratio
        } else if self.items.is_empty() {
            READING_TOP * ratio + self.reading_height
        } else {
            READING_TOP * ratio + self.reading_height + 3.0 * ratio
        }
    }
    fn row_top(&self, index: usize) -> f32 {
        let row = if self.state.columns > 0 { (index - self.start) / crate::input::GRID_COLUMNS } else { index - self.start };
        self.top() + row as f32 * (self.row + 2.0 * self.size as f32 / 16.0)
    }
    fn height(&self) -> f32 {
        let lines = if self.state.columns > 0 { (self.end - self.start).div_ceil(crate::input::GRID_COLUMNS) } else { self.end - self.start };
        let rows = if self.items.is_empty() {
            0.0
        } else {
            lines as f32 * self.row + lines.saturating_sub(1) as f32 * 2.0 * self.size as f32 / 16.0
        };
        // A reading row alone gets the same room below as above, so its text sits in the middle.
        let bottom = if self.items.is_empty() { READING_TOP } else { 5.0 };
        self.top() + rows + bottom * self.size as f32 / 16.0
    }
    fn fit_view(&mut self, selected: Option<usize>) -> f32 {
        let ratio = self.size as f32 / 16.0;
        let count = self.items.len();
        if self.state.columns > 0 && count > 0 {
            let gap = 2.0 * ratio;
            let rows = (self.work_height - self.top() - 5.0 * ratio + gap) / (self.row + gap);
            (self.start, self.end) = grid_window(count, selected, self.start, rows.max(1.0) as usize);
            let columns = crate::input::GRID_COLUMNS;
            let mut widths = [0.0f32; crate::input::GRID_COLUMNS];
            for (i, item) in self.items.iter().enumerate() {
                widths[i % columns] = widths[i % columns].max(item.natural);
            }
            let gaps = (columns - 1) as f32 * 2.0 * ratio;
            let natural = widths.iter().sum::<f32>();
            let scale = ((self.work_width - 10.0 * ratio - gaps).max(1.0) / natural).min(1.0);
            for (i, item) in self.items.iter_mut().enumerate() {
                item.width = widths[i % columns] * scale;
            }
            return (natural * scale + gaps + 10.0 * ratio).min(self.work_width);
        }
        // A page holds at most nine candidates, so every row is shown; rows share one width.
        self.start = 0;
        self.end = count;
        // The reading row alone has no rows to keep wide: the panel is exactly its text (`measure` adds the insets).
        if count == 0 {
            return 0.0;
        }
        // Rows keep a minimum width so a list of single characters is not a thin strip.
        let widest = self.items.iter().map(|i| i.natural).fold(112.0 * ratio, f32::max);
        let width = widest.min((self.work_width - 10.0 * ratio).max(1.0));
        for item in &mut self.items {
            item.width = width;
        }
        (width + 10.0 * ratio).min(self.work_width)
    }
    /// Left edge, top and width of candidate `i` inside the panel.
    fn slot(&self, i: usize) -> (f32, f32, f32) {
        let ratio = self.size as f32 / 16.0;
        let mut x = 5.0 * ratio;
        if self.state.columns > 0 {
            for item in &self.items[..i % crate::input::GRID_COLUMNS] {
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
            || progress(self.morph.map(|m| m.0), MORPH_MS, GLIDE).is_some()
    }
    /// The panel as drawn now: its size in DIPs and its top-left in physical pixels, gliding toward where it rests.
    fn view(&self) -> ((f32, f32), (i32, i32)) {
        let Some((start, size, origin)) = self.morph else { return (self.panel, self.origin) };
        let Some(t) = progress(Some(start), MORPH_MS, GLIDE) else { return (self.panel, self.origin) };
        let mix = |a: f32, b: f32| a + (b - a) * t;
        let place = |a: i32, b: i32| mix(a as f32, b as f32).round() as i32;
        ((mix(size.0, self.panel.0), mix(size.1, self.panel.1)), (place(origin.0, self.origin.0), place(origin.1, self.origin.1)))
    }
    unsafe fn animate(&self) {
        if self.moving() {
            SetTimer(Some(self.window), TIMER, 8, None);
        }
    }
    /// The window rectangle (x, y, width, height) for a panel of `panel` DIPs whose top-left is `x, y`, in physical pixels.
    fn frame(&self, panel: (f32, f32), x: i32, y: i32) -> (i32, i32, i32, i32) {
        let (side, top) = ((SIDE * self.scale).round() as i32, (TOP * self.scale).round() as i32);
        let width = (panel.0 * self.scale).ceil() as i32 + 2 * side;
        let height = ((panel.1 + TOP + BOTTOM) * self.scale).ceil() as i32;
        (x - side, y - top, width, height)
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
            // A panel on screen glides from where it is drawn now, even halfway through an earlier glide.
            let (was, at) = self.view();
            // Moving the selection inside the same page glides the highlight from where it is drawn now.
            let same = shown && self.state.candidates == state.candidates && self.state.first == state.first && self.size == size;
            let from = if same && self.state.selected != state.selected { self.highlight() } else { None };
            let page = self.start;
            if self.state.candidates != state.candidates || self.state.first != state.first || self.state.columns != state.columns {
                self.start = 0;
            }
            self.state = state;
            self.theme = theme;
            self.size = size;
            self.palette = Palette::with(theme, self.appearance)?;
            self.hint_brush.SetColor(&self.palette.hint);
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
                let scale = self.scale;
                if shown {
                    // Moving the visible window to the caret first would flash it there before the glide.
                    let (mut dpi, mut unused) = (96u32, 96u32);
                    let screen = MonitorFromRect(&caret, MONITOR_DEFAULTTONEAREST);
                    if GetDpiForMonitor(screen, MDT_EFFECTIVE_DPI, &mut dpi, &mut unused).is_ok() {
                        self.scale = dpi as f32 / 96.0;
                    }
                } else {
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
                }
                let spacing = (6.0 * self.scale).round() as i32;
                let room = (monitor.rcWork.bottom - caret.bottom).max(caret.top - monitor.rcWork.top) - spacing;
                self.work_height = room as f32 / self.scale;
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
                let motion = animated(self.appearance.animation);
                self.origin = (x, y);
                if !shown {
                    self.opened = motion.then(Instant::now);
                    self.glide = None;
                    self.morph = None;
                } else {
                    self.glide = from.filter(|_| motion && self.start == page).map(|f| (Instant::now(), f));
                    let moved = was != self.panel || at != self.origin;
                    // A new monitor scale changes what the DIP sizes mean, so that jump is not animated.
                    self.morph = (motion && moved && scale == self.scale).then(|| (Instant::now(), was, at));
                }
                let (panel, (x, y)) = self.view();
                let (x, y, w, h) = self.frame(panel, x, y);
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
        // The window follows the panel's glide: each frame sets its place and size with the new bitmap.
        let (panel, (x, y)) = self.view();
        let (x, y, w, h) = self.frame(panel, x, y);
        let buffer = Buffer::new(w, h)?;
        self.paint(buffer.dc, w, h, panel)?;
        let position = POINT { x, y };
        let size = SIZE { cx: w, cy: h };
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
    /// Draws the panel at `panel` DIPs into a `w` x `h` bitmap; while it glides the content keeps its resting layout,
    /// clipped to the panel.
    unsafe fn paint(&self, dc: HDC, w: i32, h: i32, panel: (f32, f32)) -> Result<()> {
        let bounds = RECT { left: 0, top: 0, right: w, bottom: h };
        self.target.BindDC(dc, &bounds)?;
        self.target.SetDpi(96.0 * self.scale, 96.0 * self.scale);
        self.target.BeginDraw();
        self.target.Clear(Some(&D2D1_COLOR_F::default()));
        let (width, height) = panel;
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
        let radius = 14.0 * ratio;
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
        // Light top edge: a highlight along the upper part of the outline.
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
        self.target.PushAxisAlignedClip(
            &D2D_RECT_F { left: 0.0, top: 0.0, right: width, bottom: height },
            D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
        );
        if let Some(reading) = &self.reading {
            let (left, row) = (READING_SIDE * ratio, READING_TOP * ratio);
            let top = row + self.reading_height / 2.0 - self.reading_middle;
            // The Zhuyin being typed (or the character being chosen) sits on a faint accent pill.
            if let Some((a, b)) = self.reading_mark.filter(|&(a, b)| b > a) {
                self.brush.SetColor(&self.palette.mark);
                self.target.FillRoundedRectangle(
                    &rounded(left + a - 3.0 * ratio, row + ratio, left + b + 3.0 * ratio, row + self.reading_height - ratio, 6.0 * ratio),
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
        // Item corners, concentric with the panel.
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
                    Y: y + self.row / 2.0 - item.label_middle,
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
                    Y: y + self.row / 2.0 - item.word_middle,
                },
                &item.word,
                &self.brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            );
            self.target.PopAxisAlignedClip();
        }
        self.target.PopAxisAlignedClip();
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
            let host = &*ptr;
            let mut rect = RECT::default();
            let _ = GetClientRect(window, &mut rect);
            let _ = host.paint(HDC(wp.0 as _), rect.right, rect.bottom, host.view().0);
            return LRESULT(0);
        }
        if message == WM_LBUTTONUP {
            let host = &mut *ptr;
            if host.state.reading_hint { return LRESULT(0); }
            let x = (lp.0 as u16 as i16) as f32 / host.scale - SIDE;
            let y = ((lp.0 >> 16) as u16 as i16) as f32 / host.scale - TOP;
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
                host.morph = None;
                let _ = KillTimer(Some(window), TIMER);
            }
            return LRESULT(0);
        }
        if [
            WM_SETTINGCHANGE,
            WM_SYSCOLORCHANGE,
            WM_THEMECHANGED,
            WM_DWMCOLORIZATIONCOLORCHANGED,
        ]
        .contains(&message)
        {
            let host = &mut *ptr;
            if !animated(host.appearance.animation) {
                host.opened=None;host.glide=None;host.morph=None;
                let _=KillTimer(Some(window),TIMER);
            }
            if let Ok(p) = Palette::with(host.theme, host.appearance) {
                host.hint_brush.SetColor(&p.hint);
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
#[cfg(test)]
mod appearance_tests {
    use super::*;
    use crate::appearance::Appearance;
    #[test]
    fn custom_accent_and_black_surface_follow_the_profile() {
        if crate::appearance::high_contrast() { return; }
        let mut appearance=Appearance::default();
        appearance.accent=shanjie_shell::appearance::Accent::Custom(0xffff00);
        appearance.pure_black=true;
        let dark=Palette::with(2,appearance).unwrap();
        assert_eq!((dark.background.r,dark.background.g,dark.background.b,dark.background.a),(0.0,0.0,0.0,1.0));
        assert_eq!((dark.accent.r,dark.accent.g,dark.accent.b),(1.0,1.0,0.0));
        assert_eq!((dark.selected.r,dark.selected.g,dark.selected.b),(0.0,0.0,0.0));
        assert!(Palette::with(1,appearance).unwrap().background.r>0.9);
    }
    #[test]
    #[ignore="Owned offscreen renderer measurement; run with isolated verification permission"]
    fn candidate_buffer_allocation_cost() { unsafe {
        use windows::Win32::System::{Com::*,Threading::*};
        CoInitializeEx(None,COINIT_APARTMENTTHREADED).ok().unwrap();
        std::env::set_var("SHANJIE_VERIFY_UI","1");
        let mut host=Host::start(HWND::default(),None).unwrap();
        let state=Snapshot{candidates:vec!["善解".into(),"候選視窗".into(),"文字輸入".into()],selected:Some(0),total:3,..Default::default()};
        let caret=RECT{left:100,top:100,right:101,bottom:120};
        let cpu=||{
            let(mut a,mut b,mut c,mut d)=(FILETIME::default(),FILETIME::default(),FILETIME::default(),FILETIME::default());
            GetThreadTimes(GetCurrentThread(),&mut a,&mut b,&mut c,&mut d).unwrap();
            ((c.dwHighDateTime as u64)<<32|c.dwLowDateTime as u64)+((d.dwHighDateTime as u64)<<32|d.dwLowDateTime as u64)
        };
        for size in [16,32] {
            host.publish(state.clone(),2,size,caret,true).unwrap();
            let panel=host.panel;let(_,_,w,h)=host.frame(panel,100,100);
            let baseline=GetGuiResources(GetCurrentProcess(),GR_GDIOBJECTS);
            let buffer=Buffer::new(w,h).unwrap();
            let mut fresh=Vec::new();let mut reused=Vec::new();let mut allocations=Vec::new();
            let start_cpu=cpu();
            for i in 0..320 {
                let order=if i%2==0{[true,false]}else{[false,true]};
                for allocate in order {
                    let start=Instant::now();
                    if allocate {let alloc=Instant::now();let b=Buffer::new(w,h).unwrap();if i>=20{allocations.push(alloc.elapsed().as_secs_f64()*1e6);}host.paint(b.dc,w,h,panel).unwrap();}
                    else {host.paint(buffer.dc,w,h,panel).unwrap();}
                    if i>=20{if allocate{fresh.push(start.elapsed().as_secs_f64()*1e6)}else{reused.push(start.elapsed().as_secs_f64()*1e6)}}
                }
            }
            let cpu_ms=(cpu()-start_cpu) as f64/10000.0;
            drop(buffer);
            let after=GetGuiResources(GetCurrentProcess(),GR_GDIOBJECTS);
            assert!(after<=baseline+2,"GDI objects must not grow: {baseline} -> {after}");
            for values in [&mut fresh,&mut reused,&mut allocations]{values.sort_by(f64::total_cmp);}
            println!("BUFFER size={size} pixels={w}x{h} frames=600 allocations=300 fresh_p50_us={:.3} reused_p50_us={:.3} allocation_p50_us={:.3} fresh_p95_us={:.3} reused_p95_us={:.3} thread_cpu_ms={cpu_ms:.3} gdi={baseline}->{after}",fresh[150],reused[150],allocations[150],fresh[285],reused[285]);
            host.hide();
        }
        drop(host);std::env::remove_var("SHANJIE_VERIFY_UI");CoUninitialize();
    }}
}
#[cfg(test)]
mod grid_tests {
    use super::grid_window;
    #[test]
    fn the_grid_window_keeps_whole_rows_and_the_selection_visible() {
        assert_eq!(grid_window(45, Some(0), 0, 20), (0, 45), "everything fits");
        assert_eq!(grid_window(44, Some(43), 0, 20), (0, 44), "a short last row");
        assert_eq!(grid_window(45, Some(0), 0, 4), (0, 12));
        assert_eq!(grid_window(45, Some(12), 0, 4), (3, 15), "down scrolls one row");
        assert_eq!(grid_window(45, Some(5), 3, 4), (3, 15), "inside: no movement");
        assert_eq!(grid_window(45, Some(2), 3, 4), (0, 12), "up scrolls one row");
        assert_eq!(grid_window(45, Some(44), 0, 4), (33, 45));
        assert_eq!(grid_window(44, Some(43), 0, 4), (33, 44));
        assert_eq!(grid_window(45, Some(20), 0, 0), (18, 21), "always one row");
        assert_eq!(grid_window(9, None, 6, 20), (0, 9), "a stale start is clamped");
    }
}
