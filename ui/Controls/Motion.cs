using System.Numerics;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Windows.UI.ViewManagement;
namespace Shanjie_UI.Controls;

/// <summary>Motion tokens: durations in ms and cubic Bézier curves. Durations drop to 1 ms when Windows animations are off.</summary>
internal static class Motion
{
    static readonly UISettings settings = new();
    internal const double Fast = 120, Normal = 220, Slow = 360;
    internal static readonly (float, float, float, float) Smooth = (0.2f, 0, 0, 1), Spring = (0.34f, 1.36f, 0.64f, 1), Glide = (0.25f, 1.05f, 0.35f, 1);
    internal static TimeSpan Duration(double ms) => TimeSpan.FromMilliseconds(settings.AnimationsEnabled ? ms : 1);
    internal static CubicBezierEasingFunction Ease(Compositor c, (float, float, float, float) p) => c.CreateCubicBezierEasingFunction(new Vector2(p.Item1, p.Item2), new Vector2(p.Item3, p.Item4));
    /// <summary>Default accent (霧藍); only control states and focus use it.</summary>
    internal static SolidColorBrush Accent => (SolidColorBrush)Application.Current.Resources["SjAccentBrush"];
    internal static Brush Resource(string key) => (Brush)Application.Current.Resources[key];
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
