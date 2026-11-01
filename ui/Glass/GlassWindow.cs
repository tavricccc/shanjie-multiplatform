using System.Runtime.InteropServices;
using Microsoft.UI.Composition;
using Microsoft.UI.Content;
using Microsoft.UI.Input;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;
using Windows.UI.ViewManagement;
namespace Shanjie_UI.Glass;

/// <summary>
/// A Pane-style glass window. Two top-level windows: the content window (no redirection surface, so transparent
/// XAML really is transparent, clipped to a rounded rectangle) hosts the XAML through <see cref="DesktopWindowXamlSource"/>;
/// the glass window right behind it is drawn by Rust (<c>glass.rs</c>): blur, saturation, tint, highlight edge, shadows.
/// Opening scales the glass up with a slight spring while the content fades in; closing and minimizing shrink and fade out.
/// The content never scales: the system and WinUI compositors cannot share a frame, so only the glass moves.
/// </summary>
internal sealed partial class GlassWindow : IDisposable
{
    const double Radius = 20, EdgeDip = 8;
    const int NormalMs = 220, FastMs = 120;
    static readonly System.Numerics.Vector2 Smooth1 = new(0.2f, 0), Smooth2 = new(0, 1);
    static readonly List<GlassWindow> open = [];
    readonly FrameworkElement content;
    readonly DesktopWindowXamlSource island = new();
    readonly UISettings system = new();
    readonly WndProc proc;
    readonly string className = "ShanjieGlassHost" + Guid.NewGuid().ToString("N");
    readonly nint glass;
    readonly bool resizable;
    readonly double minWidth, minHeight;
    bool ready, shown, closing, minimizing, iconic, disposed, revealed;
    int lastWidth, lastHeight;

    internal nint Handle { get; }
    internal AppWindow AppWindow { get; }
    internal event EventHandler? Closed;
    /// <summary>Asked before a user close (title bar button, Alt+F4, taskbar); returning false keeps the window open.</summary>
    internal Func<bool> CanClose { get; set; } = () => true;
    float tint = SavedTint();
    /// <summary>Glass density: the tint layer's opacity, 0 to 1 (Pane's default 0.42).</summary>
    internal float Tint { get => tint; set { tint = Math.Clamp(value, 0, 1); ApplyTheme(); } }
    float Speed => system.AnimationsEnabled ? 1 : 0;
    float Dpi => GetDpiForWindow(Handle) / 96f;

    internal GlassWindow(FrameworkElement content, string title, double width, double height, bool resizable)
    {
        this.content = content; this.resizable = resizable; minWidth = width; minHeight = height;
        if (Font is { } font && content is Microsoft.UI.Xaml.Controls.Control control) control.FontFamily = font;
        proc = WindowProc;
        var module = GetModuleHandleW(null);
        var wc = new WndClass { Size = (uint)Marshal.SizeOf<WndClass>(), Proc = proc, Instance = module, Name = className, Icon = LoadIconW(module, 32512), Cursor = LoadCursorW(0, 32512) };
        if (RegisterClassExW(ref wc) == 0) throw new System.ComponentModel.Win32Exception();
        // WS_POPUP | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX; WS_EX_APPWINDOW | WS_EX_NOREDIRECTIONBITMAP.
        Handle = CreateWindowExW(0x240000, className, title, 0x800E0000, 0, 0, 1, 1, 0, 0, module, 0);
        if (Handle == 0) throw new System.ComponentModel.Win32Exception();
        int none = 1, noColor = unchecked((int)0xFFFFFFFE), noPeek = 1;
        DwmSetWindowAttribute(Handle, 33, ref none, 4);    // DWMWA_WINDOW_CORNER_PREFERENCE: the region rounds it
        DwmSetWindowAttribute(Handle, 34, ref noColor, 4); // DWMWA_BORDER_COLOR: none
        // Taskbar peek fades every other window, the glass included, leaving bare text; keep this window out of peek.
        DwmSetWindowAttribute(Handle, 11, ref noPeek, 4);
        AppWindow = AppWindow.GetFromWindowId(Microsoft.UI.Win32Interop.GetWindowIdFromWindow(Handle));

        Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread().EnsureSystemDispatcherQueue();
        glass = sj_glass_create(0);
        if (glass == 0) throw new InvalidOperationException(Marshal.PtrToStringUTF8(Interop.Native.sj_error()));

        island.Initialize(AppWindow.Id);
        island.Content = content;
        island.SystemBackdrop = new TransparentBackdrop();
        island.SiteBridge.Show();
        ready = true;
        content.ActualThemeChanged += (_, _) => ApplyTheme();
        system.AdvancedEffectsEnabledChanged += (_, _) => content.DispatcherQueue.TryEnqueue(ApplyTheme);
        system.ColorValuesChanged += (_, _) => content.DispatcherQueue.TryEnqueue(ApplyTheme);
        ApplyTheme();
        Center(width, height);
        open.Add(this);
    }

