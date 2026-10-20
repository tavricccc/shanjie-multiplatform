using Microsoft.UI.Xaml;
using Shanjie_UI.Setup;
using Windows.Graphics;
namespace Shanjie_UI.Verification;

/// <summary>Offscreen check of the setup pages. Never starts shanjie-manager or touches the install.</summary>
internal static class SetupVerifier
{
    private static void Check(bool condition, string message) { if (!condition) throw new InvalidDataException(message); }

    public static async Task Run(string output)
    {
        var upgrade = SetupStatus.Parse("""{"installed_version":"0.11.0","keyboards":2,"listed":2,"package_version":"0.12.0","registered32":2,"registered64":2,"target":"C:\\Program Files\\Shanjie"}""");
        Check(upgrade.Installed && upgrade.SuggestedKeyboard == 2 && upgrade.Target == @"C:\Program Files\Shanjie", "Install status parsing failed");
        var fresh = SetupStatus.Parse("""{"installed_version":null,"keyboards":null,"listed":0,"package_version":"0.12.0","registered32":0,"registered64":0,"target":"C:\\Program Files\\Shanjie"}""");
        Check(!fresh.Installed && fresh.SuggestedKeyboard == 1, "Fresh install status parsing failed");
        Check(SetupStatus.Parse("""{"package_version":"0.12.0","listed":1,"registered64":1,"registered32":0}""").KeyboardSummary.Contains("不一致"), "Inconsistent registration was not reported");
        Check(SetupEvent.Parse("""{"stage":"Copy","status":"running","done":25,"total":600}""") is { Done: 25, Total: 600 }, "Progress line parsing failed");
        Check(SetupEvent.Parse("not json") is null && SetupEvent.Parse("""{"status":"started"}""") is null, "Malformed progress lines were accepted");

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
