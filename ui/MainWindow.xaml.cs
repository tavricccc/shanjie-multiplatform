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
        Vm.PropertyChanged += (_, e) => {
            if (e.PropertyName == nameof(Vm.Theme)) ApplyTheme();
            if (e.PropertyName == nameof(Vm.HasChanges) && Vm.HasChanges && StatusBar.Severity == InfoBarSeverity.Success) StatusBar.IsOpen = false;
        };
        Differences.Text = "Windows 操作預設採微軟注音的常用鍵位，關閉即時預測、選字後聯想及善解的額外標點／聲調操作。載入預設後可逐項調整，再按儲存套用。\n候選順序仍由善解的詞庫、模型與學習決定。微軟的手寫／部首辨識、雲端功能、舊版引擎與私有詞庫格式不包含在善解中。";
        RefreshPresets(); ShowLoadFeedback();
    }
    private void ApplyTheme() => Root.RequestedTheme = (ElementTheme)Vm.Theme;
    internal FrameworkElement CaptureElement => Root;
    internal void SetPredictionForVerification(bool value) => PredictionToggle.SetToggleForVerification(value);
    internal void SetCapsLockForVerification(bool value) => CapsLockToggle.SetToggleForVerification(value);
    internal void SetMixedInputForVerification(bool mixed) => MixedInputToggle.SetToggleForVerification(mixed);
    internal void SetSpaceCommitsForVerification(bool value) => SpaceCommitsToggle.SetToggleForVerification(value);
    internal void SetSpaceOperatesForVerification(bool value) => SpaceOperatesToggle.SetToggleForVerification(value);
    internal void ShowSpaceOptions() { ShowSection(2); Root.UpdateLayout(); SpaceCommitsToggle.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0 }); }
    internal void ShowSection(int index) { Navigation.SelectedItem = Navigation.MenuItems[index]; ContentScroll.ChangeView(null, 0, null, true); }
    internal void ShowModelOptions() { ShowSection(0); Root.UpdateLayout(); ModelSelector.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0 }); }
    internal void ShowAdvancedKeys() { ShowSection(1); Root.UpdateLayout(); AdvancedKeys.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0 }); }
    internal void ShowCandidateKeys() { ShowSection(3); Root.UpdateLayout(); CandidateKeyOptions.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0 }); }
    internal void ShowAppearance() { ShowSection(3); Root.UpdateLayout(); AppearanceOptions.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0 }); }
    internal void ShowDictionary() { ShowSection(4); Root.UpdateLayout(); PersonalDictionaryEditor.StartBringIntoView(new BringIntoViewOptions { AnimationDesired = false, VerticalAlignmentRatio = 0 }); }
    internal void SetDictionaryForVerification(string text) => PersonalDictionaryEditor.Text = text;
    internal string DictionaryForVerification => PersonalDictionaryEditor.Text;
    internal (bool standard, bool eten) InstalledOptionsForVerification => (StandardKeyboardOption.IsEnabled, EtenKeyboardOption.IsEnabled);
    internal InfoBarSeverity FeedbackForVerification => StatusBar.Severity;
    internal void SetInstalledForVerification(uint mask) { Vm.ApplyInstalledKeyboards(mask); ShowLoadFeedback(); }
    private void ShowLoadFeedback() { StatusBar.Severity = InfoBarSeverity.Informational; StatusBar.IsOpen = !string.IsNullOrWhiteSpace(Vm.Status); }
    private void Navigate(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        var tag = (args.SelectedItem as NavigationViewItem)?.Tag as string;
        General.Visibility = tag == "general" ? Visibility.Visible : Visibility.Collapsed;
        Composition.Visibility = tag == "composition" ? Visibility.Visible : Visibility.Collapsed;
        Keys.Visibility = tag == "keys" ? Visibility.Visible : Visibility.Collapsed;
        Candidates.Visibility = tag == "candidates" ? Visibility.Visible : Visibility.Collapsed;
        Dictionary.Visibility = tag == "dictionary" ? Visibility.Visible : Visibility.Collapsed;
        About.Visibility = tag == "about" ? Visibility.Visible : Visibility.Collapsed;
        ContentScroll.ChangeView(null, 0, null, true);
    }
    private void ShiftAllOn(object sender, RoutedEventArgs args) => Vm.SetAllPunctuation(true, true);
    private void ShiftAllOff(object sender, RoutedEventArgs args) => Vm.SetAllPunctuation(true, false);
    private void PlainAllOn(object sender, RoutedEventArgs args) => Vm.SetAllPunctuation(false, true);
    private void PlainAllOff(object sender, RoutedEventArgs args) => Vm.SetAllPunctuation(false, false);
    private List<(string Name, string File)> savedPresets = [];
    private bool refreshingPresets;
    internal void RefreshPresets()
    {
        refreshingPresets = true;
        try {
            savedPresets = SettingsViewModel.SavedPresets();
            Preset.Items.Clear();
            foreach (var name in new[] { "選擇操作預設…", "Windows／微軟注音按鍵（預測關閉）", "善解上游操作" }.Concat(savedPresets.Select(p => p.Name))) Preset.Items.Add(name);
            Preset.SelectedIndex = 0;
        }
        finally { refreshingPresets = false; }
    }
    private void LoadPreset(object sender, SelectionChangedEventArgs args)
    {
        int index = Preset.SelectedIndex;
        if (refreshingPresets || index <= 0) return;
        if (index <= 2) LoadProfile(index == 1);
        else ShowResult(() => Vm.ApplySavedPreset(savedPresets[index - 3].File, savedPresets[index - 3].Name), InfoBarSeverity.Informational);
        refreshingPresets = true; Preset.SelectedIndex = 0; refreshingPresets = false;
    }
    private void ShowResult(Action action, InfoBarSeverity success)
    {
        try { action(); StatusBar.Severity = success; }
        catch (Exception error) when (error is InvalidDataException or IOException or UnauthorizedAccessException) { Vm.Status = error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        StatusBar.IsOpen = true;
    }
    private async void ImportPreset(object sender, RoutedEventArgs args)
    {
        var picker = new Windows.Storage.Pickers.FileOpenPicker(); picker.FileTypeFilter.Add(".ini");
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this));
        var file = await picker.PickSingleFileAsync(); if (file is null) return;
        string text;
        try { text = await File.ReadAllTextAsync(file.Path); }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException) { Vm.Status = "無法讀取操作預設：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; StatusBar.IsOpen = true; return; }
        ShowResult(() => Vm.ImportPreset(text, Path.GetFileNameWithoutExtension(file.Path)), InfoBarSeverity.Informational);
        RefreshPresets();
    }
    private async void ExportPreset(object sender, RoutedEventArgs args)
    {
        var picker = new Windows.Storage.Pickers.FileSavePicker { SuggestedFileName = "我的善解操作" };
        picker.FileTypeChoices.Add("善解操作預設", [".ini"]);
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this));
        var file = await picker.PickSaveFileAsync(); if (file is null) return;
        string? text = null;
        ShowResult(() => { text = Vm.ExportPreset(Path.GetFileNameWithoutExtension(file.Path)); Vm.Status = $"已匯出到 {file.Path}。檔案只包含按鍵與選字操作。"; }, InfoBarSeverity.Success);
        if (text is null) return;
        try { await File.WriteAllTextAsync(file.Path, text); }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException) { Vm.Status = "無法寫入檔案：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; StatusBar.IsOpen = true; }
    }
    internal void LoadProfile(bool windows)
    {
        Vm.Preset(windows); StatusBar.Severity = InfoBarSeverity.Informational; StatusBar.IsOpen = true;
    }
    private void Reload(object sender, RoutedEventArgs args) { Vm.Load(); RefreshPresets(); ShowLoadFeedback(); }
    private void Cancel(object sender, RoutedEventArgs args) => Close();
    private async void ClearLearning(object sender, RoutedEventArgs args)
    {
        var dialog = new ContentDialog { XamlRoot = Root.XamlRoot, RequestedTheme = Root.ActualTheme, Title = "清除選字學習？", Content = "會清除本帳號已儲存的選字偏好，個人詞庫與設定會保留。", PrimaryButtonText = "清除", CloseButtonText = "取消", DefaultButton = ContentDialogButton.Close };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary) return;
        await ClearLearningConfirmed();
    }
    internal async Task ClearLearningConfirmed()
    {
        try { await Task.Run(Models.ProfileStore.ClearLearning); Vm.Status = "已清除選字學習。"; StatusBar.Severity = InfoBarSeverity.Success; }
        catch (Exception error) when (error is InvalidDataException or IOException or UnauthorizedAccessException or TimeoutException) { Vm.Status = "無法清除學習：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        StatusBar.IsOpen = true;
    }
    private void Save(object sender, RoutedEventArgs args)
        => SaveDraft();
    private void Restart(object sender, RoutedEventArgs args) => RestartInput();
    internal void RestartInput()
    {
        if (Vm.HasChanges && !SaveDraft()) return;
        try { Vm.Load(); Models.ProfileStore.Restart(); Vm.Status = "已通知善解重新啟動並讀取已儲存設定。正在輸入的組字會先提交。"; StatusBar.Severity = InfoBarSeverity.Success; }
        catch (InvalidDataException error) { Vm.Status = "無法重新啟動輸入法：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        catch (IOException error) { Vm.Status = "無法讀取已儲存設定：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        catch (UnauthorizedAccessException error) { Vm.Status = "無法讀取個人設定：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        StatusBar.IsOpen = true;
    }
    internal bool SaveDraft()
    {
        bool saved = false;
        try { Vm.Save(); StatusBar.Severity = InfoBarSeverity.Success; saved = true; }
        catch (InvalidDataException error) { Vm.Status = error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        catch (IOException error) { Vm.Status = "無法儲存設定：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        catch (UnauthorizedAccessException error) { Vm.Status = "無法寫入個人設定：" + error.Message; StatusBar.Severity = InfoBarSeverity.Error; }
        StatusBar.IsOpen = true;
        return saved;
    }
}
