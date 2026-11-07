using Microsoft.UI.Xaml;
using Shanjie_UI.Models;
using Shanjie_UI.Setup;
namespace Shanjie_UI.Verification;

/// <summary>Quick Setup's decisions, what it writes and what it looks like, on the isolated test profile.</summary>
internal static class QuickSetupVerifier
{
    static void Check(bool condition, string what) { if (!condition) throw new InvalidDataException("Quick Setup: " + what); }
    const string Tuned = "[profile]\nschema=9\ninteraction=windows\n[engine]\nranking=formal\nmodel_id=builtin\nfuzzy_zh_z=1\n[input]\nlearning=0\nkeyboard=0\ndictionary=my-words.tsv\ncontext=0\nmixed_input=0\nupstream_english=1\n[candidates]\nlayout=horizontal\nanchor=left\nright_action=page\nreading_panel=0\nspace_operates=1\nopen=down\n[shortcuts]\nshift_toggle=0\n[appearance]\ntheme=2\nfont_size=22\nglass_tint=70\n";

    /// One process per phase, like reopening the settings app: only the first open is due, and an upgrade does not bring it back.
    internal static async Task RunPhase(MainWindow settings, string phase)
    {
        settings.AppWindow.Move(new Windows.Graphics.PointInt32(-12000, -12000)); settings.AppWindow.Show(false);
        await Task.Delay(300);
        bool marker = File.Exists(QuickSetupGate.MarkerPath);
        bool due = QuickSetupGate.Decide(marker, elevated: false, bypass: false, testProfile: false);
        switch (phase)
        {
            case "first-open":
                Check(due && !File.Exists(ProfileStore.Path), "a person with no marker and no settings should see it");
                settings.OpenQuickSetup(); await Task.Delay(300);
                Check(File.Exists(QuickSetupGate.MarkerPath), "showing did not write the marker");
                settings.QuickSetup!.SkipForVerification();
                Check(!File.Exists(ProfileStore.Path), "skipping a fresh install wrote settings");
                break;
            case "reopen": Check(!due, "it showed again when the settings reopened"); break;
            case "after-upgrade":
                Check(!due, "a later version showed it again");
                Check(File.ReadAllText(QuickSetupGate.MarkerPath).Contains("0.0.0-older"), "the fixture's older marker was rewritten");
                break;
            default: throw new InvalidDataException(phase);
        }
        settings.Close();
    }

