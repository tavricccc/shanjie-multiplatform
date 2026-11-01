//! Glass material shared by every Shanjie window: the settings window, the installer and the candidate window.
//!
//! The look follows Pane: a window of its own behind the content, drawn by the system compositor with the host
//! backdrop (system blur) plus saturation, a luminosity and a tint layer, a gradient highlight edge and two drop
//! shadows; panels open with a slight spring scale and fade, and close by shrinking and fading out.
mod effect;

use std::sync::Once;
use windows_numerics::{Matrix4x4, Vector2, Vector3};
use windows::{
    core::*,
    
    Graphics::Effects::IGraphicsEffectSource,
    System::DispatcherQueue,
    UI::{
        Color,
        Composition::{
            CompositionBrush, CompositionColorBrush, CompositionEffectSourceParameter, CompositionLinearGradientBrush,
            CompositionRoundedRectangleGeometry, CompositionSpriteShape, Compositor, ContainerVisual, DropShadow,
            Desktop::DesktopWindowTarget, ShapeVisual, SpriteVisual,
        },
        ViewManagement::UISettings,
    },
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK, DWMWA_USE_HOSTBACKDROPBRUSH, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND},
        System::{
            LibraryLoader::GetModuleHandleW,
            WinRT::{
                Composition::ICompositorDesktopInterop, CreateDispatcherQueueController, DispatcherQueueOptions,
                DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT,
            },
        },
        UI::WindowsAndMessaging::*,
    },
};

/// Pane's design tokens. Shanjie has no settings for these, so they stay at Pane's defaults.
pub mod tokens {
    pub const RADIUS: f32 = 20.0;
    pub const TINT_OPACITY: f32 = 0.42;
    pub const LUMINOSITY_OPACITY: f32 = 0.38;
    pub const SATURATION: f32 = 1.6;
    pub const HIGHLIGHT: f32 = 0.7;
    pub const SHADOW: f32 = 0.7;
    pub const EXTRA_BLUR: f32 = 0.0;
    pub const ENTER_SCALE: f32 = 0.94;
    pub const FAST_MS: u32 = 120;
    pub const NORMAL_MS: u32 = 220;
    pub const SLOW_MS: u32 = 360;
    pub const SMOOTH: [f32; 4] = [0.2, 0.0, 0.0, 1.0];
    pub const SPRING: [f32; 4] = [0.34, 1.36, 0.64, 1.0];
    pub const GLIDE: [f32; 4] = [0.25, 1.05, 0.35, 1.0];
}
use tokens::*;

/// Room around the panel for the shadows, in DIPs: left, top and right; the bottom gets more for the offset shadow.
pub const MARGIN: f32 = 48.0;
pub const MARGIN_BOTTOM: f32 = 64.0;

/// Whether Windows asks for solid surfaces (transparency effects off).
pub fn solid() -> bool {
    UISettings::new().and_then(|s| s.AdvancedEffectsEnabled()).map(|on| !on).unwrap_or(false)
        || std::env::var("SHANJIE_FORCE_SOLID").as_deref() == Ok("1")
}

/// Whether apps use the dark theme.
pub fn dark() -> bool {
    UISettings::new()
        .and_then(|s| s.GetColorValue(windows::UI::ViewManagement::UIColorType::Foreground))
        .map(|c| c.R > 128)
        .unwrap_or(false)
}

fn argb(a: f32, r: u8, g: u8, b: u8) -> Color {
    Color { A: (255.0 * a).round().clamp(0.0, 255.0) as u8, R: r, G: g, B: b }
}

fn ensure_dispatcher() -> Result<()> {
    if DispatcherQueue::GetForCurrentThread().is_ok() {
        return Ok(());
    }
    let options = DispatcherQueueOptions {
        dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
        threadType: DQTYPE_THREAD_CURRENT,
        apartmentType: DQTAT_COM_NONE,
    };
    // The controller has to live as long as the thread's compositor objects; it is leaked on purpose.
    std::mem::forget(unsafe { CreateDispatcherQueueController(options)? });
    Ok(())
}

const CLASS: PCWSTR = w!("ShanjieGlass");

unsafe extern "system" fn proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match message {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        _ => DefWindowProcW(window, message, w, l),
    }
}

