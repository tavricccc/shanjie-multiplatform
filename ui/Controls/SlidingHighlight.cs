using System.Numerics;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Hosting;
using Windows.Foundation;
namespace Shanjie_UI.Controls;

/// <summary>
/// Pane's selection highlight: a rounded rectangle with a 1 px stroke on a layer under a list, gliding to the selected
/// item (220 ms, glide curve). The first placement jumps there instead of sliding in from elsewhere.
/// </summary>
internal sealed class SlidingHighlight
{
    readonly FrameworkElement layer;
    readonly Compositor compositor;
    readonly CompositionRoundedRectangleGeometry geometry;
    readonly CompositionSpriteShape shape;
    readonly ShapeVisual visual;
    readonly double radius;
    bool visible;

    /// <param name="layer">An empty element (a Canvas) under the list, in the list's coordinates.</param>
    internal SlidingHighlight(FrameworkElement layer, double radius)
    {
        this.layer = layer; this.radius = radius;
        compositor = ElementCompositionPreview.GetElementVisual(layer).Compositor;
        geometry = compositor.CreateRoundedRectangleGeometry();
        shape = compositor.CreateSpriteShape(geometry);
        shape.StrokeThickness = 1;
        visual = compositor.CreateShapeVisual();
        visual.Size = new Vector2(4096, 8192);
        visual.Shapes.Add(shape);
        visual.Opacity = 0;
        ElementCompositionPreview.SetElementChildVisual(layer, visual);
        layer.ActualThemeChanged += (_, _) => Colors();
        layer.Loaded += (_, _) => Colors();
        Colors();
    }

    internal void MoveTo(FrameworkElement target, bool animate = true)
    {
        if (target.ActualWidth <= 0) return;
        var p = target.TransformToVisual(layer).TransformPoint(new Point(0, 0));
        var offset = new Vector2((float)p.X, (float)p.Y);
        var size = new Vector2((float)target.ActualWidth, (float)target.ActualHeight);
        geometry.CornerRadius = new Vector2((float)Math.Min(radius, Math.Min(size.X, size.Y) / 2));
        if (!visible || !animate)
        {
            geometry.Offset = offset; geometry.Size = size; visual.Opacity = 1; visible = true;
            return;
        }
        var ease = Motion.Ease(compositor, Motion.Glide);
        var move = compositor.CreateVector2KeyFrameAnimation();
        move.InsertKeyFrame(1, offset, ease); move.Duration = Motion.Duration(Motion.Normal);
        var grow = compositor.CreateVector2KeyFrameAnimation();
        grow.InsertKeyFrame(1, size, ease); grow.Duration = Motion.Duration(Motion.Normal);
        geometry.StartAnimation("Offset", move);
        geometry.StartAnimation("Size", grow);
    }

    void Colors()
    {
        var theme = (ResourceDictionary)((ResourceDictionary)Application.Current.Resources.MergedDictionaries.First(d => d.ThemeDictionaries.ContainsKey("Light") && d.ThemeDictionaries.ContainsKey("Dark") && ((ResourceDictionary)d.ThemeDictionaries["Light"]).ContainsKey("PaneStrokeColor")))
            .ThemeDictionaries[layer.ActualTheme == ElementTheme.Dark ? "Dark" : "Light"];
        shape.FillBrush = compositor.CreateColorBrush((Windows.UI.Color)theme["PaneFillSelectedColor"]);
        shape.StrokeBrush = compositor.CreateColorBrush((Windows.UI.Color)theme["PaneStrokeColor"]);
    }
}
