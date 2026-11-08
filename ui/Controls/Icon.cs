using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;
using Microsoft.UI.Xaml.Media;
using Path = Microsoft.UI.Xaml.Shapes.Path;
namespace Shanjie_UI.Controls;

/// <summary>A Lucide stroke icon (24 × 24, line width 1.75, scaled with the control).</summary>
public sealed partial class Icon : UserControl
{
    // Lucide (ISC License, see LICENSES/lucide.txt), 24 × 24 stroke paths.
    static readonly Dictionary<string, string> Paths = new()
    {
        ["close"] = "M 18 6 L 6 18 M 6 6 l 12 12",
        ["minus"] = "M 5 12 h 14",
        ["gear"] = "M 9.671 4.136 a 2.34 2.34 0 0 1 4.659 0 a 2.34 2.34 0 0 0 3.319 1.915 a 2.34 2.34 0 0 1 2.33 4.033 a 2.34 2.34 0 0 0 0 3.831 a 2.34 2.34 0 0 1 -2.33 4.033 a 2.34 2.34 0 0 0 -3.319 1.915 a 2.34 2.34 0 0 1 -4.659 0 a 2.34 2.34 0 0 0 -3.32 -1.915 a 2.34 2.34 0 0 1 -2.33 -4.033 a 2.34 2.34 0 0 0 0 -3.831 A 2.34 2.34 0 0 1 6.35 6.051 a 2.34 2.34 0 0 0 3.319 -1.915 M 9 12 A 3 3 0 1 0 15 12 A 3 3 0 1 0 9 12 Z",
        ["globe"] = "M 2 12 A 10 10 0 1 0 22 12 A 10 10 0 1 0 2 12 Z M 12 2 a 14.5 14.5 0 0 0 0 20 a 14.5 14.5 0 0 0 0 -20 M 2 12 h 20",
        ["keyboard"] = "M 10 8 h 0.01 M 12 12 h 0.01 M 14 8 h 0.01 M 16 12 h 0.01 M 18 8 h 0.01 M 6 8 h 0.01 M 7 16 h 10 M 8 12 h 0.01 M 4 4 H 20 A 2 2 0 0 1 22 6 V 18 A 2 2 0 0 1 20 20 H 4 A 2 2 0 0 1 2 18 V 6 A 2 2 0 0 1 4 4 Z",
        ["case"] = "M 2 16 l 4.039 -9.69 a 0.5 0.5 0 0 1 0.923 0 L 11 16 M 22 9 v 7 M 3.304 13 h 6.392 M 15 12.5 A 3.5 3.5 0 1 0 22 12.5 A 3.5 3.5 0 1 0 15 12.5 Z",
        ["pencil"] = "M 21.174 6.812 a 1 1 0 0 0 -3.986 -3.987 L 3.842 16.174 a 2 2 0 0 0 -0.5 0.83 l -1.321 4.352 a 0.5 0.5 0 0 0 0.623 0.622 l 4.353 -1.32 a 2 2 0 0 0 0.83 -0.497 z M 15 5 l 4 4",
        ["list"] = "M 3 5 h 0.01 M 3 12 h 0.01 M 3 19 h 0.01 M 8 5 h 13 M 8 12 h 13 M 8 19 h 13",
        ["palette"] = "M 11 17 a 4 4 0 0 1 -8 0 V 5 a 2 2 0 0 1 2 -2 h 4 a 2 2 0 0 1 2 2 Z M 16.7 13 H 19 a 2 2 0 0 1 2 2 v 4 a 2 2 0 0 1 -2 2 H 7 M 7 17 h 0.01 M 11 8 l 2.3 -2.3 a 2.4 2.4 0 0 1 3.404 0.004 L 18.6 7.6 a 2.4 2.4 0 0 1 0.026 3.434 L 9.9 19.8",
        ["bookmark"] = "M 17 3 a 2 2 0 0 1 2 2 v 15 a 1 1 0 0 1 -1.496 0.868 l -4.512 -2.578 a 2 2 0 0 0 -1.984 0 l -4.512 2.578 A 1 1 0 0 1 5 20 V 5 a 2 2 0 0 1 2 -2 z",
        ["info"] = "M 2 12 A 10 10 0 1 0 22 12 A 10 10 0 1 0 2 12 Z M 12 16 v -4 M 12 8 h 0.01",
        ["check"] = "M 20 6 L 9 17 l -5 -5",
        ["circle-check"] = "M 2 12 A 10 10 0 1 0 22 12 A 10 10 0 1 0 2 12 Z M 9 12 l 2 2 l 4 -4",
        ["circle-x"] = "M 2 12 A 10 10 0 1 0 22 12 A 10 10 0 1 0 2 12 Z M 15 9 l -6 6 M 9 9 l 6 6",
        ["warn"] = "M 21.73 18 l -8 -14 a 2 2 0 0 0 -3.48 0 l -8 14 A 2 2 0 0 0 4 21 h 16 a 2 2 0 0 0 1.73 -3 M 12 9 v 4 M 12 17 h 0.01",
        ["chev-d"] = "M 6 9 l 6 6 l 6 -6",
        ["shield"] = "M 20 13 c 0 5 -3.5 7.5 -7.66 8.95 a 1 1 0 0 1 -0.67 -0.01 C 7.5 20.5 4 18 4 13 V 6 a 1 1 0 0 1 1 -1 c 2 0 4.5 -1.2 6.24 -2.72 a 1.17 1.17 0 0 1 1.52 0 C 14.51 3.81 17 5 19 5 a 1 1 0 0 1 1 1 z M 9 12 l 2 2 l 4 -4",
        ["trash"] = "M 10 11 v 6 M 14 11 v 6 M 19 6 v 14 a 2 2 0 0 1 -2 2 H 7 a 2 2 0 0 1 -2 -2 V 6 M 3 6 h 18 M 8 6 V 4 a 2 2 0 0 1 2 -2 h 4 a 2 2 0 0 1 2 2 v 2",
    };
    public static readonly DependencyProperty GlyphProperty = DependencyProperty.Register(nameof(Glyph), typeof(string), typeof(Icon), new PropertyMetadata(null, (d, _) => ((Icon)d).Update()));
    public string? Glyph { get => (string?)GetValue(GlyphProperty); set => SetValue(GlyphProperty, value); }
    readonly Path path = new() { StrokeStartLineCap = PenLineCap.Round, StrokeEndLineCap = PenLineCap.Round, StrokeLineJoin = PenLineJoin.Round, StrokeThickness = 1.75 };

    public Icon()
    {
        IsTabStop = false;
        var canvas = new Canvas { Width = 24, Height = 24 };
        canvas.Children.Add(path);
        Content = new Viewbox { Stretch = Stretch.Uniform, Child = canvas };
        // Follows the inherited foreground (a button's presenter, hover states) unless one is set here.
        path.SetBinding(Path.StrokeProperty, new Microsoft.UI.Xaml.Data.Binding { Source = this, Path = new PropertyPath(nameof(Foreground)) });
    }

    // A Geometry cannot be shared between elements; each icon parses its own.
    void Update() => path.Data = Glyph is { } g && Paths.TryGetValue(g, out var data) ? (Geometry)XamlBindingHelper.ConvertValue(typeof(Geometry), data) : null;
}