/// The glass window and its composition scene. Lives on the thread that created it.
pub struct Glass {
    window: HWND,
    compositor: Compositor,
    _target: DesktopWindowTarget,
    root: ContainerVisual,
    shadows: [SpriteVisual; 2],
    mask: ShapeVisual,
    mask_geometry: CompositionRoundedRectangleGeometry,
    mask_surface: windows::UI::Composition::CompositionVisualSurface,
    clip: CompositionRoundedRectangleGeometry,
    material: SpriteVisual,
    luminosity: SpriteVisual,
    tint: SpriteVisual,
    edges: ShapeVisual,
    outer: CompositionRoundedRectangleGeometry,
    inner: CompositionRoundedRectangleGeometry,
    size: (f32, f32),
    dpi: f32,
    dark: bool,
    solid: bool,
    shown: bool,
}

impl Glass {
    /// `owner` keeps the glass in the content window's z-order band and hides it with the owner.
    pub fn new(owner: Option<HWND>) -> Result<Self> {
        ensure_dispatcher()?;
        static REGISTER: Once = Once::new();
        let instance = unsafe { GetModuleHandleW(None)? };
        REGISTER.call_once(|| unsafe {
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                lpszClassName: CLASS,
                ..Default::default()
            };
            RegisterClassExW(&class);
        });
        let window = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
                CLASS,
                w!(""),
                WS_POPUP,
                0,
                0,
                1,
                1,
                owner,
                None,
                Some(instance.into()),
                None,
            )?
        };
        unsafe {
            SetLayeredWindowAttributes(window, Default::default(), 255, LWA_ALPHA)?;
            let on: i32 = 1;
            DwmSetWindowAttribute(window, DWMWA_USE_HOSTBACKDROPBRUSH, &on as *const _ as _, 4)?;
            let corner = DWMWCP_DONOTROUND;
            let _ = DwmSetWindowAttribute(window, DWMWA_WINDOW_CORNER_PREFERENCE, &corner as *const _ as _, 4);
        }
        let c = Compositor::new()?;
        let target = unsafe { c.cast::<ICompositorDesktopInterop>()?.CreateDesktopWindowTarget(window, false)? };
        let root = c.CreateContainerVisual()?;
        target.SetRoot(&root)?;
        let offset = Vector3 { X: MARGIN, Y: MARGIN, Z: 0.0 };

        let mask_geometry = c.CreateRoundedRectangleGeometry()?;
        let mask = c.CreateShapeVisual()?;
        let mask_shape = c.CreateSpriteShapeWithGeometry(&mask_geometry)?;
        mask_shape.SetFillBrush(&c.CreateColorBrushWithColor(argb(1.0, 255, 255, 255))?)?;
        mask.Shapes()?.Append(&mask_shape)?;
        let mask_surface = c.CreateVisualSurface()?;
        mask_surface.SetSourceVisual(&mask)?;
        let mask_brush = c.CreateSurfaceBrushWithSurface(&mask_surface)?;
        let shadow_layer = c.CreateContainerVisual()?;
        root.Children()?.InsertAtTop(&shadow_layer)?;
        let shadow = |_: usize| -> Result<SpriteVisual> {
            let s = c.CreateDropShadow()?;
            s.SetMask(&mask_brush)?;
            s.SetColor(argb(1.0, 0, 0, 0))?;
            let v = c.CreateSpriteVisual()?;
            v.SetShadow(&s)?;
            v.SetOffset(offset)?;
            shadow_layer.Children()?.InsertAtTop(&v)?;
            Ok(v)
        };
        let shadows = [shadow(0)?, shadow(1)?];

        let clip = c.CreateRoundedRectangleGeometry()?;
        let layer = |brush: Option<CompositionBrush>| -> Result<SpriteVisual> {
            let v = c.CreateSpriteVisual()?;
            v.SetOffset(offset)?;
            v.SetClip(&c.CreateGeometricClipWithGeometry(&clip)?)?;
            if let Some(brush) = brush {
                v.SetBrush(&brush)?;
            }
            root.Children()?.InsertAtTop(&v)?;
            Ok(v)
        };
        let material = layer(None)?;
        let luminosity = layer(Some(c.CreateColorBrush()?.cast()?))?;
        let tint = layer(Some(c.CreateColorBrush()?.cast()?))?;

        let edges = c.CreateShapeVisual()?;
        edges.SetOffset(offset)?;
        let outer = c.CreateRoundedRectangleGeometry()?;
        outer.SetOffset(Vector2 { X: -0.5, Y: -0.5 })?;
        let outer_shape = c.CreateSpriteShapeWithGeometry(&outer)?;
        outer_shape.SetStrokeThickness(1.0)?;
        outer_shape.SetStrokeBrush(&c.CreateColorBrush()?)?;
        let inner = c.CreateRoundedRectangleGeometry()?;
        inner.SetOffset(Vector2 { X: 0.5, Y: 0.5 })?;
        let gradient = c.CreateLinearGradientBrush()?;
        gradient.SetStartPoint(Vector2 { X: 0.0, Y: 0.0 })?;
        gradient.SetEndPoint(Vector2 { X: 0.0, Y: 1.0 })?;
        for stop in [0.0, 0.3, 0.7, 1.0] {
            gradient.ColorStops()?.Append(&c.CreateColorGradientStop()?.tap(|s| s.SetOffset(stop))?)?;
        }
        let inner_shape = c.CreateSpriteShapeWithGeometry(&inner)?;
        inner_shape.SetStrokeThickness(1.0)?;
        inner_shape.SetStrokeBrush(&gradient)?;
        edges.Shapes()?.Append(&outer_shape)?;
        edges.Shapes()?.Append(&inner_shape)?;
        root.Children()?.InsertAtTop(&edges)?;

        let mut glass = Self {
            window,
            compositor: c,
            _target: target,
            root,
            shadows,
            mask,
            mask_geometry,
            mask_surface,
            clip,
            material,
            luminosity,
            tint,
            edges,
            outer,
            inner,
            size: (0.0, 0.0),
            dpi: 1.0,
            dark: !dark(),
            solid: false,
            shown: false,
        };
        glass.set_theme(dark(), solid())?;
        Ok(glass)
    }

    pub fn window(&self) -> HWND {
        self.window
    }

    fn saturated_backdrop(&self) -> Result<CompositionBrush> {
        let c = &self.compositor;
        let backdrop: IGraphicsEffectSource = CompositionEffectSourceParameter::Create(h!("Backdrop"))?.cast()?;
        let mut effect = effect::saturation(&backdrop, SATURATION);
        if EXTRA_BLUR > 0.0 {
            effect = effect::blur(&effect.cast()?, EXTRA_BLUR);
        }
        let brush = c.CreateEffectFactory(&effect)?.CreateBrush()?;
        brush.SetSourceParameter(h!("Backdrop"), &c.CreateHostBackdropBrush()?)?;
        brush.cast()
    }

    /// Applies the theme and Windows' transparency setting. Does nothing when neither changed.
    pub fn set_theme(&mut self, dark: bool, solid: bool) -> Result<()> {
        if self.dark == dark && self.solid == solid && self.material.Brush().is_ok() {
            return Ok(());
        }
        self.dark = dark;
        self.solid = solid;
        let c = &self.compositor;
        let brush: CompositionBrush = if solid {
            c.CreateColorBrushWithColor(if dark { argb(1.0, 22, 22, 25) } else { argb(1.0, 249, 249, 250) })?.cast()?
        } else {
            self.saturated_backdrop()?
        };
        self.material.SetBrush(&brush)?;
        let color = |v: &SpriteVisual, value: Color| -> Result<()> { v.Brush()?.cast::<CompositionColorBrush>()?.SetColor(value) };
        color(&self.tint, if dark { argb(TINT_OPACITY, 28, 28, 32) } else { argb(TINT_OPACITY, 246, 246, 248) })?;
        color(&self.luminosity, if dark { argb(LUMINOSITY_OPACITY, 10, 10, 12) } else { argb(LUMINOSITY_OPACITY, 255, 255, 255) })?;
        let layers = if solid { 0.0 } else { 1.0 };
        self.tint.SetOpacity(layers)?;
        self.luminosity.SetOpacity(layers)?;
        // Dark glass gets no dark outline (it reads as a black frame on busy backgrounds), only a faint white highlight.
        let shapes = self.edges.Shapes()?;
        shapes.GetAt(0)?.cast::<CompositionSpriteShape>()?.StrokeBrush()?.cast::<CompositionColorBrush>()?
            .SetColor(argb(if dark { 0.0 } else { 0.1 * HIGHLIGHT }, 0, 0, 0))?;
        let gradient = shapes.GetAt(1)?.cast::<CompositionSpriteShape>()?.StrokeBrush()?.cast::<CompositionLinearGradientBrush>()?;
        let strengths: [f32; 4] = if dark { [1.0, 0.6, 0.45, 0.65] } else { [1.0, 0.3, 0.1, 0.45] };
        for (i, strength) in strengths.iter().enumerate() {
            gradient.ColorStops()?.GetAt(i as u32)?.SetColor(argb((if dark { 0.22 } else { 0.75 }) * HIGHLIGHT * strength, 255, 255, 255))?;
        }
        for (i, visual) in self.shadows.iter().enumerate() {
            let s = visual.Shadow()?.cast::<DropShadow>()?;
            s.SetBlurRadius(if i == 0 { 64.0 * SHADOW } else { 8.0 })?;
            s.SetOffset(Vector3 { X: 0.0, Y: if i == 0 { 24.0 * SHADOW } else { 2.0 }, Z: 0.0 })?;
            s.SetOpacity((if i == 0 { 0.32 } else { 0.12 }) * SHADOW)?;
        }
        Ok(())
    }

    fn resize(&mut self, width: f32, height: f32) -> Result<()> {
        if self.size == (width, height) {
            return Ok(());
        }
        self.size = (width, height);
        let size = Vector2 { X: width, Y: height };
        let radius = Vector2 { X: RADIUS, Y: RADIUS };
        self.root.SetSize(Vector2 { X: width + 2.0 * MARGIN, Y: height + MARGIN + MARGIN_BOTTOM })?;
        for g in [&self.mask_geometry, &self.clip] {
            g.SetSize(size)?;
            g.SetCornerRadius(radius)?;
        }
        self.mask.SetSize(size)?;
        self.mask_surface.SetSourceSize(size)?;
        for v in self.shadows.iter().chain([&self.material, &self.luminosity, &self.tint]) {
            v.SetSize(size)?;
        }
        self.edges.SetSize(Vector2 { X: width + 2.0, Y: height + 2.0 })?;
        self.outer.SetSize(Vector2 { X: width + 1.0, Y: height + 1.0 })?;
        self.outer.SetCornerRadius(Vector2 { X: RADIUS + 0.5, Y: RADIUS + 0.5 })?;
        self.inner.SetSize(Vector2 { X: width - 1.0, Y: height - 1.0 })?;
        self.inner.SetCornerRadius(Vector2 { X: RADIUS - 0.5, Y: RADIUS - 0.5 })?;
        self.root.SetCenterPoint(Vector3 { X: MARGIN + width / 2.0, Y: MARGIN + height / 2.0, Z: 0.0 })?;
        Ok(())
    }

    /// Puts the glass behind the panel whose client area is `x, y, width, height` in physical pixels,
    /// directly below `above` in the z-order.
    pub fn place(&mut self, x: i32, y: i32, width: i32, height: i32, dpi: f32, above: HWND) -> Result<()> {
        self.dpi = dpi;
        self.resize(width as f32 / dpi, height as f32 / dpi)?;
        self.root.SetTransformMatrix(Matrix4x4 { M11: dpi, M22: dpi, M33: 1.0, M44: 1.0, ..Default::default() })?;
        let (left, bottom) = ((MARGIN * dpi).round() as i32, (MARGIN_BOTTOM * dpi).round() as i32);
        unsafe {
            SetWindowPos(
                self.window,
                Some(above),
                x - left,
                y - left,
                width + 2 * left,
                height + left + bottom,
                SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            )
        }
    }

    fn ease(&self, curve: [f32; 4]) -> Result<windows::UI::Composition::CubicBezierEasingFunction> {
        self.compositor.CreateCubicBezierEasingFunction(
            Vector2 { X: curve[0], Y: curve[1] },
            Vector2 { X: curve[2], Y: curve[3] },
        )
    }

    /// Starts the open (spring scale from 0.94 and fade in) or close (shrink and fade out) animation from wherever
    /// the glass is now. Returns how long it runs, so the caller can hide its windows once it ends.
    pub fn animate(&mut self, show: bool, speed: f32) -> Result<u32> {
        let c = &self.compositor;
        if show && !self.shown {
            unsafe { let _ = ShowWindow(self.window, SW_SHOWNOACTIVATE); }
        }
        self.shown = show;
        let scale_ms = ((if show { SLOW_MS } else { FAST_MS }) as f32 * speed).max(1.0);
        let fade_ms = ((if show { NORMAL_MS } else { FAST_MS }) as f32 * speed).max(1.0);
        let to = if show { 1.0 } else { ENTER_SCALE };
        let grow = c.CreateVector3KeyFrameAnimation()?;
        grow.InsertExpressionKeyFrame(0.0, h!("this.StartingValue"))?;
        grow.InsertKeyFrameWithEasingFunction(1.0, Vector3 { X: to, Y: to, Z: 1.0 }, &self.ease(if show { SPRING } else { SMOOTH })?)?;
        grow.SetDuration(std::time::Duration::from_secs_f32(scale_ms / 1000.0).into())?;
        let fade = c.CreateScalarKeyFrameAnimation()?;
        fade.InsertExpressionKeyFrame(0.0, h!("this.StartingValue"))?;
        fade.InsertKeyFrameWithEasingFunction(1.0, if show { 1.0 } else { 0.0 }, &self.ease(SMOOTH)?)?;
        fade.SetDuration(std::time::Duration::from_secs_f32(fade_ms / 1000.0).into())?;
        self.root.StartAnimation(h!("Scale"), &grow)?;
        self.root.StartAnimation(h!("Opacity"), &fade)?;
        Ok(scale_ms.max(fade_ms) as u32)
    }

    /// Resets to the closed state (scaled down and transparent) without animating, ready for the next open.
    pub fn reset_hidden(&mut self) -> Result<()> {
        self.shown = false;
        self.root.StopAnimation(h!("Scale"))?;
        self.root.StopAnimation(h!("Opacity"))?;
        self.root.SetScale(Vector3 { X: ENTER_SCALE, Y: ENTER_SCALE, Z: 1.0 })?;
        self.root.SetOpacity(0.0)?;
        unsafe { let _ = ShowWindow(self.window, SW_HIDE); }
        Ok(())
    }

    /// Shows the glass at full size immediately, for places that must not animate (motion turned off).
    pub fn show_now(&mut self) -> Result<()> {
        self.shown = true;
        self.root.StopAnimation(h!("Scale"))?;
        self.root.StopAnimation(h!("Opacity"))?;
        self.root.SetScale(Vector3 { X: 1.0, Y: 1.0, Z: 1.0 })?;
        self.root.SetOpacity(1.0)?;
        unsafe { let _ = ShowWindow(self.window, SW_SHOWNOACTIVATE); }
        Ok(())
    }

    /// Hides the glass from the screen while it keeps composing (DWM cloak), used while content renders its first frame.
    pub fn cloak(&self, on: bool) {
        let value: i32 = on.into();
        unsafe {
            let _ = DwmSetWindowAttribute(self.window, DWMWA_CLOAK, &value as *const _ as _, 4);
        }
    }
}

