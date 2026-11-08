using System.Numerics;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Windows.UI;
using Windows.UI.ViewManagement;
namespace Shanjie_UI.Controls;

/// <summary>Motion tokens: durations in ms and cubic Bézier curves. Durations drop to 1 ms when Windows animations are off.</summary>
internal static class Motion
{
    static readonly UISettings settings = new();
    static readonly AccessibilitySettings accessibility = new();
    internal const double Fast = 120, Normal = 220, Slow = 360;
    internal static readonly (float, float, float, float) Smooth = (0.2f, 0, 0, 1), Spring = (0.34f, 1.36f, 0.64f, 1), Glide = (0.25f, 1.05f, 0.35f, 1), Exit = (0.4f, 0, 1, 1);
    /// Lets the harness check both states without changing the Windows setting.
    internal static bool? AnimationsForVerification { get; set; }
    /// The Windows "Animation effects" setting.
    internal static bool Animated => AnimationsForVerification ?? settings.AnimationsEnabled;
    internal static bool HighContrast => accessibility.HighContrast;
    internal static TimeSpan Duration(double ms) => TimeSpan.FromMilliseconds(Animated ? ms : 1);
    internal static CubicBezierEasingFunction Ease(Compositor c, (float, float, float, float) p) => c.CreateCubicBezierEasingFunction(new Vector2(p.Item1, p.Item2), new Vector2(p.Item3, p.Item4));
    /// <summary>Default accent (霧藍); only control states and focus use it.</summary>
    internal static SolidColorBrush Accent => (SolidColorBrush)Application.Current.Resources["SjAccentBrush"];
    internal static Brush Resource(string key) => (Brush)Application.Current.Resources[key];
    /// The accent, or the system highlight under high contrast.
    internal static Color AccentColor => HighContrast ? settings.UIElementColor(UIElementType.Highlight) : Accent.Color;
    internal static Color WithAlpha(Color color, double alpha) => Color.FromArgb((byte)Math.Round(alpha * 255), color.R, color.G, color.B);
}