    public static async Task Run(MainWindow settings, string output)
    {
        VerifyGate();
        VerifyDraft();
        await VerifyFlow(settings);
        await VerifyDemos(settings);
        await Capture(settings, output);
        File.Delete(QuickSetupGate.MarkerPath);
    }
    static void VerifyGate()
    {
        Check(QuickSetupGate.Decide(false, false, false, false), "the plain first launch should be due");
        Check(!QuickSetupGate.Decide(true, false, false, false), "the marker should stop it");
        Check(!QuickSetupGate.Decide(false, true, false, false), "an elevated process should not show it");
        Check(!QuickSetupGate.Decide(false, false, true, false), "the bypass should stop it");
        Check(!QuickSetupGate.Decide(false, false, false, true), "a test profile should not show it");
        File.Delete(QuickSetupGate.MarkerPath);
        Check(!QuickSetupGate.Due([]), "this verification run itself is due");
        Check(QuickSetupGate.Bypassed(["app", QuickSetupGate.BypassArgument]), "the bypass argument is not read");
        Environment.SetEnvironmentVariable(QuickSetupGate.BypassVariable, "1");
        Check(QuickSetupGate.Bypassed(["app"]), "the bypass variable is not read");
        Environment.SetEnvironmentVariable(QuickSetupGate.BypassVariable, null);
        Check(!QuickSetupGate.Bypassed(["app"]), "bypassed without being asked");
        Check(!File.Exists(QuickSetupGate.MarkerPath), "the checks wrote a marker");
        // Per user: the marker sits with the profile, in the folder the native code resolves for this token.
        Check(Path.GetDirectoryName(QuickSetupGate.MarkerPath) == Path.GetDirectoryName(ProfileStore.Path), "the marker is not beside the profile");
    }
    static void VerifyDraft()
    {
        void Fixture(string text) { Directory.CreateDirectory(Path.GetDirectoryName(ProfileStore.Path)!); File.WriteAllText(ProfileStore.Path, text); }
        string Saved() => File.ReadAllText(ProfileStore.Path);
        void Personal(ProfileDocument d)
        {
            Check(d.Read("engine", "ranking", "") == "formal" && d.Read("engine", "fuzzy_zh_z", "") == "1" && d.Read("input", "dictionary", "") == "my-words.tsv" && d.Read("input", "context", "") == "0"
                && d.Read("appearance", "theme", "") == "2" && d.Read("appearance", "font_size", "") == "22" && d.Read("appearance", "glass_tint", "") == "70", "a personal key changed");
        }
        // Keeping everything and finishing writes nothing at all.
        Fixture(Tuned);
        var keep = new QuickSetupDraft();
        Check(keep.HadSettings && keep.Template == QuickTemplate.Keep, "existing settings should start from keeping them");
        Check(!keep.Changed && !keep.Apply() && Saved() == Tuned, "keeping everything rewrote the file");
        // One detail on top of keeping: only that key differs from the tuned file.
        keep.RightAction = 2;
        Check(keep.Apply(), "a detail was not saved");
        var one = ProfileStore.Load();
        Personal(one);
        // The fixture's old layout line loads harmlessly and is gone once the file is saved.
        Check(!Saved().Contains("layout"), "a stale layout line survived the save");
        Check(one.Read("candidates", "right_action", "") == "expand" && one.Read("candidates", "anchor", "") == "left" && one.Read("shortcuts", "shift_toggle", "") == "0" && one.Read("input", "learning", "") == "0", "one detail changed other values");
        // Going back and undoing it restores the starting point.
        keep.RightAction = 1;
        Check(!keep.Apply() && ProfileStore.Load().Read("candidates", "right_action", "") == "page", "undoing a detail did not restore the file");
        // A template replaces operation keys and keeps personal ones, and a detail wins over it.
        Fixture(Tuned);
        var shanjie = new QuickSetupDraft();
        shanjie.Choose(QuickTemplate.Shanjie);
        Check(shanjie.Space == SpaceChoice.OpenList && shanjie.RightAction == 0 && shanjie.Mix == MixChoice.Automatic && !shanjie.PanelShown && shanjie.ShiftToggle, "the 善解 template values");
        Check(!shanjie.Learning, "a template changed learning");
        shanjie.RightAction = 1; shanjie.Apply();
        var saved = ProfileStore.Load();
        Personal(saved);
        Check(saved.Read("input", "learning", "") == "0" && saved.Read("profile", "interaction", "") == "shanjie" && saved.Read("candidates", "right_action", "") == "page" && saved.Read("candidates", "space_operates", "") == "1", "template plus detail was not saved");
        var windows = new QuickSetupDraft();
        windows.Choose(QuickTemplate.Windows);
        Check(windows.Space == SpaceChoice.Insert && windows.RightAction == 2 && windows.Mix == MixChoice.Automatic && !windows.PanelShown && windows.ShiftToggle, "the 微軟注音風格 template values");
        // Another template drops earlier details, but not learning, which no template changes.
        windows.RightAction = 1; windows.Learning = true; windows.Choose(QuickTemplate.Shanjie);
        Check(windows.RightAction == 0 && windows.Learning, "changing template should reset details except learning");
        // Space choices reach the keys the TSF reads.
        Fixture(Tuned);
        var space = new QuickSetupDraft();
        Check(space.Space == SpaceChoice.Custom, "operate with open=down is a custom combination");
        space.Space = SpaceChoice.OpenList;
        Check(space.Current.Read("candidates", "open", "") == "both" && space.Current.Flag("candidates", "space_operates", false) && space.Current.Flag("candidates", "space_pages", false), "open list");
        space.Space = SpaceChoice.Commit;
        Check(!space.Current.Flag("candidates", "space_operates", true) && space.Current.Flag("input", "space_commits", false), "commit");
        space.Space = SpaceChoice.Insert;
        Check(!space.Current.Flag("candidates", "space_operates", true) && !space.Current.Flag("input", "space_commits", true), "insert");
        // Raw-key mixing forces the panel at read time and leaves the stored choice alone.
        space.Mix = MixChoice.RawKeys;
        Check(space.PanelForced && space.PanelShown && !space.PanelChoice && space.Current.Flag("input", "mixed_input", false) && !space.Current.Flag("input", "upstream_english", true), "raw-key mixing");
        space.Mix = MixChoice.Automatic;
        Check(!space.PanelForced && !space.PanelShown, "leaving raw-key mixing should bring back the stored panel choice");
        // A fresh install starts from the default template, offers no keep, and an untouched finish writes nothing.
        File.Delete(ProfileStore.Path);
        var fresh = new QuickSetupDraft();
        Check(!fresh.HadSettings && fresh.Template == QuickTemplate.Windows && !fresh.Apply() && !File.Exists(ProfileStore.Path), "an untouched fresh install wrote settings");
        fresh.Choose(QuickTemplate.Shanjie);
        Check(fresh.Apply() && ProfileStore.Load().Read("profile", "interaction", "") == "shanjie" && ProfileStore.Load().Flag("input", "learning", false), "a fresh install's template");
    }
    /// The real view over the real window: skipping leaves the file, finishing updates the settings page, and the forced panel greys out.
    static async Task VerifyFlow(MainWindow settings)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(ProfileStore.Path)!);
        File.WriteAllText(ProfileStore.Path, Tuned); settings.Vm.Load();
        File.Delete(QuickSetupGate.MarkerPath);
        settings.OpenQuickSetup(); await Task.Delay(120);
        var view = settings.QuickSetup!;
        Check(File.Exists(QuickSetupGate.MarkerPath), "opening did not write the marker");
        Check(view.KeepOfferedForVerification && view.SkipTextForVerification.Contains("保留"), "an upgrader should be offered keeping");
        Check(view.TemplateOverflowForVerification <= 0.5, $"the upgrader's first step overflows the smallest window by {view.TemplateOverflowForVerification}");
        view.NextForVerification(); await Task.Delay(60);
        Check(view.StepForVerification == 1 && view.RowKeysForVerification.SequenceEqual(["space", "right", "mix", "shift", "learning", "panel"]), "the detail rows");
        view.SkipForVerification();
        Check(settings.QuickSetup is null && File.ReadAllText(ProfileStore.Path) == Tuned, "skipping changed the settings");
        settings.OpenQuickSetup(true); view = settings.QuickSetup!; await Task.Delay(60);
        Check(view.SkipTextForVerification == "取消", "reopening from settings should offer cancel");
        view.ChooseForVerification(QuickTemplate.Windows); view.NextForVerification(); await Task.Delay(60);
        Check(view.RowTitleForVerification("right").Contains("Tab"), "the Windows interaction should name Tab");
        view.SelectForVerification("mix", 1); await Task.Delay(60);
        Check(!view.RowEnabledForVerification("panel") && view.RowShownForVerification("panel") == "開啟" && view.RowShownForVerification("mix") == ViewModels.SettingsViewModel.RawKeyMixingName, "raw-key mixing should force and grey the panel");
        view.SelectForVerification("mix", 0); await Task.Delay(60);
        Check(view.RowEnabledForVerification("panel") && view.RowShownForVerification("panel") == "關閉", "leaving raw-key mixing should free the panel");
        view.SelectForVerification("shift", 1); view.SelectForVerification("learning", 0); view.SelectForVerification("panel", 1); await Task.Delay(60);
        view.NextForVerification(); await Task.Delay(60);
        Check(view.StepForVerification == 2, "applying should move on");
        view.BackForVerification(); await Task.Delay(60);
        Check(view.StepForVerification == 1 && !view.SkipOfferedForVerification, "skip should not be offered once the choices are saved");
        view.NextForVerification(); await Task.Delay(60);
        var saved = ProfileStore.Load();
        Check(saved.Read("profile", "interaction", "") == "windows" && saved.Read("shortcuts", "shift_toggle", "") == "0" && saved.Read("input", "learning", "") == "1" && saved.Read("candidates", "reading_panel", "") == "1" && saved.Read("appearance", "font_size", "") == "22", "the flow's choices were not saved");
        view.NextForVerification();
        Check(settings.QuickSetup is null && !settings.Vm.HasChanges && settings.Vm.ReadingPanel && !settings.Vm.ShiftToggle && settings.Vm.Learning, "the settings page did not pick up the result");
    }
    static async Task VerifyDemos(MainWindow settings)
    {
        foreach (var (name, script) in new (string, DemoScript)[] {
            ("template", QuickSetupDemos.Template(false)), ("template", QuickSetupDemos.Template(true)),
            ("space", QuickSetupDemos.Space(SpaceChoice.Insert, false)), ("space", QuickSetupDemos.Space(SpaceChoice.OpenList, true)), ("space", QuickSetupDemos.Space(SpaceChoice.Commit, true)), ("space", QuickSetupDemos.Space(SpaceChoice.Custom, true)),
            ("right", QuickSetupDemos.Right(0, false)), ("right", QuickSetupDemos.Right(1, true)), ("right", QuickSetupDemos.Right(2, true)),
            ("mix", QuickSetupDemos.MixAutomatic()), ("mix", QuickSetupDemos.MixRawKeys()), ("mix", QuickSetupDemos.MixUnchanged()),
            ("shift", QuickSetupDemos.Shift(true)), ("shift", QuickSetupDemos.Shift(false)),
            ("learning", QuickSetupDemos.Learning(true, true)), ("learning", QuickSetupDemos.Learning(false, false)),
            ("panel", QuickSetupDemos.Panel(true, true)), ("panel", QuickSetupDemos.Panel(true, false)), ("panel", QuickSetupDemos.Panel(false, true)) })
        {
            Check(script.Keyframe >= 0 && script.Keyframe < script.Frames.Count && script.Frames.All(f => f.Ms > 0), name + " demo frames");
            foreach (var frame in script.Frames)
            {
                Check(frame.Caret <= frame.Composing.Length, name + " caret");
                if (frame.Panel is { } panel) Check(frame.MarkStart + frame.MarkLength <= panel.Length, name + " mark");
                if (frame.List is { } list) Check(list.Selected < list.Items.Length, name + " selection");
            }
        }
        // The panel marks the cursor's character inside the sentence, on its anchor's side, and nothing at the end.
        var right = QuickSetupDemos.Panel(true, true).Frames; var left = QuickSetupDemos.Panel(true, false).Frames;
        Check(right[0].MarkStart == -1 && right[1].MarkStart == right[1].Caret && left[1].MarkStart == left[1].Caret - 1, "the panel's mark");
        Check(QuickSetupDemos.Panel(false, true).Frames.All(f => f.Panel is null), "the panel showed while off");
        Check(QuickSetupDemos.Right(2, true).Frames.Any(f => f is { Key: "Tab", List.Layout: DemoLayout.Grid }) && QuickSetupDemos.Right(2, false).Frames.All(f => f.Key != "Tab"), "Tab belongs to the Windows interaction");
        Check(QuickSetupDemos.Space(SpaceChoice.Commit, true).Frames[^1].Done == "你好 " && QuickSetupDemos.Space(SpaceChoice.Commit, false).Frames[^1].Done == "你好", "the space after a commit");
        // It loops while animations are on and holds its key frame, with no timer, while they are off.
        settings.OpenQuickSetup(true); var view = settings.QuickSetup!;
        var demo = view.TemplateDemoForVerification(false);
        await Task.Delay(120);
        Check(TypingDemo_Running(demo) == Controls.TypingDemo.Animated, "the timer should run only with animations on");
        Controls.TypingDemo.AnimationsForVerification = true; demo.RestartForVerification();
        int first = demo.FrameForVerification; await Task.Delay(900);
        Check(demo.RunningForVerification && demo.FrameForVerification != first, "the demo did not advance");
        Controls.TypingDemo.AnimationsForVerification = false; demo.RestartForVerification(); await Task.Delay(60);
        var script2 = QuickSetupDemos.Template(false);
        Check(!demo.RunningForVerification && demo.FrameForVerification == script2.Keyframe && demo.CaptionForVerification == script2.Frames[script2.Keyframe].Caption, "reduced motion should hold the key frame");
        Controls.TypingDemo.AnimationsForVerification = null;
        // Every frame of every demo, each option of each row, fits its scene.
        Controls.TypingDemo.AnimationsForVerification = false;
        foreach (bool windowsDemo in new[] { false, true })
        {
            var card = view.TemplateDemoForVerification(windowsDemo); var script3 = QuickSetupDemos.Template(windowsDemo);
            for (int i = 0; i < script3.Frames.Count; i++) { card.HoldForVerification(i); card.UpdateLayout(); Check(card.FitsForVerification, $"template demo frame {i} overflows"); }
        }
        view.ChooseForVerification(QuickTemplate.Windows); view.NextForVerification(); await Task.Delay(120);
        foreach (string key in view.RowKeysForVerification.ToList())
        {
            view.ActivateForVerification(key);
            for (int option = 0; option < 4; option++)
            {
                if (!view.RowEnabledForVerification(key) || option >= view.RowOptionCountForVerification(key)) break;
                view.SelectForVerification(key, option); await Task.Delay(20);
                var scene = view.DetailDemoForVerification; scene.UpdateLayout();
                for (int i = 0; i < scene.FrameCountForVerification; i++) { scene.HoldForVerification(i); scene.UpdateLayout(); Check(scene.FitsForVerification, $"{key} option {option} frame {i} overflows"); }
            }
        }
        Controls.TypingDemo.AnimationsForVerification = null;
        view.SkipForVerification();
    }
    static bool TypingDemo_Running(Controls.TypingDemo demo) => demo.RunningForVerification;

    /// Each step in both themes, for a fresh install and an upgrader, with the demos held on telling frames.
    static async Task Capture(MainWindow settings, string output)
    {
        string folder = Path.Combine(output, "quick-setup"); Directory.CreateDirectory(folder);
        foreach (int theme in new[] { 1, 2 })
        {
            string name = theme == 1 ? "light" : "dark";
            File.Delete(ProfileStore.Path); settings.Vm.Load(); settings.Vm.Theme = theme;
            Controls.TypingDemo.AnimationsForVerification = false;
            settings.OpenQuickSetup(); var view = settings.QuickSetup!; await Task.Delay(300);
            await VisualVerifier.Capture(settings.CaptureElement, Path.Combine(folder, $"1-template-new-{name}.png"));
            view.NextForVerification(); await Task.Delay(200);
            var details = new (string Key, int Choice, string File, int Frame)[] {
                ("space", 1, "space-open", -1), ("right", 2, "right-expand", -1), ("mix", 1, "mix-raw", -1), ("shift", 0, "shift", -1), ("learning", 0, "learning", -1), ("panel", 1, "panel-on", -1) };
            foreach (var (key, choice, file, frame) in details)
            {
                if (theme == 2 && file is not ("right-expand" or "mix-raw" or "panel-on")) continue;
                view.ActivateForVerification(key); view.SelectForVerification(key, choice); await Task.Delay(120);
                await VisualVerifier.Capture(settings.CaptureElement, Path.Combine(folder, $"2-details-{file}-{name}.png"));
                if (key == "mix") { view.SelectForVerification("mix", 0); await Task.Delay(60); }
            }
            view.NextForVerification(); await Task.Delay(200);
            await VisualVerifier.Capture(settings.CaptureElement, Path.Combine(folder, $"3-try-{name}.png"));
            view.NextForVerification(); await Task.Delay(120);
            // An upgrader's first step offers keeping.
            File.WriteAllText(ProfileStore.Path, Tuned.Replace("theme=2", "theme=" + theme)); settings.Vm.Load();
            settings.OpenQuickSetup(); await Task.Delay(300);
            await VisualVerifier.Capture(settings.CaptureElement, Path.Combine(folder, $"1-template-upgrade-{name}.png"));
            settings.QuickSetup!.SkipForVerification();
            Controls.TypingDemo.AnimationsForVerification = null;
        }
        settings.Vm.Theme = 0;
    }
}