    void ApplyTheme() { if (!disposed) sj_glass_theme(glass, content.ActualTheme == ElementTheme.Dark, sj_glass_solid(), tint); }

    void Center(double width, double height)
    {
        GetCursorPos(out var cursor);
        var info = new MonitorInfo { Size = Marshal.SizeOf<MonitorInfo>() };
        GetMonitorInfoW(MonitorFromPoint(cursor, 2), ref info);
        GetDpiForMonitor(MonitorFromPoint(cursor, 2), 0, out uint dpi, out _);
        int w = (int)Math.Ceiling(width * dpi / 96), h = (int)Math.Ceiling(height * dpi / 96);
        var work = info.Work;
        SetWindowPos(Handle, 0, work.Left + Math.Max(0, (work.Right - work.Left - w) / 2), work.Top + Math.Max(0, (work.Bottom - work.Top - h) / 2), w, h, 0x14);
    }

    /// <summary>Starts the user-facing window: hidden (cloaked) until content and glass have drawn a frame, then opens.</summary>
    internal async void Show()
    {
        if (shown || disposed) return;
        shown = true;
        Cloak(true);
        sj_glass_state(glass, 0, 0);
        ContentVisual.Opacity = 0;
        ShowWindow(Handle, 5);
        Follow();
        Activate();
        // A fresh settings page takes a frame or two to draw; fading from the first moment would show only the end of it.
        var rendered = new TaskCompletionSource();
        void Rendered(object? sender, object e) { if (content.IsLoaded) rendered.TrySetResult(); }
        CompositionTarget.Rendering += Rendered;
        try { await Task.WhenAny(rendered.Task, Task.Delay(400)); await Task.Delay(16); }
        finally { CompositionTarget.Rendering -= Rendered; }
        if (disposed) return;
        revealed = true; Follow(); Cloak(false); Open();
    }

    /// <summary>Brings the window forward, restoring it if minimized.</summary>
    internal void Activate()
    {
        if (IsIconic(Handle)) ShowWindow(Handle, 9);
        SetForegroundWindow(Handle);
    }

    Visual ContentVisual => ElementCompositionPreview.GetElementVisual(content);

    void Open()
    {
        sj_glass_state(glass, 1, Speed);
        Fade(1, NormalMs);
    }

    int Fade(float to, int ms)
    {
        var visual = ContentVisual; var c = visual.Compositor;
        int duration = (int)Math.Max(1, ms * Speed);
        var fade = c.CreateScalarKeyFrameAnimation();
        fade.InsertExpressionKeyFrame(0, "this.StartingValue");
        fade.InsertKeyFrame(1, to, c.CreateCubicBezierEasingFunction(Smooth1, Smooth2));
        fade.Duration = TimeSpan.FromMilliseconds(duration);
        visual.StartAnimation("Opacity", fade);
        return duration;
    }

    async Task Shrink()
    {
        int ms = (int)sj_glass_state(glass, 2, Speed);
        await Task.Delay(Math.Max(ms, Fade(0, FastMs)));
    }

    /// <summary>Closes the window after the close animation.</summary>
    internal async void Close()
    {
        if (closing || disposed) return;
        closing = true;
        if (revealed && !IsIconic(Handle)) await Shrink();
        Dispose();
    }

    async void Minimize()
    {
        if (minimizing) return;
        minimizing = true;
        await Shrink();
        minimizing = false;
        if (!disposed) ShowWindow(Handle, 6);
    }