/// <summary>
/// A choreography of composition animations that share one length, so a stagger lives in the key frames. With Windows
/// animations off every animation lasts 1 ms and lands on its end value. The harness can freeze it at any point:
/// RenderTargetBitmap does not see running composition animations, so a frozen moment stops them and writes the
/// values they would have at that point, by the same key frames and curves, as plain properties.
/// </summary>
internal sealed class Choreography
{
    readonly Compositor c;
    readonly float total;
    readonly bool animated = Motion.Animated;
    readonly List<Track> tracks = [];
    readonly CompositionScopedBatch batch;
    sealed record Track(CompositionObject Target, string Property, double Delay, double Ms, (float, float, float, float) Curve, Func<float, object> At, UIElement? Element);
    internal Choreography(Compositor compositor, double totalMs)
    {
        c = compositor; total = (float)totalMs;
        batch = c.CreateScopedBatch(CompositionBatchTypes.Animation);
    }
    internal static Visual Of(UIElement element, bool translation = true)
    {
        if (translation) ElementCompositionPreview.SetIsTranslationEnabled(element, true);
        return ElementCompositionPreview.GetElementVisual(element);
    }
    float At(double ms) => Math.Clamp((float)ms / total, 0, 1);
    T Frames<T>(T animation, double delay, double ms, Action<T, float, bool> insert) where T : KeyFrameAnimation
    {
        if (!animated) { insert(animation, 1, true); animation.Duration = TimeSpan.FromMilliseconds(1); return animation; }
        insert(animation, 0, false);
        if (delay > 0) insert(animation, At(delay), false);
        insert(animation, At(delay + ms), true);
        if (At(delay + ms) < 1) insert(animation, 1, true);
        animation.Duration = TimeSpan.FromMilliseconds(total);
        return animation;
    }
    void Start(CompositionObject target, string property, CompositionAnimation animation, Track track)
    {
        target.StartAnimation(property, animation);
        tracks.Add(track);
    }
    internal Choreography Scalar(CompositionObject target, string property, float from, float to, double delay, double ms, (float, float, float, float) curve, UIElement? element = null)
    {
        var ease = Motion.Ease(c, curve);
        Start(target, property, Frames(c.CreateScalarKeyFrameAnimation(), delay, ms, (a, t, end) => { if (end) a.InsertKeyFrame(t, to, ease); else a.InsertKeyFrame(t, from); }),
            new(target, property, delay, ms, curve, k => from + (to - from) * k, element));
        return this;
    }
    internal Choreography Vector2(CompositionObject target, string property, Vector2 from, Vector2 to, double delay, double ms, (float, float, float, float) curve)
    {
        var ease = Motion.Ease(c, curve);
        Start(target, property, Frames(c.CreateVector2KeyFrameAnimation(), delay, ms, (a, t, end) => { if (end) a.InsertKeyFrame(t, to, ease); else a.InsertKeyFrame(t, from); }),
            new(target, property, delay, ms, curve, k => System.Numerics.Vector2.Lerp(from, to, k), null));
        return this;
    }
    internal Choreography Vector3(CompositionObject target, string property, Vector3 from, Vector3 to, double delay, double ms, (float, float, float, float) curve, UIElement? element = null)
    {
        var ease = Motion.Ease(c, curve);
        Start(target, property, Frames(c.CreateVector3KeyFrameAnimation(), delay, ms, (a, t, end) => { if (end) a.InsertKeyFrame(t, to, ease); else a.InsertKeyFrame(t, from); }),
            new(target, property, delay, ms, curve, k => System.Numerics.Vector3.Lerp(from, to, k), element));
        return this;
    }
    /// An element's own visual rising or drifting in: translation from an offset and opacity from 0.
    internal Choreography Enter(UIElement element, Vector3 offset, double delay, double ms)
    {
        var v = Of(element);
        Vector3(v, "Translation", offset, System.Numerics.Vector3.Zero, delay, ms, Motion.Smooth, element);
        return Scalar(v, "Opacity", 0, 1, delay, ms * 0.8, Motion.Smooth, element);
    }
    internal Choreography Leave(UIElement element, Vector3 offset, double delay, double ms)
    {
        var v = Of(element);
        Vector3(v, "Translation", System.Numerics.Vector3.Zero, offset, delay, ms, Motion.Exit, element);
        return Scalar(v, "Opacity", 1, 0, delay, ms, Motion.Exit, element);
    }
    /// Runs once every animation has finished (immediately with animations off).
    internal void Then(Action done)
    {
        batch.Completed += (_, _) => done();
        batch.End();
    }

    /// Stops every animation and sets the values it would have <paramref name="progress"/> of the way through.
    internal int FreezeForVerification(float progress)
    {
        double now = progress * total;
        foreach (var track in tracks)
        {
            float local = track.Ms <= 0 ? 1 : (float)Math.Clamp((now - track.Delay) / track.Ms, 0, 1);
            object value = track.At(Bezier(track.Curve, local));
            track.Target.StopAnimation(track.Property);
            if (track.Element is { } element)
            {
                // The element's own visual is drawn by XAML for the capture, so the frozen value goes on XAML properties.
                if (track.Property == "Opacity") element.Opacity = (float)value;
                else if (value is Vector3 move) element.RenderTransform = new TranslateTransform { X = move.X, Y = move.Y };
                continue;
            }
            switch (track.Target, track.Property, value)
            {
                case (Visual v, "Opacity", float f): v.Opacity = f; break;
                case (Visual v, "Scale", Vector3 f): v.Scale = f; break;
                case (Visual v, "Offset", Vector3 f): v.Offset = f; break;
                case (CompositionGeometry g, "TrimEnd", float f): g.TrimEnd = f; break;
                case (CompositionRoundedRectangleGeometry g, "Size", Vector2 f): g.Size = f; break;
            }
        }
        return tracks.Count;
    }
    /// The eased fraction of a CSS-style cubic Bézier at time x.
    static float Bezier((float X1, float Y1, float X2, float Y2) p, float x)
    {
        if (x <= 0 || x >= 1) return x <= 0 ? 0 : 1;
        static float Curve(float a, float b, float t) => 3 * (1 - t) * (1 - t) * t * a + 3 * (1 - t) * t * t * b + t * t * t;
        float low = 0, high = 1, t = x;
        for (int i = 0; i < 40; i++) { t = (low + high) / 2; if (Curve(p.X1, p.X2, t) < x) low = t; else high = t; }
        return Curve(p.Y1, p.Y2, t);
    }
}

