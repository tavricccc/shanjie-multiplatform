using System.Diagnostics;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Shanjie_UI.Setup;
namespace Shanjie_UI;

/// <summary>螢幕右下角的新版通知，不搶焦點。一分鐘沒有回應視同「稍後」。</summary>
public sealed partial class UpdateCard : UserControl
{
    static readonly TimeSpan Lifetime = TimeSpan.FromMinutes(1);
    readonly UpdateRelease release;
    readonly UpdateNotice notice;
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer timer;

    internal UpdateCard(UpdateRelease release, UpdateNotice notice)
    {
        this.release = release; this.notice = notice;
        InitializeComponent();
        VersionText.Text = (release.Prerelease ? "測試版 " : "正式版 ") + release.Version;
        DetailText.Text = $"{release.SizeBytes / 1024.0 / 1024:0.#} MB · {release.PublishedAt.LocalDateTime:M 月 d 日}發布";
        Root.RequestedTheme = SavedTheme();
        Host = new Glass.GlassWindow(this, "善解有新版本", 340, 148, resizable: false, notification: true);
        timer = DispatcherQueue.CreateTimer();
        timer.Interval = Lifetime; timer.IsRepeating = false;
        timer.Tick += (_, _) => Later(this, new RoutedEventArgs());
        Loaded += (_, _) => timer.Start();
    }

    internal Glass.GlassWindow Host { get; }
    internal FrameworkElement CaptureElement => Root;
    internal void SetThemeForVerification(ElementTheme theme) => Root.RequestedTheme = theme;

    /// <summary>設定程式的主題（0 跟隨系統、1 淺色、2 深色）；卡片不載入整份設定。</summary>
    static ElementTheme SavedTheme()
    {
        try
        {
            foreach (string line in File.ReadLines(Models.ProfileStore.Path))
                if (line.StartsWith("theme=") && int.TryParse(line[6..], out int value) && value is 1 or 2) return (ElementTheme)value;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
        return ElementTheme.Default;
    }

    void Later(object sender, RoutedEventArgs args) { timer.Stop(); notice.Later(release.Version, DateTimeOffset.UtcNow); Host.Close(); }
    void Dismiss(object sender, RoutedEventArgs args) { timer.Stop(); notice.Dismiss(release.Version); Host.Close(); }
    void Open(object sender, RoutedEventArgs args)
    {
        timer.Stop();
        try
        {
            Process.Start(new ProcessStartInfo(Environment.ProcessPath!, "--about") { UseShellExecute = false })?.Dispose();
            notice.Dismiss(release.Version);
        }
        catch (Exception e) when (e is System.ComponentModel.Win32Exception or InvalidOperationException) { notice.Later(release.Version, DateTimeOffset.UtcNow); }
        Host.Close();
    }
}
