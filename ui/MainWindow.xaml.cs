using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Shanjie_UI.ViewModels;
using Windows.Graphics;
namespace Shanjie_UI;
public sealed partial class MainWindow : Window
{
    public SettingsViewModel Vm { get; } = new();
    public MainWindow()
    {
        InitializeComponent(); Bindings.Initialize(); ExtendsContentIntoTitleBar = true; SetTitleBar(AppTitleBar);
        double scale = Interop.Native.GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        AppWindow.Resize(new SizeInt32((int)(960 * scale), (int)(760 * scale)));
        Navigation.SelectedItem = Navigation.MenuItems[0]; ApplyTheme();
        Vm.PropertyChanged += (_, e) => { if (e.PropertyName == nameof(Vm.Theme)) ApplyTheme(); };
        Differences.Text = "標準／倚天、方向鍵選字、中英與全半形切換已接入。\n漢語／臺灣拼音、聯想詞、無效注音轉英數、使用者詞庫、IME 手寫／部首面板與舊版微軟相容模式仍需獨立引擎支援。";
    }
    private void ApplyTheme() => Root.RequestedTheme = (ElementTheme)Vm.Theme;
    internal FrameworkElement CaptureElement => Root;
    internal void ShowSection(int index) => Navigation.SelectedItem = Navigation.MenuItems[index];
    private void Navigate(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        var tag = (args.SelectedItem as NavigationViewItem)?.Tag as string;
        General.Visibility = tag == "general" ? Visibility.Visible : Visibility.Collapsed;
        Keys.Visibility = tag == "keys" ? Visibility.Visible : Visibility.Collapsed;
        Candidates.Visibility = tag == "candidates" ? Visibility.Visible : Visibility.Collapsed;
        About.Visibility = tag == "about" ? Visibility.Visible : Visibility.Collapsed;
    }
    private void LoadPreset(object sender, SelectionChangedEventArgs args)
    {
        int index = ((ComboBox)sender).SelectedIndex;
        if (index > 0) { Vm.Preset(index == 1); StatusBar.IsOpen = true; }
    }
    private void Reload(object sender, RoutedEventArgs args) { Vm.Load(); Preset.SelectedIndex = 0; StatusBar.IsOpen = false; }
    private void Cancel(object sender, RoutedEventArgs args) => Close();
    private void Save(object sender, RoutedEventArgs args)
    {
        try { Vm.Save(); StatusBar.Severity = InfoBarSeverity.Success; }
        catch (IOException error) { Vm.Status = "無法儲存設定：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        catch (UnauthorizedAccessException error) { Vm.Status = "無法寫入個人設定：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        StatusBar.IsOpen = true;
    }
}
