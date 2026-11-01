using System.Numerics;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.UI.ViewManagement;
namespace Shanjie_UI.Controls;

/// <summary>Pane's motion tokens: durations in ms and cubic Bézier curves. Durations drop to 1 ms when Windows animations are off.</summary>
internal static class Motion
{
    static readonly UISettings settings = new();
    internal const double Fast = 120, Normal = 220, Slow = 360;
    internal static readonly (float, float, float, float) Smooth = (0.2f, 0, 0, 1), Spring = (0.34f, 1.36f, 0.64f, 1), Glide = (0.25f, 1.05f, 0.35f, 1);
    internal static TimeSpan Duration(double ms) => TimeSpan.FromMilliseconds(settings.AnimationsEnabled ? ms : 1);
    internal static CubicBezierEasingFunction Ease(Compositor c, (float, float, float, float) p) => c.CreateCubicBezierEasingFunction(new Vector2(p.Item1, p.Item2), new Vector2(p.Item3, p.Item4));
    /// <summary>Pane's default accent (霧藍); only control states and focus use it.</summary>
    internal static SolidColorBrush Accent => (SolidColorBrush)Application.Current.Resources["PaneAccentBrush"];
    internal static Brush Resource(string key) => (Brush)Application.Current.Resources[key];
}

/// <summary>Keyboard focus ring like Pane's: 2 px accent at 45 % outside the control, only for Tab / arrow focus.</summary>
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
