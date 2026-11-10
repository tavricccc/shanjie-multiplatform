using System.Numerics;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Windows.UI;
namespace Shanjie_UI.Controls;

/// <summary>
/// The finish: an accent ring draws itself, fills, a white check draws in two strokes, the badge gives a small spring,
/// a soft glow settles behind it and a ring of light and a few dots drift out and fade. All composition shapes on one
/// <see cref="Choreography"/>; with animations off it lands on the drawn check at once. High contrast keeps only the
/// ring and check, in system colors.
/// </summary>
internal sealed partial class FinishMark : UserControl
{
    internal const float Size = 96;
    const float Center = Size / 2, Radius = 40;
    readonly Compositor compositor;
    readonly ContainerVisual root, burst;
    readonly SpriteVisual? glow;
    readonly ShapeVisual disk, ring, check, wave;
    readonly CompositionEllipseGeometry ringGeometry;
    readonly CompositionLineGeometry shortLeg, longLeg;
    readonly List<(ShapeVisual Visual, Vector3 To)> dots = [];

    internal FinishMark()
    {
        IsTabStop = false;
        var host = new Grid { Width = Size, Height = Size };
        Content = host;
        compositor = ElementCompositionPreview.GetElementVisual(host).Compositor;
        root = compositor.CreateContainerVisual(); root.Size = new Vector2(Size); root.CenterPoint = new Vector3(Center, Center, 0);
        // The wave and the dots wait, hidden, until the check is drawn.
        burst = compositor.CreateContainerVisual(); burst.Size = new Vector2(Size); burst.Opacity = 0;
        bool contrast = Motion.HighContrast;
        Color accent = Motion.AccentColor;
        Color mark = contrast ? new Windows.UI.ViewManagement.UISettings().UIElementColor(Windows.UI.ViewManagement.UIElementType.HighlightText) : Motion.OnAccent.Color;
        if (!contrast)
        {
            var light = compositor.CreateRadialGradientBrush();
            light.ColorStops.Add(compositor.CreateColorGradientStop(0, Motion.WithAlpha(accent, 0.42)));
            light.ColorStops.Add(compositor.CreateColorGradientStop(0.55f, Motion.WithAlpha(accent, 0.12)));
            light.ColorStops.Add(compositor.CreateColorGradientStop(1, Motion.WithAlpha(accent, 0)));
            glow = compositor.CreateSpriteVisual(); glow.Size = new Vector2(200); glow.Offset = new Vector3(Center - 100, Center - 100, 0); glow.Brush = light; glow.Opacity = 0;
            root.Children.InsertAtBottom(glow);
        }
        ShapeVisual Circle(Color? fill, Color? stroke, float thickness, out CompositionEllipseGeometry geometry)
        {
            geometry = compositor.CreateEllipseGeometry(); geometry.Center = new Vector2(Center); geometry.Radius = new Vector2(Radius);
            var shape = compositor.CreateSpriteShape(geometry);
            if (fill is { } f) shape.FillBrush = compositor.CreateColorBrush(f);
            if (stroke is { } s) { shape.StrokeBrush = compositor.CreateColorBrush(s); shape.StrokeThickness = thickness; }
            var visual = compositor.CreateShapeVisual(); visual.Size = new Vector2(Size); visual.CenterPoint = new Vector3(Center, Center, 0); visual.Shapes.Add(shape);
            return visual;
        }
        wave = Circle(null, accent, 2, out _);
        disk = Circle(accent, null, 0, out _); disk.Opacity = 0;
        ring = Circle(null, accent, 3, out ringGeometry); ringGeometry.TrimEnd = 0;
        // The stroke starts at the top and runs clockwise.
        ring.RotationAngleInDegrees = -90;
        CompositionLineGeometry Leg(Vector2 start, Vector2 end)
        {
            var line = compositor.CreateLineGeometry(); line.Start = start; line.End = end; line.TrimEnd = 0;
            return line;
        }
        shortLeg = Leg(new(31, 49), new(42, 60)); longLeg = Leg(new(42, 60), new(65, 37));
        check = compositor.CreateShapeVisual(); check.Size = new Vector2(Size);
        foreach (var leg in new[] { shortLeg, longLeg })
        {
            var shape = compositor.CreateSpriteShape(leg);
            shape.StrokeBrush = compositor.CreateColorBrush(mark); shape.StrokeThickness = 5.5f;
            shape.StrokeStartCap = shape.StrokeEndCap = CompositionStrokeCap.Round;
            check.Shapes.Add(shape);
        }
        burst.Children.InsertAtTop(wave); root.Children.InsertAtTop(burst); root.Children.InsertAtTop(disk); root.Children.InsertAtTop(ring); root.Children.InsertAtTop(check);
        if (!contrast)
        {
            // Fixed positions, so every run and every screenshot is the same.
            Color[] tints = [accent, Color.FromArgb(0xFF, 0x8D, 0xA2, 0xE3), Color.FromArgb(0xFF, 0xB9, 0xC6, 0xEE), Color.FromArgb(0xFF, 0xF2, 0xC9, 0x6B)];
            for (int i = 0; i < 18; i++)
            {
                double angle = i * (Math.PI * 2 / 18) + (i % 2 == 0 ? 0.09 : -0.12);
                float distance = 60 + (i * 37 % 5) * 8, size = 5 + i * 7 % 4;
                var dot = compositor.CreateEllipseGeometry(); dot.Center = new Vector2(size / 2); dot.Radius = new Vector2(size / 2);
                var shape = compositor.CreateSpriteShape(dot); shape.FillBrush = compositor.CreateColorBrush(tints[i % tints.Length]);
                var visual = compositor.CreateShapeVisual(); visual.Size = new Vector2(size); visual.Shapes.Add(shape);
                visual.CenterPoint = new Vector3(size / 2, size / 2, 0);
                visual.Offset = new Vector3(Center - size / 2, Center - size / 2, 0);
                dots.Add((visual, new Vector3(Center - size / 2 + (float)Math.Cos(angle) * distance, Center - size / 2 + (float)Math.Sin(angle) * distance, 0)));
                burst.Children.InsertAtBottom(visual);
            }
        }
        ElementCompositionPreview.SetElementChildVisual(host, root);
    }

