using Microsoft.UI.Xaml;
using Shanjie_UI.Setup;
using Windows.Graphics;
namespace Shanjie_UI.Verification;

/// <summary>Offscreen check of the setup pages. Never starts shanjie-manager or touches the install.</summary>
internal static class SetupVerifier
{
    private static void Check(bool condition, string message) { if (!condition) throw new InvalidDataException(message); }

    /// <summary>Update channel filtering against a canned release list served on 127.0.0.1; nothing leaves the machine.</summary>
    private static async Task CheckUpdateChannels()
    {
        int port;
        using (var probe = new System.Net.Sockets.TcpListener(System.Net.IPAddress.Loopback, 0)) { probe.Start(); port = ((System.Net.IPEndPoint)probe.LocalEndpoint).Port; }
        using var listener = new System.Net.HttpListener();
        listener.Prefixes.Add($"http://127.0.0.1:{port}/");
        listener.Start();
        string body = "[]";
        var server = Task.Run(async () =>
        {
            while (listener.IsListening)
            {
                try
                {
                    var context = await listener.GetContextAsync();
                    byte[] bytes = System.Text.Encoding.UTF8.GetBytes(body);
                    context.Response.ContentType = "application/json";
                    await context.Response.OutputStream.WriteAsync(bytes);
                    context.Response.Close();
                }
                catch (Exception) { break; }
            }
        });
        string Release(string tag, bool pre) => $$"""
            {"draft":false,"tag_name":"v{{tag}}","prerelease":{{(pre ? "true" : "false")}},"body":"","published_at":"2026-01-01T00:00:00Z","html_url":"http://127.0.0.1:{{port}}/r/{{tag}}",
             "assets":[{"name":"Shanjie-Windows-Setup.exe","size":1,"browser_download_url":"http://127.0.0.1:{{port}}/a/{{tag}}/exe"},
                       {"name":"Shanjie-Windows-Setup.exe.sha256","size":1,"browser_download_url":"http://127.0.0.1:{{port}}/a/{{tag}}/sha"}]}
            """;
        string[] data = [Path.Combine(Path.GetTempPath(), "shanjie-channel-" + Guid.NewGuid().ToString("N")), Path.Combine(Path.GetTempPath(), "shanjie-channel-" + Guid.NewGuid().ToString("N"))];
        var uri = new Uri($"http://127.0.0.1:{port}/");
        try
        {
            // Stable channel (default for a stable build) hides a newer beta; switching shows it and is remembered.
            body = $"[{Release("0.15.3", false)},{Release("0.15.4-beta.1", true)}]";
            using (var service = new UpdateService("0.15.3", data[0], uri))
            {
                await service.CheckAsync(CancellationToken.None);
                Check(service.Channel == UpdateChannel.Stable && service.State == UpdateState.UpToDate && service.Choices.Count == 0, "Stable channel offered a beta");
                Check(service.SetChannel(UpdateChannel.Prerelease), "Channel could not be switched");
                await service.CheckAsync(CancellationToken.None);
                Check(service.Latest?.Version == "0.15.4-beta.1" && service.Latest.Prerelease, "Beta channel did not offer the beta");
            }
            using (var again = new UpdateService("0.15.3", data[0], uri)) Check(again.Channel == UpdateChannel.Prerelease, "Update channel was not remembered");

            // A beta build defaults to the beta channel, sees the next beta, and prefers its own stable release over later betas.
            body = $"[{Release("0.15.3", false)},{Release("0.15.4-beta.1", true)},{Release("0.15.4-beta.2", true)}]";
            using (var beta = new UpdateService("0.15.4-beta.1", data[1], uri))
            {
                Check(beta.Channel == UpdateChannel.Prerelease, "A beta build did not default to the beta channel");
                await beta.CheckAsync(CancellationToken.None);
                Check(beta.Latest?.Version == "0.15.4-beta.2", "Beta build was not offered the next beta");
                body = $"[{Release("0.15.3", false)},{Release("0.15.4-beta.2", true)},{Release("0.15.4", false)}]";
                await beta.CheckAsync(CancellationToken.None);
                Check(beta.Choices.Count == 1 && beta.Latest?.Version == "0.15.4" && !beta.Latest.Prerelease, "Stable release did not replace the older beta");
            }
        }
        finally
        {
            listener.Stop();
            await server;
            foreach (var folder in data) try { Directory.Delete(folder, true); } catch (IOException) { }
        }
    }

