using System.Numerics;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
namespace Shanjie_UI.Controls;

/// <summary>
/// Quick Setup's progress: a thin capsule track whose accent fill glides to the current step, drawn with composition
/// shapes so it never touches layout. The fill starts as a dot and reaches the end on the last step.
/// </summary>
public sealed partial class StepProgress : UserControl
{
    internal const float Length = 136, Thickness = 4;
    readonly Compositor compositor;
    readonly CompositionRoundedRectangleGeometry track, fill;
    readonly CompositionSpriteShape trackShape, fillShape;
    float shown = Thickness;

    public StepProgress()
    {
        IsTabStop = false;
        var host = new Grid { Width = Length, Height = Thickness, VerticalAlignment = VerticalAlignment.Center };
        Content = host;
        compositor = ElementCompositionPreview.GetElementVisual(host).Compositor;
        var radius = new Vector2(Thickness / 2);
        track = compositor.CreateRoundedRectangleGeometry(); track.Size = new Vector2(Length, Thickness); track.CornerRadius = radius;
        fill = compositor.CreateRoundedRectangleGeometry(); fill.Size = new Vector2(Thickness, Thickness); fill.CornerRadius = radius;
        trackShape = compositor.CreateSpriteShape(track); fillShape = compositor.CreateSpriteShape(fill);
        var visual = compositor.CreateShapeVisual(); visual.Size = new Vector2(Length, Thickness);
        visual.Shapes.Add(trackShape); visual.Shapes.Add(fillShape);
        ElementCompositionPreview.SetElementChildVisual(host, visual);
        Colors();
        ActualThemeChanged += (_, _) => Colors();
    }

    /// <param name="fraction">0 for the first step, 1 for the last.</param>
    internal void Set(double fraction, Choreography? timeline)
    {
        float to = Thickness + (float)Math.Clamp(fraction, 0, 1) * (Length - Thickness);
        if (timeline is null) { fill.StopAnimation("Size"); fill.Size = new Vector2(to, Thickness); }
        else timeline.Vector2(fill, "Size", new Vector2(shown, Thickness), new Vector2(to, Thickness), 40, 560, Motion.Glide);
        shown = to;
    }
    internal float FillForVerification => shown;

    void Colors()
    {
        var accent = Motion.AccentColor;
        bool dark = ActualTheme == ElementTheme.Dark;
        trackShape.FillBrush = compositor.CreateColorBrush(Motion.HighContrast ? Motion.WithAlpha(accent, 0.35) : dark ? Windows.UI.Color.FromArgb(0x29, 0xFF, 0xFF, 0xFF) : Windows.UI.Color.FromArgb(0x1A, 0, 0, 0));
        fillShape.FillBrush = compositor.CreateColorBrush(accent);
    }
}