impl Drop for Glass {
    fn drop(&mut self) {
        let _ = self.root.Children().and_then(|c| c.RemoveAll());
        unsafe {
            let _ = DestroyWindow(self.window);
        }
    }
}

trait Tap: Sized {
    fn tap(self, f: impl FnOnce(&Self) -> Result<()>) -> Result<Self> {
        f(&self)?;
        Ok(self)
    }
}
impl<T> Tap for T {}

// C ABI for Shanjie.UI's WinUI windows.

#[no_mangle]
pub extern "C" fn sj_glass_create(owner: isize) -> *mut Glass {
    let owner = (owner != 0).then_some(HWND(owner as _));
    match Glass::new(owner) {
        Ok(glass) => Box::into_raw(Box::new(glass)),
        Err(error) => {
            crate::bridge::failure(error.message());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn sj_glass_window(glass: *mut Glass) -> isize {
    unsafe { glass.as_ref() }.map_or(0, |g| g.window.0 as isize)
}

#[no_mangle]
pub extern "C" fn sj_glass_theme(glass: *mut Glass, dark: bool, solid: bool) -> bool {
    unsafe { glass.as_mut() }.is_some_and(|g| g.set_theme(dark, solid).is_ok())
}

#[no_mangle]
pub extern "C" fn sj_glass_place(glass: *mut Glass, x: i32, y: i32, width: i32, height: i32, dpi: f32, above: isize) -> bool {
    unsafe { glass.as_mut() }.is_some_and(|g| g.place(x, y, width, height, dpi, HWND(above as _)).is_ok())
}

/// `state`: 0 hide now, 1 animate open, 2 animate close, 3 show now. Returns the animation length in ms.
#[no_mangle]
pub extern "C" fn sj_glass_state(glass: *mut Glass, state: u32, speed: f32) -> u32 {
    let Some(g) = (unsafe { glass.as_mut() }) else { return 0 };
    match state {
        0 => g.reset_hidden().map(|_| 0),
        1 => g.animate(true, speed),
        2 => g.animate(false, speed),
        _ => g.show_now().map(|_| 0),
    }
    .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn sj_glass_cloak(glass: *mut Glass, on: bool) {
    if let Some(g) = unsafe { glass.as_ref() } {
        g.cloak(on);
    }
}

#[no_mangle]
pub extern "C" fn sj_glass_solid() -> bool {
    solid()
}

#[no_mangle]
pub extern "C" fn sj_glass_free(glass: *mut Glass) {
    if !glass.is_null() {
        drop(unsafe { Box::from_raw(glass) });
    }
}
