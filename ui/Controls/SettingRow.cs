using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;
namespace Shanjie_UI.Controls;

/// <summary>
/// One row of a settings card: label (14) and optional description (11, tertiary) on the left, the control on
/// the right; at least 56 high, 14 / 10 padding. Rows after the first get a 1 px divider from <see cref="SettingCard"/>.
/// </summary>
[ContentProperty(Name = nameof(Control))]
public partial class SettingRow : UserControl
{
    public static readonly DependencyProperty LabelProperty = DependencyProperty.Register(nameof(Label), typeof(string), typeof(SettingRow), new PropertyMetadata("", (d, e) => ((SettingRow)d).label.Text = (string)e.NewValue));
    public static readonly DependencyProperty DescriptionProperty = DependencyProperty.Register(nameof(Description), typeof(string), typeof(SettingRow), new PropertyMetadata("", (d, e) => ((SettingRow)d).Describe((string)e.NewValue)));
    public static readonly DependencyProperty ControlProperty = DependencyProperty.Register(nameof(Control), typeof(UIElement), typeof(SettingRow), new PropertyMetadata(null, (d, e) => ((SettingRow)d).slot.Content = e.NewValue));
    public string Label { get => (string)GetValue(LabelProperty); set => SetValue(LabelProperty, value); }
    public string Description { get => (string)GetValue(DescriptionProperty); set => SetValue(DescriptionProperty, value); }
    public UIElement? Control { get => (UIElement?)GetValue(ControlProperty); set => SetValue(ControlProperty, value); }
    /// <summary>Puts the control under the text, full width (multi-line text boxes, lists).</summary>
    public bool Stacked { get; set; }
    readonly TextBlock label = new() { FontSize = 14, TextWrapping = TextWrapping.Wrap };
    readonly TextBlock description = new() { FontSize = 11, LineHeight = 18, Margin = new Thickness(0, 2, 0, 0), TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };
    readonly ContentPresenter slot = new() { VerticalAlignment = VerticalAlignment.Center };
    readonly Border divider = new() { BorderThickness = new Thickness(0) };

    public SettingRow()
    {
        IsTabStop = false;
        label.Foreground = Motion.Resource("SjText1Brush");
        description.Foreground = Motion.Resource("SjText3Brush");
        divider.BorderBrush = Motion.Resource("SjStrokeBrush");
        Content = divider;
        Loaded += (_, _) => Layout();
        IsEnabledChanged += (_, _) => label.Opacity = description.Opacity = IsEnabled ? 1 : 0.4;
    }

    void Describe(string text) { description.Text = text; description.Visibility = string.IsNullOrEmpty(text) ? Visibility.Collapsed : Visibility.Visible; }

    internal bool Divided { set => divider.BorderThickness = new Thickness(0, value ? 1 : 0, 0, 0); }

    bool laid;
    void Layout()
    {
        if (laid) return;
        laid = true;
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(label); text.Children.Add(description);
        var row = new Grid { MinHeight = 56, Padding = new Thickness(14, 10, 14, 10), ColumnSpacing = 16, RowSpacing = 8 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        bool bare = string.IsNullOrEmpty(Label) && string.IsNullOrEmpty(Description);
        if (!bare) row.Children.Add(text);
        // A row with only buttons (the add row) puts them on the left without an empty label line above.
        if (Stacked && bare) { Grid.SetColumnSpan(slot, 2); slot.HorizontalAlignment = HorizontalAlignment.Left; }
        else if (Stacked) { row.RowDefinitions.Add(new()); row.RowDefinitions.Add(new()); Grid.SetRow(slot, 1); Grid.SetColumnSpan(slot, 2); slot.HorizontalContentAlignment = HorizontalAlignment.Stretch; }
        else Grid.SetColumn(slot, 1);
        row.Children.Add(slot);
        divider.Child = row;
    }
}

/// <summary>A settings card: rounded (12) neutral fill with a thin stroke; dividers between its rows.</summary>
public sealed partial class SettingCard : StackPanel
{
    public SettingCard()
    {
        CornerRadius = new CornerRadius(12); BorderThickness = new Thickness(1);
        Background = Motion.Resource("SjFillControlBrush"); BorderBrush = Motion.Resource("SjStrokeBrush");
        Loaded += (_, _) =>
        {
            // Directly under a note (not a group title) a card needs its own gap.
            if (Parent is Panel panel && panel.Children.IndexOf(this) is > 0 and var i && panel.Children[i - 1] is TextBlock note && note.Style == Application.Current.Resources["SjNote"]) Margin = new Thickness(0, 12, 0, 0);
            bool first = true; foreach (var child in Children) if (child is SettingRow row && child.Visibility == Visibility.Visible) { row.Divided = !first; first = false; }
        };
    }
}
