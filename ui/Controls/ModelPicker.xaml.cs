using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Shanjie_UI.Models;
namespace Shanjie_UI.Controls;

public sealed partial class ModelPicker : UserControl
{
    public static readonly DependencyProperty ItemsSourceProperty = DependencyProperty.Register(nameof(ItemsSource), typeof(object), typeof(ModelPicker),
        new PropertyMetadata(null, (d, e) => ((ModelPicker)d).Choices.ItemsSource = e.NewValue));
    public static readonly DependencyProperty SelectedIndexProperty = DependencyProperty.Register(nameof(SelectedIndex), typeof(int), typeof(ModelPicker),
        new PropertyMetadata(-1, (d, e) => ((ModelPicker)d).Choices.SelectedIndex = (int)e.NewValue));
    public object? ItemsSource { get => GetValue(ItemsSourceProperty); set => SetValue(ItemsSourceProperty, value); }
    public int SelectedIndex { get => (int)GetValue(SelectedIndexProperty); set => SetValue(SelectedIndexProperty, value); }
    public object? SelectedItem => Choices.SelectedItem;
    public event SelectionChangedEventHandler? SelectionChanged;
    public ModelPicker() => InitializeComponent();

    private void Opening(object sender, object args) => Options.Width = Math.Max(0, PickerButton.ActualWidth - 32);
    private void Picked(object sender, ItemClickEventArgs args) => PickerFlyout.Hide();
    private void Changed(object sender, SelectionChangedEventArgs args)
    {
        SelectedIndex = Choices.SelectedIndex;
        SelectedName.Text = (Choices.SelectedItem as ModelChoice)?.Name ?? "選擇模型配置";
        PickerFlyout.Hide();
        SelectionChanged?.Invoke(this, args);
    }

    internal void CloseOptions() => PickerFlyout.Hide();
    internal void ShowOptions() => PickerFlyout.ShowAt(PickerButton);
    internal ListView OptionsForVerification => Choices;
}
