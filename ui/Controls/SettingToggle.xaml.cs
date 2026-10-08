using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
namespace Shanjie_UI.Controls;
public sealed partial class SettingToggle : UserControl
{
    public SettingToggle() => InitializeComponent();
    internal void SetToggleForVerification(bool value) => Toggle.IsOn = value;
    public static readonly DependencyProperty LabelProperty = DependencyProperty.Register(nameof(Label), typeof(string), typeof(SettingToggle), new PropertyMetadata(""));
    public static readonly DependencyProperty DescriptionProperty = DependencyProperty.Register(nameof(Description), typeof(string), typeof(SettingToggle), new PropertyMetadata("", DescriptionChanged));
    public static readonly DependencyProperty IsOnProperty = DependencyProperty.Register(nameof(IsOn), typeof(bool), typeof(SettingToggle), new PropertyMetadata(false));
    public static readonly DependencyProperty OnTextProperty = DependencyProperty.Register(nameof(OnText), typeof(string), typeof(SettingToggle), new PropertyMetadata("開啟"));
    public static readonly DependencyProperty OffTextProperty = DependencyProperty.Register(nameof(OffText), typeof(string), typeof(SettingToggle), new PropertyMetadata("關閉"));
    public string Label { get => (string)GetValue(LabelProperty); set => SetValue(LabelProperty, value); }
    public string Description { get => (string)GetValue(DescriptionProperty); set => SetValue(DescriptionProperty, value); }
    public bool IsOn { get => (bool)GetValue(IsOnProperty); set => SetValue(IsOnProperty, value); }
    public string OnText { get => (string)GetValue(OnTextProperty); set => SetValue(OnTextProperty, value); }
    public string OffText { get => (string)GetValue(OffTextProperty); set => SetValue(OffTextProperty, value); }
    private static void DescriptionChanged(DependencyObject sender, DependencyPropertyChangedEventArgs args)
        => ((SettingToggle)sender).DescriptionText.Visibility = string.IsNullOrEmpty((string)args.NewValue) ? Visibility.Collapsed : Visibility.Visible;
}