/// <summary>
/// Appear / dismiss motion for an element whose Visibility toggles (the status bar's template root): it rises 6 px and
/// fades in over Normal with the Smooth curve, and fades out over Fast. Windows animations off makes both instant.
/// </summary>
public static class StatusMotion
{
    public static readonly DependencyProperty IsAnimatedProperty = DependencyProperty.RegisterAttached("IsAnimated", typeof(bool), typeof(StatusMotion), new PropertyMetadata(false, (d, e) => { if (d is UIElement element && (bool)e.NewValue) Attach(element); }));
    public static bool GetIsAnimated(UIElement element) => (bool)element.GetValue(IsAnimatedProperty);
    public static void SetIsAnimated(UIElement element, bool value) => element.SetValue(IsAnimatedProperty, value);

    static void Attach(UIElement element)
    {
        ElementCompositionPreview.SetIsTranslationEnabled(element, true);
        var c = ElementCompositionPreview.GetElementVisual(element).Compositor;
        var ease = Motion.Ease(c, Motion.Smooth);
        var rise = c.CreateVector3KeyFrameAnimation();
        rise.Target = "Translation"; rise.InsertKeyFrame(0, new Vector3(0, 6, 0)); rise.InsertKeyFrame(1, Vector3.Zero, ease); rise.Duration = Motion.Duration(Motion.Normal);
        var fadeIn = c.CreateScalarKeyFrameAnimation();
        fadeIn.Target = "Opacity"; fadeIn.InsertKeyFrame(0, 0); fadeIn.InsertKeyFrame(1, 1, ease); fadeIn.Duration = Motion.Duration(Motion.Normal);
        var show = c.CreateAnimationGroup(); show.Add(rise); show.Add(fadeIn);
        var fadeOut = c.CreateScalarKeyFrameAnimation();
        fadeOut.Target = "Opacity"; fadeOut.InsertKeyFrame(0, 1); fadeOut.InsertKeyFrame(1, 0, ease); fadeOut.Duration = Motion.Duration(Motion.Fast);
        ElementCompositionPreview.SetImplicitShowAnimation(element, show);
        ElementCompositionPreview.SetImplicitHideAnimation(element, fadeOut);
    }
}

/// <summary>Keyboard focus ring: 2 px accent at 45 % outside the control, only for Tab / arrow focus.</summary>
internal static class FocusRing
{
    internal static void Attach(Control owner, Grid host, double radius)
    {
        owner.UseSystemFocusVisuals = false;
        var accent = Motion.Accent.Color;
        var ring = new Border
        {
            Margin = new Thickness(-3), CornerRadius = new CornerRadius(radius + 3), BorderThickness = new Thickness(2), IsHitTestVisible = false, Visibility = Visibility.Collapsed,
            BorderBrush = new SolidColorBrush(Windows.UI.Color.FromArgb(0x73, accent.R, accent.G, accent.B)),
        };
        host.Children.Add(ring);
        owner.GotFocus += (_, _) => ring.Visibility = owner.FocusState == FocusState.Keyboard ? Visibility.Visible : Visibility.Collapsed;
        owner.LostFocus += (_, _) => ring.Visibility = Visibility.Collapsed;
    }
}