    /// Adds the whole moment to a timeline, starting <paramref name="start"/> ms in. About 1.6 s long.
    internal void Play(Choreography t, double start)
    {
        var smooth = Motion.Smooth;
        t.Scalar(ringGeometry, "TrimEnd", 0, 1, start, 520, smooth);
        t.Scalar(disk, "Opacity", 0, 1, start + 360, 200, smooth);
        t.Vector3(disk, "Scale", new Vector3(0.45f, 0.45f, 1), Vector3.One, start + 360, 560, Motion.Spring);
        t.Scalar(shortLeg, "TrimEnd", 0, 1, start + 620, 170, smooth);
        t.Scalar(longLeg, "TrimEnd", 0, 1, start + 770, 280, smooth);
        t.Vector3(root, "Scale", new Vector3(0.94f, 0.94f, 1), Vector3.One, start + 980, 460, Motion.Spring);
        if (glow is not null) t.Scalar(glow, "Opacity", 0, 0.8f, start + 420, 900, smooth);
        if (glow is null) return;
        t.Scalar(burst, "Opacity", 0, 1, start + 950, 1, smooth);
        t.Scalar(wave, "Opacity", 0.6f, 0, start + 960, 820, smooth);
        t.Vector3(wave, "Scale", Vector3.One, new Vector3(1.95f, 1.95f, 1), start + 960, 820, smooth);
        for (int i = 0; i < dots.Count; i++)
        {
            var (visual, to) = dots[i];
            double delay = start + 980 + i % 3 * 40;
            t.Vector3(visual, "Offset", visual.Offset, to, delay, 900, smooth);
            t.Scalar(visual, "Opacity", 1, 0, delay + 380, 560, smooth);
            t.Vector3(visual, "Scale", Vector3.One, new Vector3(0.4f, 0.4f, 1), delay, 940, smooth);
        }
    }
}