    internal void Minimize(bool animate) { if (animate && revealed && Speed > 0) Minimize(); else ShowWindow(Handle, 6); }

    /// <summary>Noto Sans when installed (looked up by <c>font.rs</c>), otherwise null for the system default.</summary>
    internal static FontFamily? Font { get; } = LoadFont();
    static FontFamily? LoadFont()
    {
        try { return Marshal.PtrToStringUTF8(sj_ui_font()) is { Length: > 0 } name ? new FontFamily(name) : null; }
        catch (Exception) { return null; }
    }

    FrameworkElement[] captions = [];
    RectInt32[] captionRects = [], passRects = [];
    /// <summary>Areas that move the window when dragged, handled by the system like a title bar so the window follows
    /// the pointer without the XAML thread in the loop; buttons inside them stay clickable.</summary>
    internal void SetCaption(params FrameworkElement[] areas)
    {
        captions = areas;
        foreach (var area in areas) area.SizeChanged += (_, _) => UpdateCaption();
        content.Loaded += (_, _) => UpdateCaption();
    }

    void UpdateCaption()
    {
        if (disposed || content.XamlRoot is null) return;
        double scale = content.XamlRoot.RasterizationScale;
        RectInt32 Bounds(FrameworkElement e)
        {
            var p = e.TransformToVisual(null).TransformPoint(default);
            return new((int)Math.Round(p.X * scale), (int)Math.Round(p.Y * scale), (int)Math.Round(e.ActualWidth * scale), (int)Math.Round(e.ActualHeight * scale));
        }
        static IEnumerable<FrameworkElement> Buttons(DependencyObject parent)
        {
            for (int i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
            {
                var child = VisualTreeHelper.GetChild(parent, i);
                if (child is Microsoft.UI.Xaml.Controls.Primitives.ButtonBase button) yield return button;
                else foreach (var inner in Buttons(child)) yield return inner;
            }
        }
        var shown = captions.Where(e => e.Visibility == Visibility.Visible && e.ActualWidth > 0).ToArray();
        captionRects = [.. shown.Select(Bounds)];
        passRects = [.. shown.SelectMany(Buttons).Where(b => b.Visibility == Visibility.Visible).Select(Bounds)];
        var source = InputNonClientPointerSource.GetForWindowId(AppWindow.Id);
        source.SetRegionRects(NonClientRegionKind.Caption, captionRects);
        source.SetRegionRects(NonClientRegionKind.Passthrough, passRects);
    }

    static bool Inside(RectInt32[] rects, int x, int y) => rects.Any(r => x >= r.X && x < r.X + r.Width && y >= r.Y && y < r.Y + r.Height);
    bool Caption(int x, int y) => Inside(captionRects, x, y) && !Inside(passRects, x, y);

    void Cloak(bool on) { int v = on ? 1 : 0; DwmSetWindowAttribute(Handle, 13, ref v, 4); sj_glass_cloak(glass, on); }

    /// <summary>Keeps the glass right behind the window, the island filling it, and the window clipped to the rounded rectangle.</summary>
    void Follow()
    {
        if (disposed || !GetWindowRect(Handle, out var r)) return;
        int w = r.Right - r.Left, h = r.Bottom - r.Top;
        if (shown) sj_glass_place(glass, r.Left, r.Top, w, h, Dpi, Handle);
        if (w == lastWidth && h == lastHeight) return;
        lastWidth = w; lastHeight = h;
        island.SiteBridge.MoveAndResize(new RectInt32(0, 0, w, h));
        int d = (int)Math.Round(2 * Radius * Dpi);
        var region = CreateRoundRectRgn(0, 0, w + 1, h + 1, d, d);
        if (SetWindowRgn(Handle, region, true) == 0) DeleteObject(region);
    }

    nint HitTest(nint l)
    {
        int x = (short)(l & 0xffff), y = (short)((l >> 16) & 0xffff);
        GetWindowRect(Handle, out var r);
        if (!resizable) return Caption(x - r.Left, y - r.Top) ? 2 : 1; // HTCAPTION
        int edge = (int)(EdgeDip * Dpi);
        bool left = x < r.Left + edge, right = x >= r.Right - edge, top = y < r.Top + edge, bottom = y >= r.Bottom - edge;
        if (top) return left ? 13 : right ? 14 : 12;
        if (bottom) return left ? 16 : right ? 17 : 15;
        if (left || right) return left ? 10 : 11;
        return Caption(x - r.Left, y - r.Top) ? 2 : 1;
    }

    nint WindowProc(nint h, uint m, nint w, nint l)
    {
        // Messages sent while the window is being created arrive before the island and glass exist.
        if (!ready) return m == 0x83 ? 0 : DefWindowProcW(h, m, w, l);
        switch (m)
        {
            case 0x83: return 0; // WM_NCCALCSIZE: no frame; the whole window is client area
            case 0x84: return HitTest(l);
            case 0x14: return 1; // WM_ERASEBKGND
            case 0x24 when l != 0: // WM_GETMINMAXINFO
                Marshal.WriteInt32(l, 24, (int)Math.Ceiling(minWidth * Dpi)); Marshal.WriteInt32(l, 28, (int)Math.Ceiling(minHeight * Dpi));
                return 0;
            case 0x02E0: // WM_DPICHANGED: take the suggested rectangle
            {
                var r = Marshal.PtrToStructure<Rect>(l);
                SetWindowPos(Handle, 0, r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top, 0x14);
                return 0;
            }
            case 0x112: // WM_SYSCOMMAND
                switch ((int)w & 0xFFF0)
                {
                    case 0xF020 when revealed && Speed > 0 && !IsIconic(Handle): Minimize(); return 0;
                    case 0xF030: return 0; // Pane's windows do not maximize
                    case 0xF060: if (!closing && CanClose()) Close(); return 0;
                }
                break;
            case 0x10: if (!closing && CanClose()) Close(); return 0; // WM_CLOSE
            case 0x46 when shown && l != 0: // WM_WINDOWPOSCHANGING: a move takes the glass along in the same frame
            {
                var p = Marshal.PtrToStructure<WindowPos>(l);
                if ((p.Flags & 0x3) == 0x1 && !IsIconic(Handle) && GetWindowRect(Handle, out var r)) // SWP_NOSIZE without SWP_NOMOVE
                    sj_glass_place(glass, p.X, p.Y, r.Right - r.Left, r.Bottom - r.Top, Dpi, Handle);
                break;
            }
            case 0x47: if (!IsIconic(Handle)) Follow(); break; // WM_WINDOWPOSCHANGED
            case 0x05: // WM_SIZE
                if (w == 1) { iconic = true; sj_glass_state(glass, 0, 0); }
                else if (iconic && revealed) { iconic = false; Follow(); ContentVisual.Opacity = 0; Open(); }
                break;
            case 0x06 when (w & 0xffff) != 0: Follow(); break; // WM_ACTIVATE: re-seat the glass behind the window
            case 0x07: // WM_SETFOCUS: hand focus to the XAML content
                if (!disposed) island.NavigateFocus(new XamlSourceFocusNavigationRequest(XamlSourceFocusNavigationReason.Restore));
                break;
        }
        return DefWindowProcW(h, m, w, l);
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        open.Remove(this);
        ShowWindow(Handle, 0);
        sj_glass_free(glass);
        Closed?.Invoke(this, EventArgs.Empty);
        island.Content = null; island.SystemBackdrop = null; island.Dispose();
        DestroyWindow(Handle);
        UnregisterClassW(className, GetModuleHandleW(null));
        if (open.Count == 0) Application.Current.Exit();
    }

    delegate nint WndProc(nint h, uint m, nint w, nint l);
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct WndClass { public uint Size, Style; public WndProc Proc; public int ClassExtra, WindowExtra; public nint Instance, Icon, Cursor, Background; public string? Menu; public string Name; public nint SmallIcon; }
    [StructLayout(LayoutKind.Sequential)] struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct Point { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] struct WindowPos { public nint Window, After; public int X, Y, Width, Height; public uint Flags; }
    [StructLayout(LayoutKind.Sequential)] struct MonitorInfo { public int Size; public Rect Monitor, Work; public uint Flags; }
    const string Rust = "shanjie_windows.dll";
    [DllImport(Rust)] static extern nint sj_glass_create(nint owner);
    [DllImport(Rust)] [return: MarshalAs(UnmanagedType.I1)] static extern bool sj_glass_theme(nint glass, [MarshalAs(UnmanagedType.I1)] bool dark, [MarshalAs(UnmanagedType.I1)] bool solid, float tint);
    [DllImport(Rust)] static extern float sj_glass_saved_tint();
    /// <summary>The saved strength from the profile, so setup and settings open with what the user chose.</summary>
    internal static float SavedTint() { try { return sj_glass_saved_tint(); } catch (Exception) { return 0.42f; } }
    [DllImport(Rust)] [return: MarshalAs(UnmanagedType.I1)] static extern bool sj_glass_place(nint glass, int x, int y, int width, int height, float dpi, nint above);
    [DllImport(Rust)] static extern uint sj_glass_state(nint glass, uint state, float speed);
    [DllImport(Rust)] static extern void sj_glass_cloak(nint glass, [MarshalAs(UnmanagedType.I1)] bool on);
    [DllImport(Rust)] [return: MarshalAs(UnmanagedType.I1)] static extern bool sj_glass_solid();
    [DllImport(Rust)] static extern void sj_glass_free(nint glass);
    [DllImport(Rust)] static extern nint sj_ui_font();
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern ushort RegisterClassExW(ref WndClass wc);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern bool UnregisterClassW(string name, nint instance);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern nint CreateWindowExW(uint ex, string cls, string title, uint style, int x, int y, int w, int h, nint parent, nint menu, nint instance, nint param);
    [DllImport("user32.dll")] static extern bool DestroyWindow(nint h);
    [DllImport("user32.dll")] static extern nint DefWindowProcW(nint h, uint m, nint w, nint l);
    [DllImport("user32.dll")] static extern nint LoadIconW(nint instance, nint name);
    [DllImport("user32.dll")] static extern nint LoadCursorW(nint instance, nint name);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern nint GetModuleHandleW(string? name);
    [DllImport("user32.dll")] static extern bool GetWindowRect(nint h, out Rect r);
    [DllImport("user32.dll")] static extern bool SetWindowPos(nint h, nint after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] static extern uint GetDpiForWindow(nint h);
    [DllImport("user32.dll")] static extern bool IsIconic(nint h);
    [DllImport("user32.dll")] static extern bool ShowWindow(nint h, int command);
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(nint h);
    [DllImport("user32.dll")] static extern int SetWindowRgn(nint h, nint region, bool redraw);
    [DllImport("user32.dll")] static extern bool GetCursorPos(out Point p);
    [DllImport("user32.dll")] static extern nint MonitorFromPoint(Point p, uint flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern bool GetMonitorInfoW(nint monitor, ref MonitorInfo info);
    [DllImport("shcore.dll")] static extern int GetDpiForMonitor(nint monitor, int type, out uint x, out uint y);
    [DllImport("gdi32.dll")] static extern nint CreateRoundRectRgn(int l, int t, int r, int b, int w, int h);
    [DllImport("gdi32.dll")] static extern bool DeleteObject(nint o);
    [DllImport("dwmapi.dll")] static extern int DwmSetWindowAttribute(nint h, int attribute, ref int value, int size);
}

/// <summary>Clears the island to transparent so the glass behind it shows through.</summary>
internal sealed partial class TransparentBackdrop : SystemBackdrop
{
    Windows.UI.Composition.Compositor? compositor;
    protected override void OnTargetConnected(ICompositionSupportsSystemBackdrop target, XamlRoot root)
    {
        base.OnTargetConnected(target, root);
        compositor = new();
        target.SystemBackdrop = compositor.CreateColorBrush(Windows.UI.Color.FromArgb(0, 0, 0, 0));
    }
    // The base implementation throws on a DesktopWindowXamlSource when activation or the theme changes; transparency never changes.
    protected override void OnDefaultSystemBackdropConfigurationChanged(ICompositionSupportsSystemBackdrop target, XamlRoot root) { }
    protected override void OnTargetDisconnected(ICompositionSupportsSystemBackdrop target)
    {
        target.SystemBackdrop = null; compositor?.Dispose(); compositor = null;
        base.OnTargetDisconnected(target);
    }
}
