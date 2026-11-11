using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
namespace Shanjie_UI.Controls;

/// <summary>A settings row with a switch; the state text (開啟／關閉 or a pair like 中文／英文) sits left of it.</summary>
public sealed partial class SettingToggle : SettingRow
{
    public static readonly DependencyProperty IsOnProperty = DependencyProperty.Register(nameof(IsOn), typeof(bool), typeof(SettingToggle), new PropertyMetadata(false, (d, _) => ((SettingToggle)d).Sync()));
    public static readonly DependencyProperty OnTextProperty = DependencyProperty.Register(nameof(OnText), typeof(string), typeof(SettingToggle), new PropertyMetadata("", (d, _) => ((SettingToggle)d).Sync()));
    public static readonly DependencyProperty OffTextProperty = DependencyProperty.Register(nameof(OffText), typeof(string), typeof(SettingToggle), new PropertyMetadata("", (d, _) => ((SettingToggle)d).Sync()));
    public bool IsOn { get => (bool)GetValue(IsOnProperty); set => SetValue(IsOnProperty, value); }
    public string OnText { get => (string)GetValue(OnTextProperty); set => SetValue(OnTextProperty, value); }
    public string OffText { get => (string)GetValue(OffTextProperty); set => SetValue(OffTextProperty, value); }
    readonly Switch toggle = new();
    readonly TextBlock state = new() { FontSize = 12, VerticalAlignment = VerticalAlignment.Center };

    public SettingToggle()
    {
        state.Foreground = Motion.Resource("SjText2Brush");
        var pair = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        pair.Children.Add(state); pair.Children.Add(toggle);
        Control = pair;
        toggle.RegisterPropertyChangedCallback(Switch.IsOnProperty, (_, _) => IsOn = toggle.IsOn);
        RegisterPropertyChangedCallback(LabelProperty, (_, _) => AutomationProperties.SetName(toggle, Label));
        Loaded += (_, _) => SyncEnabled();
        IsEnabledChanged += (_, _) => SyncEnabled();
    }

    void SyncEnabled()
    {
        toggle.IsEnabled = IsEnabled;
        state.Opacity = IsEnabled ? 1 : 0.4;
    }

    internal void SetToggleForVerification(bool value) => toggle.IsOn = value;

    void Sync()
    {
        toggle.IsOn = IsOn;
        state.Text = IsOn ? OnText : OffText;
        state.Visibility = string.IsNullOrEmpty(state.Text) ? Visibility.Collapsed : Visibility.Visible;
    }
}
