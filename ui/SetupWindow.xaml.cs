using System.Collections.ObjectModel;
using System.Diagnostics;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Shanjie_UI.Setup;
using Windows.Graphics;
namespace Shanjie_UI;

/// <summary>Install (from the package) or uninstall (from the installed copy) front end for shanjie-manager.</summary>
public sealed partial class SetupWindow : UserControl
{
    internal enum Page { Welcome, Keyboard, Uninstall, Progress, Done }
    private readonly bool uninstalling;
    private readonly ObservableCollection<SetupStep> steps = [];
    private SetupStatus? status;
    private SetupResult? result;
    private Page page;
    private bool running;
    private bool updating;

    public SetupWindow(bool uninstall)
    {
        uninstalling = uninstall;
        InitializeComponent();
        string title = uninstall ? "解除安裝善解" : "安裝善解";
        Host = new Glass.GlassWindow(this, title, 520, 600, resizable: false); TitleText.Text = title;
        Host.SetCaption(AppTitleBar);
        Steps.ItemsSource = steps;
        foreach (var (stage, label) in uninstall ? SetupStages.Uninstall : SetupStages.Install) steps.Add(new(stage, label));
        // Closing mid-install would hide the outcome; the manager itself still finishes or rolls back.
        Host.CanClose = () => !running;
        Closed += (_, _) => { if (uninstalling) UninstallCopy.RemoveAfterExit(); };
        Show(uninstall ? Page.Uninstall : Page.Welcome);
    }

    internal Glass.GlassWindow Host { get; }
    internal Microsoft.UI.Windowing.AppWindow AppWindow => Host.AppWindow;
    internal nint Handle => Host.Handle;
    internal event EventHandler? Closed { add => Host.Closed += value; remove => Host.Closed -= value; }
    internal void Activate() => Host.Activate();
    internal void Close() => Host.Close();
    private void MinimizeWindow(object sender, RoutedEventArgs args) => Host.Minimize(true);
    private void CloseWindow(object sender, RoutedEventArgs args) { if (Host.CanClose()) Host.Close(); }
    internal FrameworkElement CaptureElement => Root;
    internal void SetThemeForVerification(ElementTheme theme) => Root.RequestedTheme = theme;
    internal IReadOnlyList<SetupStep> StepsForVerification => steps;
    internal string PrimaryForVerification => PrimaryButton.Visibility == Visibility.Visible ? (string)PrimaryButton.Content : "";
    internal bool PrimaryEnabledForVerification => PrimaryButton.IsEnabled;
    internal uint KeyboardForVerification => KeyboardChoice.SelectedIndex == 1 ? 2u : 1u;

    internal async Task LoadStatus()
    {
        try { ShowStatus(await SetupRunner.Status()); }
        catch (Exception error)
        {
            StatusError.Message = error.Message; StatusError.IsOpen = true;
            if (!uninstalling) PrimaryButton.IsEnabled = false;
        }
    }

    internal void ShowStatus(SetupStatus value)
    {
        status = value; StatusError.IsOpen = false;
        WelcomeTitle.Text = $"安裝善解輸入法 {value.PackageVersion}";
        WelcomeText.Text = !value.Installed ? "這台電腦還沒有安裝善解，將進行全新安裝。"
            : value.InstalledVersion == value.PackageVersion ? $"已安裝相同版本 {value.InstalledVersion}，將重新安裝並修復註冊。"
            : $"已安裝 {value.InstalledVersion}，將更新到 {value.PackageVersion}。個人詞庫、選字學習與設定都會保留。";
        WelcomeTarget.Text = value.Target.Length == 0 ? "" : $"安裝位置：{value.Target}";
        KeyboardSummary.Text = value.KeyboardSummary;
        KeyboardChoice.SelectedIndex = value.SuggestedKeyboard == 2 ? 1 : 0;
        if (value.InstalledVersion is { } installed) UninstallTitle.Text = $"解除安裝善解輸入法 {installed}";
        PrimaryButton.IsEnabled = true;
    }

    internal void Show(Page next)
    {
        page = next;
        foreach (var (panel, which) in new (StackPanel, Page)[] { (WelcomePage, Page.Welcome), (KeyboardPage, Page.Keyboard), (UninstallPage, Page.Uninstall), (ProgressPage, Page.Progress), (DonePage, Page.Done) })
            panel.Visibility = which == next ? Visibility.Visible : Visibility.Collapsed;
        BackButton.Visibility = next == Page.Keyboard ? Visibility.Visible : Visibility.Collapsed;
        bool opensSettings = next == Page.Done && !uninstalling && result?.Outcome == SetupOutcome.Succeeded;
        SecondaryButton.Content = next == Page.Done ? "關閉" : "取消";
        SecondaryButton.Visibility = next == Page.Progress || (next == Page.Done && !opensSettings) ? Visibility.Collapsed : Visibility.Visible;
        PrimaryButton.Visibility = next == Page.Progress ? Visibility.Collapsed : Visibility.Visible;
        PrimaryButton.Content = next switch
        {
            Page.Welcome => "下一步",
            Page.Keyboard => "安裝",
            Page.Uninstall => "解除安裝",
            _ => opensSettings ? "開啟設定" : "關閉",
        };
        PrimaryButton.IsEnabled = next != Page.Welcome || status is not null;
    }

