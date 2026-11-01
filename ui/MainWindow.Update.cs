using Microsoft.UI.Xaml;
using Shanjie_UI.Setup;

namespace Shanjie_UI;

// 關於 → 更新：一列狀態加按鈕，有新版時下面列出更新說明。第一次打開「關於」時自動查一次，不在背景排程。
public sealed partial class MainWindow
{
    private UpdateService? updates;
    private CancellationTokenSource? updateWork;

    private void StartUpdateCheck()
    {
        if (updates is not null) return;
        updates = new UpdateService(UpdateService.RunningVersion(), UpdateService.DataFolder());
        updates.Changed += (_, _) => DispatcherQueue.TryEnqueue(RenderUpdate);
        Closed += (_, _) => { updateWork?.Cancel(); updates.Dispose(); };
        RenderUpdate();
        RunUpdate(updates.CheckAsync);
    }

    private void RunUpdate(Func<CancellationToken, Task> work)
    {
        updateWork?.Cancel();
        updateWork = new CancellationTokenSource();
        _ = Guard(work(updateWork.Token));
        // 失敗由服務放進 State／Error。
        static async Task Guard(Task task) { try { await task; } catch (Exception) { } }
    }

    private void RenderUpdate()
    {
        if (updates is not { } u) return;
        var latest = u.Latest;
        static string Size(long bytes) => $"{bytes / 1024.0 / 1024:0.#} MB";
        static string Name(UpdateRelease r) => r.Prerelease ? $"測試版 {r.Version}" : $"正式版 {r.Version}";
        UpdateRow.Description = u.State switch
        {
            UpdateState.Checking => $"目前 {u.CurrentVersion}，正在檢查…",
            UpdateState.UpToDate => $"目前 {u.CurrentVersion}，已是最新版",
            UpdateState.Available when latest is not null => $"有新版 {Name(latest)}（{Size(latest.SizeBytes)}，{latest.PublishedAt.LocalDateTime:M 月 d 日}發布）",
            UpdateState.Downloading when latest is not null => $"正在下載 {Name(latest)}：{u.DownloadProgress:P0}",
            UpdateState.Ready when latest is not null => $"{Name(latest)} 已下載並核對；安裝時會關閉設定，裝完自動開啟新版。個人詞庫、學習與設定都會保留",
            UpdateState.Failed => $"目前 {u.CurrentVersion}。{u.Error}",
            _ => $"目前 {u.CurrentVersion}",
        };
        UpdateButton.Content = u.State switch
        {
            UpdateState.Checking => "檢查中…",
            UpdateState.Available => "下載更新",
            UpdateState.Downloading => "下載中…",
            UpdateState.Ready => "安裝更新",
            UpdateState.Failed when latest is not null => "重試下載",
            _ => "檢查更新",
        };
        UpdateButton.Style = u.State is UpdateState.Available or UpdateState.Ready ? (Style)Application.Current.Resources["PaneAccentButton"] : null;
        UpdateButton.IsEnabled = u.State is not (UpdateState.Checking or UpdateState.Downloading);
        // 正式版與測試版都有時，另一個放在旁邊可以改選。
        var other = u.Choices.FirstOrDefault(c => c != latest);
        UpdateSwitch.Visibility = other is not null && u.State is UpdateState.Available or UpdateState.Ready or UpdateState.Failed ? Visibility.Visible : Visibility.Collapsed;
        UpdateSwitch.Content = other is null ? "" : $"改選{Name(other)}";
        UpdateDetails.Visibility = latest is not null ? Visibility.Visible : Visibility.Collapsed;
        UpdateNotes.Text = latest?.Notes ?? "";
        UpdateNotes.Visibility = latest is not null && latest.Notes.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    private void UpdateAction(object sender, RoutedEventArgs args)
    {
        if (updates is not { } u) return;
        switch (u.State)
        {
            case UpdateState.Available: RunUpdate(u.DownloadAsync); break;
            case UpdateState.Failed when u.Latest is not null: RunUpdate(u.DownloadAsync); break;
            case UpdateState.Ready: if (u.Install()) Close(); break;
            default: RunUpdate(u.CheckAsync); break;
        }
    }

    private void SwitchUpdate(object sender, RoutedEventArgs args)
    {
        if (updates is { } u && u.Choices.FirstOrDefault(c => c != u.Latest) is { } other) u.Choose(other);
    }

    private void OpenRelease(object sender, RoutedEventArgs args)
    {
        if (updates?.Latest?.PageUrl is not { } page) return;
        try { System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(page) { UseShellExecute = true })?.Dispose(); }
        catch (Exception) { }
    }
}
