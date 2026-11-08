using System.Numerics;
using Microsoft.UI.Composition;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.System;
namespace Shanjie_UI.Controls;

/// <summary>
/// One question's answers as a column of glass cards (radio buttons). The chosen card wears an accent outline that
/// glides from the previous choice, its check pops in with a spring and the card gives a small press; the arrow keys
/// move the choice as in any radio group. With Windows animations off everything moves at once.
/// </summary>
internal sealed partial class ChoiceCards : UserControl
{
    const double Radius = 14;
    readonly StackPanel column = new() { Spacing = 8 };
    readonly Canvas layer = new() { IsHitTestVisible = false };
    readonly List<RadioButton> cards = [];
    readonly List<Border> badges = [];
    readonly Compositor compositor;
    readonly CompositionRoundedRectangleGeometry outline;
    readonly CompositionSpriteShape shape;
    readonly ShapeVisual visual;
    readonly string group = Guid.NewGuid().ToString("N");
    bool placed, syncing;
    int selected = -1;
    internal event Action<int>? Chosen;

    internal ChoiceCards(Style card, IReadOnlyList<(string Title, string Description)> options, int index, string name)
    {
        IsTabStop = false;
        var grid = new Grid();
        grid.Children.Add(column); grid.Children.Add(layer);
        Content = grid;
        AutomationProperties.SetName(column, name);
        compositor = ElementCompositionPreview.GetElementVisual(layer).Compositor;
        outline = compositor.CreateRoundedRectangleGeometry();
        outline.CornerRadius = new Vector2((float)Radius);
        shape = compositor.CreateSpriteShape(outline); shape.StrokeThickness = 1.5f;
        visual = compositor.CreateShapeVisual(); visual.Size = new Vector2(4096, 4096); visual.Shapes.Add(shape); visual.Opacity = 0;
        ElementCompositionPreview.SetElementChildVisual(layer, visual);
        for (int i = 0; i < options.Count; i++) Add(card, options[i].Title, options[i].Description, i);
        Colors();
        ActualThemeChanged += (_, _) => Colors();
        Select(index, false);
        column.SizeChanged += (_, _) => Place(false);
    }

    void Add(Style style, string title, string description, int index)
    {
        var text = new StackPanel { Spacing = 3, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = title, FontSize = 15, FontWeight = FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap, Foreground = Motion.Resource("SjText1Brush") });
        if (description.Length > 0) text.Children.Add(new TextBlock { Text = description, FontSize = 12.5, LineHeight = 19, TextWrapping = TextWrapping.Wrap, Foreground = Motion.Resource("SjText2Brush") });
        // An empty ring that the accent check covers when chosen.
        var badge = new Border { Width = 20, Height = 20, CornerRadius = new CornerRadius(10), Background = new SolidColorBrush(Motion.AccentColor), Opacity = 0, Child = new Icon { Glyph = "check", Width = 12, Height = 12, Foreground = new SolidColorBrush(Microsoft.UI.Colors.White) } };
        var mark = new Grid { Width = 20, Height = 20, VerticalAlignment = VerticalAlignment.Center };
        mark.Children.Add(new Ellipse { Stroke = Motion.Resource("SjText3Brush"), StrokeThickness = 1.5 });
        mark.Children.Add(badge);
        var line = new Grid { ColumnSpacing = 14 };
        line.ColumnDefinitions.Add(new ColumnDefinition()); line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        line.Children.Add(text); Grid.SetColumn(mark, 1); line.Children.Add(mark);
        var card = new RadioButton { Style = style, GroupName = group, Content = line };
        AutomationProperties.SetName(card, description.Length > 0 ? $"{title}：{description}" : title);
        card.Checked += (_, _) => { if (!syncing) Select(index, true); };
        card.KeyDown += Arrow;
        cards.Add(card); badges.Add(badge); column.Children.Add(card);
    }

    internal int SelectedIndex => selected;
    internal Control Focusable => cards[Math.Max(0, selected)];
    internal int Count => cards.Count;
    internal void SetEnabled(bool enabled) { foreach (var card in cards) card.IsEnabled = enabled; }
    /// From the harness or a key: marks the card and tells the owner.
    internal void Choose(int index) { if (index != selected) cards[index].IsChecked = true; }

    void Select(int index, bool animate)
    {
        if (index < 0 || index >= cards.Count) return;
        int before = selected; selected = index;
        syncing = true; cards[index].IsChecked = true; syncing = false;
        for (int i = 0; i < badges.Count; i++) badges[i].Opacity = i == index ? 1 : 0;
        Place(animate && before >= 0);
        if (animate && Motion.Animated)
        {
            Pop(badges[index], 0.4f, Motion.Slow);
            Pop(cards[index], 0.975f, Motion.Slow);
        }
        if (before >= 0 && before != index) Chosen?.Invoke(index);
    }

    /// The outline sits on the chosen card; it glides there unless this is the first placement.
    void Place(bool animate)
    {
        if (selected < 0) return;
        var target = cards[selected];
        if (target.ActualWidth <= 0) return;
        var p = target.TransformToVisual(layer).TransformPoint(new Windows.Foundation.Point(0, 0));
        var offset = new Vector2((float)p.X, (float)p.Y);
        var size = new Vector2((float)target.ActualWidth, (float)target.ActualHeight);
        if (!placed || !animate || !Motion.Animated)
        {
            outline.StopAnimation("Offset"); outline.StopAnimation("Size");
            outline.Offset = offset; outline.Size = size; visual.Opacity = 1; placed = true;
            return;
        }
        var ease = Motion.Ease(compositor, Motion.Glide);
        var move = compositor.CreateVector2KeyFrameAnimation(); move.InsertKeyFrame(1, offset, ease); move.Duration = Motion.Duration(320);
        var grow = compositor.CreateVector2KeyFrameAnimation(); grow.InsertKeyFrame(1, size, ease); grow.Duration = Motion.Duration(320);
        outline.StartAnimation("Offset", move); outline.StartAnimation("Size", grow);
    }

    static void Pop(FrameworkElement element, float from, double ms)
    {
        var v = ElementCompositionPreview.GetElementVisual(element); var c = v.Compositor;
        v.CenterPoint = new Vector3((float)element.ActualWidth / 2, (float)element.ActualHeight / 2, 0);
        var grow = c.CreateVector3KeyFrameAnimation();
        grow.InsertKeyFrame(0, new Vector3(from, from, 1)); grow.InsertKeyFrame(1, Vector3.One, Motion.Ease(c, Motion.Spring));
        grow.Duration = Motion.Duration(ms);
        v.StartAnimation("Scale", grow);
    }

    void Arrow(object sender, KeyRoutedEventArgs args)
    {
        int move = args.Key switch { VirtualKey.Up or VirtualKey.Left => -1, VirtualKey.Down or VirtualKey.Right => 1, _ => 0 };
        if (move == 0) return;
        args.Handled = true;
        int next = cards.IndexOf((RadioButton)sender) + move;
        if (next < 0 || next >= cards.Count || !cards[next].IsEnabled) return;
        cards[next].IsChecked = true; cards[next].Focus(FocusState.Keyboard);
    }

    void Colors()
    {
        var accent = Motion.AccentColor;
        shape.StrokeBrush = compositor.CreateColorBrush(accent);
        shape.FillBrush = compositor.CreateColorBrush(Motion.HighContrast ? Microsoft.UI.Colors.Transparent : Motion.WithAlpha(accent, ActualTheme == ElementTheme.Dark ? 0.16 : 0.07));
        foreach (var badge in badges) badge.Background = new SolidColorBrush(accent);
    }
}
