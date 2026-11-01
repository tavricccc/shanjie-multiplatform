using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.System;
namespace Shanjie_UI.Controls;

/// <summary>
/// Pane's segmented choice: neutral fill with a thin stroke and 2 px inset, equal segments 28 high; the selected segment's
/// text turns bold and primary, and the selected fill glides over.
/// </summary>
public sealed partial class Segmented : UserControl
{
    public static readonly DependencyProperty SelectedIndexProperty = DependencyProperty.Register(nameof(SelectedIndex), typeof(int), typeof(Segmented), new PropertyMetadata(0, (d, _) => ((Segmented)d).Selected(true)));
    public static readonly DependencyProperty OptionsProperty = DependencyProperty.Register(nameof(Options), typeof(string), typeof(Segmented), new PropertyMetadata("", (d, _) => ((Segmented)d).Build()));
    public int SelectedIndex { get => (int)GetValue(SelectedIndexProperty); set => SetValue(SelectedIndexProperty, value); }
    /// <summary>Segment labels separated by "|".</summary>
    public string Options { get => (string)GetValue(OptionsProperty); set => SetValue(OptionsProperty, value); }
    const double Radius = 10;
    readonly Canvas layer = new() { IsHitTestVisible = false };
    readonly Grid row = new();
    readonly List<Border> items = [];
    readonly SlidingHighlight highlight;

    public Segmented()
    {
        IsTabStop = true;
        var grid = new Grid();
        grid.Children.Add(layer); grid.Children.Add(row);
        var shell = new Grid();
        shell.Children.Add(new Border { Padding = new Thickness(2), CornerRadius = new CornerRadius(Radius), Background = Motion.Resource("PaneFillControlBrush"), BorderBrush = Motion.Resource("PaneStrokeBrush"), BorderThickness = new Thickness(1), Child = grid });
        Content = shell;
        FocusRing.Attach(this, shell, Radius);
        highlight = new SlidingHighlight(layer, Radius - 2);
        KeyDown += (_, e) =>
        {
            if (e.Key == VirtualKey.Left && SelectedIndex > 0) { SelectedIndex--; e.Handled = true; }
            if (e.Key == VirtualKey.Right && SelectedIndex < items.Count - 1) { SelectedIndex++; e.Handled = true; }
        };
        IsEnabledChanged += (_, _) => Opacity = IsEnabled ? 1 : 0.4;
        ActualThemeChanged += (_, _) => Text();
    }

    void Build()
    {
        row.Children.Clear(); row.ColumnDefinitions.Clear(); items.Clear();
        var labels = Options.Split('|');
        for (int i = 0; i < labels.Length; i++)
        {
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            var text = new TextBlock { Text = labels[i], FontSize = 12, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
            var item = new Border { Height = 28, Padding = new Thickness(10, 0, 10, 0), Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent), Child = text };
            AutomationProperties.SetName(item, labels[i]);
            Grid.SetColumn(item, i);
            int index = i;
            item.Tapped += (_, _) => { if (IsEnabled) SelectedIndex = index; };
            item.SizeChanged += (_, _) => { if (index == SelectedIndex) highlight.MoveTo(item, false); };
            items.Add(item); row.Children.Add(item);
        }
        Text();
    }

    void Selected(bool animate)
    {
        if (SelectedIndex >= 0 && SelectedIndex < items.Count) highlight.MoveTo(items[SelectedIndex], animate);
        Text();
    }

    void Text()
    {
        for (int i = 0; i < items.Count; i++)
        {
            var t = (TextBlock)items[i].Child;
            t.Foreground = Motion.Resource(i == SelectedIndex ? "PaneText1Brush" : "PaneText2Brush");
            t.FontWeight = i == SelectedIndex ? FontWeights.SemiBold : FontWeights.Normal;
        }
    }
}
