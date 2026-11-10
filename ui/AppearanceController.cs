using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Shanjie_UI.ViewModels;
using Windows.UI;
using Windows.UI.ViewManagement;
namespace Shanjie_UI;

/// The settings draft previews only this process. Saving uses the existing profile notification.
internal sealed class AppearanceController : IDisposable
{
    readonly FrameworkElement root;
    readonly Glass.GlassWindow host;
    readonly SettingsViewModel vm;
    readonly UISettings system = new();
    internal AppearanceController(FrameworkElement root, Glass.GlassWindow host, SettingsViewModel vm)
    {
        (this.root, this.host, this.vm) = (root, host, vm);
        system.ColorValuesChanged += ColorsChanged;
        host.SystemAppearanceChanged += Apply;
        root.ActualThemeChanged += ThemeChanged;
    }
    void ColorsChanged(UISettings sender, object args) => root.DispatcherQueue.TryEnqueue(Apply);
    void ThemeChanged(FrameworkElement sender, object args) => Apply();
    internal static Color ColorOf(uint rgb) => Color.FromArgb(255, (byte)(rgb >> 16), (byte)(rgb >> 8), (byte)rgb);
    internal void Apply()
    {
        string profile = vm.AppearanceDocument();
        var colors = System.Text.Json.JsonSerializer.Deserialize<Dictionary<string, uint>>(
            Interop.Native.Take(Interop.Native.sj_appearance_colors(profile, (uint)root.ActualTheme)))!;
        var resources = Application.Current.Resources;
        void Set(string key, string role) => ((SolidColorBrush)resources[key]).Color = ColorOf(colors[role]);
        Set("SjAccentBrush", "accent"); Set("SjAccentForegroundBrush", "foreground"); Set("SjAccentTextBrush", "text");
        void StatusColors(ResourceDictionary dictionary)
        {
            if (dictionary.ContainsKey("SjStatusInfoBrush")) ((SolidColorBrush)dictionary["SjStatusInfoBrush"]).Color=ColorOf(colors["text"]);
            foreach(string key in new[]{"SjStatusInfoTintBrush","SjStatusInfoStrokeBrush"}) if(dictionary.ContainsKey(key)) {
                var brush=(SolidColorBrush)dictionary[key];var color=ColorOf(colors["accent"]);color.A=brush.Color.A;brush.Color=color;
            }
            foreach(var merged in dictionary.MergedDictionaries) StatusColors(merged);
            foreach(var theme in dictionary.ThemeDictionaries.Values.OfType<ResourceDictionary>()) StatusColors(theme);
        }
        StatusColors(resources);
        Controls.Motion.AnimationMode = vm.AnimationMode;
        host.PureBlack = vm.PureBlack;
    }
    public void Dispose()
    {
        system.ColorValuesChanged -= ColorsChanged; host.SystemAppearanceChanged -= Apply;
        root.ActualThemeChanged -= ThemeChanged;
    }
}