    private void Back(object sender, RoutedEventArgs args) => Show(Page.Welcome);
    private void Secondary(object sender, RoutedEventArgs args) => Close();
    private async void Primary(object sender, RoutedEventArgs args)
    {
        switch (page)
        {
            case Page.Welcome: Show(Page.Keyboard); break;
            case Page.Keyboard:
                uint keyboard = KeyboardChoice.SelectedIndex == 1 ? 2u : 1u;
                await Execute(async () =>
                {
                    await BundleExtraction.WaitReady(ShowExtraction);
                    return await SetupRunner.Install(keyboard, Apply);
                });
                break;
            case Page.Uninstall: await Execute(() => SetupRunner.Uninstall(DataChoice.SelectedIndex == 1, Apply)); break;
            case Page.Done when !uninstalling && result?.Outcome == SetupOutcome.Succeeded: OpenSettings(); break;
            default: Close(); break;
        }
    }

    /// <summary>The settings app's updater started this run: install with the keyboard already in use and reopen the
    /// settings when it succeeds; a failure stays on the done page like a manual install.</summary>
    internal async Task InstallUpdate()
    {
        if (status is null) return;
        updating = true;
        uint keyboard = status.SuggestedKeyboard == 2 ? 2u : 1u;
        await Execute(async () =>
        {
            await BundleExtraction.WaitReady(ShowExtraction);
            return await SetupRunner.Install(keyboard, Apply);
        });
        if (result?.Outcome == SetupOutcome.Succeeded) OpenSettings();
    }

    private async Task Execute(Func<Task<SetupResult>> operation)
    {
        running = true;
        ProgressTitle.Text = uninstalling ? "正在解除安裝" : updating ? "正在更新" : "正在安裝";
        Show(Page.Progress);
        SetupResult outcome;
        try { outcome = await operation(); }
        catch (Exception error) { outcome = new(SetupOutcome.Failed, error.Message); }
        finally { running = false; }
        Finish(outcome);
    }

    /// <summary>The single-file installer is still writing the models and data.</summary>
    private void ShowExtraction(double percent)
    {
        ProgressText.Text = "正在解開安裝檔案…";
        Progress.IsIndeterminate = false;
        Progress.Value = percent;
    }

    internal void Apply(SetupEvent item)
    {
        if (item.Stage == "Result") return;
        var step = steps.FirstOrDefault(s => s.Stage == item.Stage);
        if (step is null) { step = new(item.Stage, SetupStages.Label(item.Stage)); steps.Add(step); }
        step.State = item.Status switch { "succeeded" => StepState.Succeeded, "failed" => StepState.Failed, _ => StepState.Running };
        if (step.State == StepState.Running)
        {
            ProgressText.Text = step.Label + "…";
            bool counted = item.Done is { } done && item.Total is > 0;
            Progress.IsIndeterminate = !counted;
            if (counted) Progress.Value = 100.0 * item.Done!.Value / item.Total!.Value;
        }
    }

    internal void Finish(SetupResult outcome)
    {
        result = outcome;
        // A stopped run leaves its current step unfinished; show where it ended.
        foreach (var step in steps.Where(s => s.State == StepState.Running))
            step.State = outcome.Outcome == SetupOutcome.Succeeded ? StepState.Succeeded : StepState.Failed;
        (DoneTitle.Text, DoneSummary.Text) = (outcome.Outcome, uninstalling) switch
        {
            (SetupOutcome.Succeeded, false) => ("安裝完成", "善解已可使用"),
            (SetupOutcome.Succeeded, true) => ("已解除安裝", "善解已移除"),
            (SetupOutcome.Cancelled, _) => ("已取消", "沒有進行任何變更"),
            (_, false) => ("安裝未完成", "安裝失敗"),
            (_, true) => ("解除安裝未完成", "解除安裝失敗"),
        };
        // Done state: accent circle with a check; a failure shows the warning sign instead.
        bool ok = outcome.Outcome != SetupOutcome.Failed;
        DoneIcon.Glyph = ok ? "check" : "warn";
        DoneCircle.Background = ok ? Controls.Motion.Accent : Controls.Motion.Resource("SjFillControlBrush");
        DoneIcon.Foreground = ok ? new Microsoft.UI.Xaml.Media.SolidColorBrush(Microsoft.UI.Colors.White) : Controls.Motion.Resource("SjText1Brush");
        DoneHint.Text = outcome.Outcome == SetupOutcome.Succeeded && !uninstalling ? "請重新開啟正在使用輸入法的程式；登出再登入後，核心會在登入時自動啟動。" : "";
        DoneText.Text = outcome.Message;
        Show(Page.Done);
    }

    private void OpenSettings()
    {
        try
        {
            string target = status?.Target ?? "";
            string relative = File.ReadAllText(Path.Combine(target, "ui-path.txt")).Trim();
            Process.Start(new ProcessStartInfo(Path.Combine(target, relative)) { UseShellExecute = false, WorkingDirectory = target });
            Close();
        }
        catch (Exception error) { DoneText.Text = result?.Message + "\n\n設定程式未能開啟：" + error.Message; }
    }
}
