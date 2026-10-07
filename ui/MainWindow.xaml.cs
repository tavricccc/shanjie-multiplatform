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
        AppWindow.Resize(new SizeInt32((int)(840 * scale), (int)(680 * scale)));
        Navigation.SelectedItem = Navigation.MenuItems[0]; ApplyTheme();
        Vm.PropertyChanged += (_, e) => { if (e.PropertyName == nameof(Vm.Theme)) ApplyTheme(); };
        Differences.Text = "支援標準／倚天、漢語／臺灣拼音、聯想詞、選字學習、個人詞庫與字碼輸入。拼音以 1–5 輸入聲調；不帶聲調時按 Enter，空白可分隔音節，連按兩次空白轉換。\n善解使用自己的詞庫與語言模型，因此候選順序與微軟注音不同。微軟舊版相容模式與 IME 手寫／部首辨識屬於微軟的引擎功能。";
    }
    private void ApplyTheme() => Root.RequestedTheme = (ElementTheme)Vm.Theme;
    internal FrameworkElement CaptureElement => Root;
    internal void ShowSection(int index) => Navigation.SelectedItem = Navigation.MenuItems[index];
    internal void ShowModelOptions() { ShowSection(0); Root.UpdateLayout(); ModelSelector.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false }); }
    private void Navigate(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        var tag = (args.SelectedItem as NavigationViewItem)?.Tag as string;
        General.Visibility = tag == "general" ? Visibility.Visible : Visibility.Collapsed;
        Keys.Visibility = tag == "keys" ? Visibility.Visible : Visibility.Collapsed;
        Candidates.Visibility = tag == "candidates" ? Visibility.Visible : Visibility.Collapsed;
        Dictionary.Visibility = tag == "dictionary" ? Visibility.Visible : Visibility.Collapsed;
        About.Visibility = tag == "about" ? Visibility.Visible : Visibility.Collapsed;
    }
    private void LoadPreset(object sender, SelectionChangedEventArgs args)
    {
        int index = ((ComboBox)sender).SelectedIndex;
        if (index > 0) LoadProfile(index == 1);
    }
    internal void LoadProfile(bool windows)
    {
        Vm.Preset(windows); StatusBar.Severity = InfoBarSeverity.Informational; StatusBar.IsOpen = true;
    }
    private void Reload(object sender, RoutedEventArgs args) { Vm.Load(); Preset.SelectedIndex = 0; StatusBar.IsOpen = false; }
    private void Cancel(object sender, RoutedEventArgs args) => Close();
    private async void ClearLearning(object sender, RoutedEventArgs args)
    {
        var dialog = new ContentDialog { XamlRoot = Root.XamlRoot, RequestedTheme = Root.ActualTheme, Title = "清除選字學習？", Content = "會刪除本帳號已儲存的選字偏好，個人詞庫與設定會保留。", PrimaryButtonText = "清除", CloseButtonText = "取消", DefaultButton = ContentDialogButton.Close };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary) return;
        try { await Task.Run(Models.ProfileStore.ClearLearning); Vm.Status = "已清除選字學習。"; StatusBar.Severity = InfoBarSeverity.Success; }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or TimeoutException) { Vm.Status = "無法清除學習：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        StatusBar.IsOpen = true;
    }
    private void Save(object sender, RoutedEventArgs args)
        => SaveDraft();
    internal void SaveDraft()
    {
        try { Vm.Save(); StatusBar.Severity = InfoBarSeverity.Success; }
        catch (InvalidDataException error) { Vm.Status = error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        catch (IOException error) { Vm.Status = "無法儲存設定：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        catch (UnauthorizedAccessException error) { Vm.Status = "無法寫入個人設定：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        StatusBar.IsOpen = true;
    }
}
