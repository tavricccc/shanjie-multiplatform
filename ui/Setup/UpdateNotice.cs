using System.Diagnostics;
using System.Globalization;
using System.Runtime.InteropServices;

namespace Shanjie_UI.Setup;

/// <summary>
/// 背景更新通知的選擇：自動檢查開關（update-auto.txt）與每個版本的通知紀錄（update-notice.txt）。
/// 兩者都跟 update-channel.txt 一樣獨立於與核心共用的 profile.ini。「稍後」隔天再提醒，關掉或查看後這一版不再通知，出更新的版本會再通知。
/// </summary>
internal sealed class UpdateNotice(string data)
{
    static readonly TimeSpan Snooze = TimeSpan.FromDays(1);
    static Mutex? single;
    readonly string autoFile = Path.Combine(data, "update-auto.txt"), noticeFile = Path.Combine(data, "update-notice.txt");

    internal bool Enabled
    {
        get
        {
            try { return !File.Exists(autoFile) || File.ReadAllText(autoFile).Trim() != "off"; }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException) { return true; }
        }
        set => Save(autoFile, value ? "on" : "off");
    }

    internal bool ShouldShow(string version, DateTimeOffset now)
    {
        string[] saved;
        try { if (!File.Exists(noticeFile)) return true; saved = File.ReadAllLines(noticeFile); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { return true; }
        if (saved.Length != 2 || saved[0] != version) return true;
        if (saved[1] == "dismissed") return false;
        return !DateTimeOffset.TryParse(saved[1], CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind, out var until) || now >= until;
    }

    internal void Later(string version, DateTimeOffset now) => Save(noticeFile, version + "\n" + (now + Snooze).ToString("O", CultureInfo.InvariantCulture));
    internal void Dismiss(string version) => Save(noticeFile, version + "\ndismissed");

    static void Save(string path, string text)
    {
        try { Directory.CreateDirectory(Path.GetDirectoryName(path)!); File.WriteAllText(path, text); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
    }

    /// <summary>--check-update：查到該通知的新版就回傳卡片；關閉、已有一張、離線、勿擾或全螢幕時回傳 null，下次排程再查。</summary>
    internal static async Task<UpdateCard?> Check()
    {
        string folder = UpdateService.DataFolder();
        var notice = new UpdateNotice(folder);
        if (!notice.Enabled) return null;
        single = new Mutex(true, @"Local\Shanjie.UpdateNotice.v1", out bool first);
        if (!first) return null;
        using var service = new UpdateService(UpdateService.RunningVersion(), folder);
        using var timeout = new CancellationTokenSource(TimeSpan.FromMinutes(2));
        await service.CheckAsync(timeout.Token);
        if (service.State != UpdateState.Available) return null;
        // 測試版通道同時有正式版與測試版時，通知比較新的那個；「關於」頁仍可改選。
        var release = service.Choices[^1];
        if (!notice.ShouldShow(release.Version, DateTimeOffset.UtcNow) || !AcceptsNotifications()) return null;
        return new UpdateCard(release, notice);
    }

    /// <summary>開啟設定程式時也查一次，走同一條路：卡片在獨立程序，設定視窗不用等網路。</summary>
    internal static void CheckInBackground()
    {
        if (!new UpdateNotice(UpdateService.DataFolder()).Enabled) return;
        try { Process.Start(new ProcessStartInfo(Environment.ProcessPath!, "--check-update") { UseShellExecute = false })?.Dispose(); }
        catch (Exception e) when (e is System.ComponentModel.Win32Exception or InvalidOperationException) { }
    }

    /// <summary>QUNS_ACCEPTS_NOTIFICATIONS；全螢幕、簡報與勿擾回傳其他值。查詢失敗時不打擾。</summary>
    static bool AcceptsNotifications() => SHQueryUserNotificationState(out int state) == 0 && state == 5;
    [DllImport("shell32.dll")] static extern int SHQueryUserNotificationState(out int state);
}