    public static async Task Run(string output)
    {
        var upgrade = SetupStatus.Parse("""{"installed_version":"0.11.0","keyboards":2,"listed":2,"package_version":"0.12.0","registered32":2,"registered64":2,"target":"C:\\Program Files\\Shanjie"}""");
        Check(upgrade.Installed && upgrade.SuggestedKeyboard == 2 && upgrade.Target == @"C:\Program Files\Shanjie", "Install status parsing failed");
        var fresh = SetupStatus.Parse("""{"installed_version":null,"keyboards":null,"listed":0,"package_version":"0.12.0","registered32":0,"registered64":0,"target":"C:\\Program Files\\Shanjie"}""");
        Check(!fresh.Installed && fresh.SuggestedKeyboard == 1, "Fresh install status parsing failed");
        Check(SetupStatus.Parse("""{"package_version":"0.12.0","listed":1,"registered64":1,"registered32":0}""").KeyboardSummary.Contains("不一致"), "Inconsistent registration was not reported");
        Check(SetupEvent.Parse("""{"stage":"Copy","status":"running","done":25,"total":600}""") is { Done: 25, Total: 600 }, "Progress line parsing failed");
        Check(SetupEvent.Parse("not json") is null && SetupEvent.Parse("""{"status":"started"}""") is null, "Malformed progress lines were accepted");

        // Update ordering: a beta is newer than the stable release before it and older than its own stable release.
        string[] ascending = ["0.15.3", "0.15.4-beta.1", "0.15.4-beta.2", "0.15.4-beta.10", "0.15.4", "0.15.10", "0.16.0"];
        for (int i = 0; i < ascending.Length; i++)
            for (int j = 0; j < ascending.Length; j++)
                Check(Math.Sign(UpdateService.Compare(ascending[i], ascending[j])) == Math.Sign(i.CompareTo(j)), $"Version order wrong: {ascending[i]} vs {ascending[j]}");
        var running = UpdateService.RunningVersion();
        Check(System.Text.RegularExpressions.Regex.IsMatch(running, @"^\d+\.\d+\.\d+(-beta\.\d+)?$"), $"Running version has an unexpected format: {running}");
        await CheckUpdateChannels();

        // The update button must keep the themed style in every state; a null style falls back to the native button.
        var themed = Application.Current.Resources;
        foreach (var state in Enum.GetValues<UpdateState>())
        {
            var expected = (Style)themed[state is UpdateState.Available or UpdateState.Ready ? "SjAccentButton" : "SjTextButton"];
            Check(ReferenceEquals(MainWindow.ButtonStyle(state), expected), $"Update button is not themed in state {state}");
        }

        foreach (var theme in new[] { ElementTheme.Light, ElementTheme.Dark })
        {
            string suffix = theme == ElementTheme.Light ? "light" : "dark";
            var install = Open(false, theme);
            await Task.Delay(400);
            Check(!install.PrimaryEnabledForVerification, "Next was enabled before the install status loaded");
            install.ShowStatus(upgrade);
            Check(install.PrimaryEnabledForVerification && install.PrimaryForVerification == "下一步", "Welcome page actions failed");
            await VisualVerifier.Capture(install.CaptureElement, Path.Combine(output, $"setup-welcome-{suffix}.png"));
            install.Show(SetupWindow.Page.Keyboard);
            Check(install.KeyboardForVerification == 2 && install.PrimaryForVerification == "安裝", "Keyboard page did not keep the installed choice");
            await VisualVerifier.Capture(install.CaptureElement, Path.Combine(output, $"setup-keyboard-{suffix}.png"));

            install.Show(SetupWindow.Page.Progress);
            foreach (var line in new[] {
                """{"stage":"Verify","status":"started","message":""}""", """{"stage":"Verify","status":"succeeded","message":""}""",
                """{"stage":"Copy","status":"started","message":""}""", """{"stage":"Copy","status":"running","done":300,"total":600}""" })
                install.Apply(SetupEvent.Parse(line)!);
            Check(install.PrimaryForVerification == "", "Actions were available during the install");
            var steps = install.StepsForVerification;
            Check(steps[0].State == StepState.Succeeded && steps[1].State == StepState.Running && steps[2].State == StepState.Pending, "Step states did not follow progress");
            await VisualVerifier.Capture(install.CaptureElement, Path.Combine(output, $"setup-progress-{suffix}.png"));
            install.Apply(SetupEvent.Parse("""{"stage":"Copy","status":"succeeded","message":""}""")!);
            install.Apply(SetupEvent.Parse("""{"stage":"Validate","status":"failed","message":"x64 安裝註冊校驗失敗。"}""")!);
            install.Apply(SetupEvent.Parse("""{"stage":"Rollback","status":"started","message":""}""")!);
            install.Finish(new(SetupOutcome.Failed, "安裝未完成，已回復安裝前狀態：x64 安裝註冊校驗失敗。\n紀錄：C:\\Program Files\\Shanjie\\.installer-recovery\\1"));
            Check(steps.Any(s => s.Stage == "Rollback" && s.State == StepState.Failed) && install.PrimaryForVerification == "關閉", "Failed install page failed");
            await VisualVerifier.Capture(install.CaptureElement, Path.Combine(output, $"setup-failed-{suffix}.png"));

            var done = Open(false, theme);
            done.ShowStatus(fresh);
            done.Finish(new(SetupOutcome.Succeeded, "已更新善解，並設定登入後啟動常駐核心。"));
            Check(done.PrimaryForVerification == "開啟設定" && done.StepsForVerification.All(s => s.State == StepState.Pending), "Completed install page failed");
            await VisualVerifier.Capture(done.CaptureElement, Path.Combine(output, $"setup-done-{suffix}.png"));

            var uninstall = Open(true, theme);
            uninstall.ShowStatus(upgrade);
            Check(uninstall.PrimaryForVerification == "解除安裝", "Uninstall page actions failed");
            await VisualVerifier.Capture(uninstall.CaptureElement, Path.Combine(output, $"uninstall-{suffix}.png"));
            uninstall.Finish(new(SetupOutcome.Cancelled, "已取消系統管理員權限要求，沒有修改任何內容。"));
            Check(uninstall.PrimaryForVerification == "關閉", "Cancelled uninstall page failed");
            await VisualVerifier.Capture(uninstall.CaptureElement, Path.Combine(output, $"uninstall-cancelled-{suffix}.png"));
        }
    }

    // Closing the last window ends the app, so every test window stays open until the run ends.
    private static readonly List<SetupWindow> windows = [];
    private static SetupWindow Open(bool uninstall, ElementTheme theme)
    {
        // Keep verification away from the user's windows and never activate the test surfaces.
        var window = new SetupWindow(uninstall);
        window.AppWindow.Move(new PointInt32(-12000, -12000)); window.AppWindow.Show(false);
        window.SetThemeForVerification(theme);
        windows.Add(window);
        return window;
    }
}
