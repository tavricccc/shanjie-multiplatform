using Shanjie_UI.Controls;
using Shanjie_UI.Models;
using Shanjie_UI.Setup;
using Windows.System;
namespace Shanjie_UI.Verification;

/// <summary>Quick Setup's decisions, what it writes and what it looks like, on the isolated test profile.</summary>
internal static class QuickSetupVerifier
{
    static void Check(bool condition, string what) { if (!condition) throw new InvalidDataException("Quick Setup: " + what); }
    const string Tuned = "[profile]\nschema=9\ninteraction=windows\n[engine]\nranking=formal\nmodel_id=builtin\nacg_pack=0\nchat_apps=line.exe\nfuzzy_zh_z=1\n[input]\nlearning=0\nkeyboard=0\ndictionary=my-words.tsv\ncontext=0\nmixed_input=0\nupstream_english=1\n[candidates]\nlayout=horizontal\nanchor=left\nright_action=page\nreading_panel=0\nspace_operates=1\nopen=down\n[shortcuts]\nshift_toggle=0\n[appearance]\ntheme=2\nfont_size=22\nglass_tint=70\n";

    /// One process per phase, like reopening the settings app: only the first open is due, and an upgrade does not
    /// bring it back. The flow phase runs every check below and writes the screenshots to <paramref name="output"/>.
    internal static async Task RunPhase(MainWindow settings, string phase, string output)
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
                settings.QuickSetup!.SkipForVerification(); await Closed(settings);
                Check(!File.Exists(ProfileStore.Path), "skipping a fresh install wrote settings");
                break;
            case "reopen": Check(!due, "it showed again when the settings reopened"); break;
            case "after-upgrade":
                Check(!due, "a later version showed it again");
                Check(File.ReadAllText(QuickSetupGate.MarkerPath).Contains("0.0.0-older"), "the fixture's older marker was rewritten");
                break;
            case "flow": await Run(settings, output); break;
            default: throw new InvalidDataException(phase);
        }
        settings.Close();
    }
    /// Leaving plays a short fade before the settings come back.
    static async Task Closed(MainWindow settings)
    {
        for (int i = 0; i < 60 && settings.QuickSetup is not null; i++) await Task.Delay(50);
        Check(settings.QuickSetup is null, "the flow did not hand back to the settings");
    }

    static async Task Run(MainWindow settings, string output)
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
    static string Sorted(string text) => string.Join("\n", text.Split('\n').Select(line => line.Trim()).Where(line => line.Length > 0).Order(StringComparer.Ordinal));
    static void VerifyDraft()
    {
        string folder = Path.GetDirectoryName(ProfileStore.Path)!;
        void Fixture(string text) { Directory.CreateDirectory(folder); File.WriteAllText(ProfileStore.Path, text); File.Delete(QuickSetupDraft.BackupPath); }
        string Saved() => File.ReadAllText(ProfileStore.Path);
        // Personal data lives in its own files; finishing must not touch them.
        string words = Path.Combine(folder, "my-words.tsv"), memory = Path.Combine(folder, "learning-fixture.bin");
        File.WriteAllText(words, "ㄋㄧˇ 你\n"); File.WriteAllBytes(memory, [1, 2, 3, 4]);

        // Keeping everything and finishing writes nothing at all, not even a backup.
        Fixture(Tuned);
        var keep = new QuickSetupDraft();
        Check(keep.HadSettings && keep.Template == QuickTemplate.Keep, "existing settings should start from keeping them");
        Check(!keep.Changed && !keep.Apply() && Saved() == Tuned && !File.Exists(QuickSetupDraft.BackupPath), "keeping everything rewrote the file");
        // One detail on top of keeping: only that key differs from the tuned file, and the old file is backed up byte for byte.
        keep.RightAction = 2;
        Check(keep.Apply(), "a detail was not saved");
        Check(File.ReadAllText(QuickSetupDraft.BackupPath) == Tuned, "the backup is not the file as it was");
        var one = ProfileStore.Load();
        Check(one.Read("engine", "ranking", "") == "formal" && one.Read("appearance", "font_size", "") == "22" && one.Read("engine", "fuzzy_zh_z", "") == "1", "keeping lost a value");
        Check(!Saved().Contains("layout"), "a stale layout line survived the save");
        Check(one.Read("candidates", "right_action", "") == "expand" && one.Read("candidates", "anchor", "") == "left" && one.Read("shortcuts", "shift_toggle", "") == "0" && one.Read("input", "learning", "") == "0", "one detail changed other values");
        // Going back and undoing it restores the starting point; the backup still holds the original, not the first save.
        keep.RightAction = 1;
        Check(!keep.Apply() && ProfileStore.Load().Read("candidates", "right_action", "") == "page", "undoing a detail did not restore the file");
        Check(File.ReadAllText(QuickSetupDraft.BackupPath) == Tuned, "a second save in the same run replaced the backup");

        // A template is a fresh file: the template as it ships, the carried keys, the choices, and nothing else.
        foreach (bool windowsTemplate in new[] { false, true })
        {
            Fixture(Tuned);
            var draft = new QuickSetupDraft();
            draft.Choose(windowsTemplate ? QuickTemplate.Windows : QuickTemplate.Shanjie);
            draft.RightAction = 1; draft.ShiftToggle = false;
            Check(draft.Apply(), "a template was not saved");
            var expected = ProfileStore.Defaults(windowsTemplate);
            expected.PreserveFrom(ProfileDocument.Parse(Tuned), QuickSetupDraft.Carried);
            expected.Set("candidates", "right_action", "page"); expected.Set("shortcuts", "shift_toggle", false);
            Check(Sorted(ProfileStore.Load().ToString()) == Sorted(expected.ToString()), $"the {(windowsTemplate ? "Windows" : "善解")} template file is not template + carried keys + choices");
            // Byte for byte what saving that document writes.
            string written = Saved();
            ProfileStore.Save(expected.ToString());
            Check(Saved() == written, "the saved bytes differ from the expected document");
            var saved = ProfileStore.Load();
            Check(saved.Read("engine", "acg_pack", "") == "0" && saved.Read("engine", "chat_apps", "") == "line.exe" && saved.Read("input", "dictionary", "") == "my-words.tsv", "a carried key was lost");
            Check(saved.Read("engine", "ranking", "") == "auto" && saved.Read("engine", "fuzzy_zh_z", "") == "0" && saved.Read("appearance", "theme", "") == "0" && saved.Read("appearance", "font_size", "") == "16"
                && saved.Read("input", "context", "") == "1" && saved.Read("input", "learning", "") == "1", "an old value outside the carried keys came along");
            Check(File.ReadAllText(QuickSetupDraft.BackupPath) == Tuned, "the template save did not back up the old file");
        }
        Check(File.ReadAllText(words) == "ㄋㄧˇ 你\n" && File.ReadAllBytes(memory).SequenceEqual(new byte[] { 1, 2, 3, 4 }), "a personal data file was touched");
        File.Delete(words); File.Delete(memory);

        // Template values as the questions read them.
        Fixture(Tuned);
        var shanjie = new QuickSetupDraft(); shanjie.Choose(QuickTemplate.Shanjie);
        Check(shanjie.Space == SpaceChoice.OpenList && shanjie.RightAction == 0 && shanjie.Mix == MixChoice.Automatic && !shanjie.PanelShown && shanjie.ShiftToggle && shanjie.Learning, "the 善解 template values");
        var windows = new QuickSetupDraft(); windows.Choose(QuickTemplate.Windows);
        Check(windows.Space == SpaceChoice.Insert && windows.RightAction == 2 && windows.Mix == MixChoice.Automatic && !windows.PanelShown && windows.ShiftToggle, "the 微軟注音風格 template values");
        // Another template drops earlier details, but not learning, which is asked the same way under each.
        windows.RightAction = 1; windows.Learning = false; windows.Choose(QuickTemplate.Shanjie);
        Check(windows.RightAction == 0 && !windows.Learning, "changing template should reset details except learning");
        // Space choices reach the keys the TSF reads.
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
        // A fresh install starts from the default template and offers no keep; finishing writes that template.
        File.Delete(ProfileStore.Path); File.Delete(QuickSetupDraft.BackupPath);
        var fresh = new QuickSetupDraft();
        Check(!fresh.HadSettings && fresh.Template == QuickTemplate.Windows, "a fresh install's starting point");
        fresh.Choose(QuickTemplate.Shanjie);
        Check(fresh.Apply() && ProfileStore.Load().Read("profile", "interaction", "") == "shanjie" && ProfileStore.Load().Flag("input", "learning", false) && !File.Exists(QuickSetupDraft.BackupPath), "a fresh install's template");
        File.Delete(ProfileStore.Path);
    }
    /// The real view over the real window: skipping leaves the file, finishing updates the settings page, and the forced panel greys out.
    static async Task VerifyFlow(MainWindow settings)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(ProfileStore.Path)!);
        File.WriteAllText(ProfileStore.Path, Tuned); settings.Vm.Load();
        File.Delete(QuickSetupGate.MarkerPath); File.Delete(QuickSetupDraft.BackupPath);
        settings.OpenQuickSetup(); await Task.Delay(200);
        var view = settings.QuickSetup!;
        Check(File.Exists(QuickSetupGate.MarkerPath), "opening did not write the marker");
        Check(view.StepForVerification == 0 && view.NextTextForVerification == "開始" && view.TitleForVerification == "善解多了快速設定", "it should open on the welcome");
        Check(view.SkipTextForVerification.Contains("保留"), "an upgrader should be offered keeping");
        view.KeyForVerification(VirtualKey.Enter); await Task.Delay(60);
        Check(view.StepForVerification == 1 && view.KeepOfferedForVerification, "Enter should go on to the template with keeping offered");
        Check(view.OverflowForVerification <= 0.5, $"the upgrader's template step overflows the smallest window by {view.OverflowForVerification}");
        view.KeyForVerification(VirtualKey.Escape); await Task.Delay(60);
        Check(view.StepForVerification == 0, "Esc should go back");
        view.NextForVerification(); view.NextForVerification(); await Task.Delay(60);
        Check(view.StepForVerification == 2 && view.RowKeysForVerification.SequenceEqual(["space", "right", "mix", "shift", "learning", "panel"]), "one screen per question");
        view.SkipForVerification(); await Closed(settings);
        Check(File.ReadAllText(ProfileStore.Path) == Tuned && !File.Exists(QuickSetupDraft.BackupPath), "skipping changed the settings");

        settings.OpenQuickSetup(true); view = settings.QuickSetup!; await Task.Delay(60);
        Check(view.SkipTextForVerification == "取消" && view.TitleForVerification == "快速設定", "reopening from settings should offer cancel");
        view.NextForVerification(); view.ChooseForVerification(QuickTemplate.Windows); view.NextForVerification(); await Task.Delay(60);
        Check(view.RowTitleForVerification("right").Contains("Tab"), "the Windows interaction should name Tab");
        foreach (string key in view.RowKeysForVerification.ToList())
        {
            view.ActivateForVerification(key); await Task.Delay(40);
            Check(view.OverflowForVerification <= 0.5, $"the {key} step overflows the smallest window by {view.OverflowForVerification}");
        }
        view.ActivateForVerification("mix");
        view.SelectForVerification("mix", 1); await Task.Delay(60);
        Check(!view.RowEnabledForVerification("panel") && view.RowShownForVerification("panel") == "開啟" && view.RowShownForVerification("mix") == ViewModels.SettingsViewModel.RawKeyMixingName, "raw-key mixing should force and grey the panel");
        view.SelectForVerification("mix", 0); await Task.Delay(60);
        Check(view.RowEnabledForVerification("panel") && view.RowShownForVerification("panel") == "關閉", "leaving raw-key mixing should free the panel");
        view.SelectForVerification("shift", 1); view.SelectForVerification("learning", 0); view.SelectForVerification("panel", 1); await Task.Delay(60);
        view.ActivateForVerification("panel");
        Check(view.NextTextForVerification == "套用", "the last question should apply");
        view.NextForVerification(); await Task.Delay(60);
        Check(view.StepForVerification == view.FinishStepForVerification && view.NextTextForVerification == "開始使用", "applying should move on to the finish");
        Check(File.ReadAllText(QuickSetupDraft.BackupPath) == Tuned, "applying a template did not back up the old file");
        view.BackForVerification(); await Task.Delay(60);
        Check(view.StepForVerification == view.FinishStepForVerification - 1 && !view.SkipOfferedForVerification, "skip should not be offered once the choices are saved");
        view.NextForVerification(); await Task.Delay(60);
        var saved = ProfileStore.Load();
        Check(saved.Read("profile", "interaction", "") == "windows" && saved.Read("shortcuts", "shift_toggle", "") == "0" && saved.Read("input", "learning", "") == "1" && saved.Read("candidates", "reading_panel", "") == "1"
            && saved.Read("appearance", "font_size", "") == "16" && saved.Read("input", "dictionary", "") == "my-words.tsv", "the flow's choices were not saved as a fresh template");
        view.NextForVerification(); await Closed(settings);
        Check(!settings.Vm.HasChanges && settings.Vm.ReadingPanel && !settings.Vm.ShiftToggle && settings.Vm.Learning, "the settings page did not pick up the result");

        // A profile saved with both mixing modes (兩者並用, no longer offered) loads as 原鍵混打 among the two single modes; untouched, the file stays.
        string both = Tuned.Replace("mixed_input=0", "mixed_input=1");
        File.WriteAllText(ProfileStore.Path, both); settings.Vm.Load(); File.Delete(QuickSetupDraft.BackupPath);
        settings.OpenQuickSetup(true); view = settings.QuickSetup!; await Task.Delay(60);
        view.NextForVerification(); view.NextForVerification(); await Task.Delay(60);
        Check(view.RowShownForVerification("mix") == ViewModels.SettingsViewModel.RawKeyMixingName && view.RowOptionsForVerification("mix").SequenceEqual(["自動判斷", ViewModels.SettingsViewModel.RawKeyMixingName]), "both modes should load as 原鍵混打 among the two single modes");
        view.ActivateForVerification("panel"); view.NextForVerification(); await Task.Delay(60);
        Check(File.ReadAllText(ProfileStore.Path) == both && !File.Exists(QuickSetupDraft.BackupPath), "keeping both modes untouched wrote the file");
        view.NextForVerification(); await Closed(settings);
    }
    static async Task VerifyDemos(MainWindow settings)
    {
        foreach (var (name, script) in new (string, DemoScript)[] {
            ("template", QuickSetupDemos.Template(false)), ("template", QuickSetupDemos.Template(true)), ("keep", QuickSetupDemos.Keep()),
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
        File.Delete(ProfileStore.Path); settings.Vm.Load();
        settings.OpenQuickSetup(true); var view = settings.QuickSetup!; await Task.Delay(120);
        view.NextForVerification(); view.ChooseForVerification(QuickTemplate.Shanjie); await Task.Delay(120);
        var demo = view.DemoForVerification!;
        Check(demo.RunningForVerification == Motion.Animated, "the timer should run only with animations on");
        Motion.AnimationsForVerification = true; demo.RestartForVerification();
        int first = demo.FrameForVerification; await Task.Delay(900);
        Check(demo.RunningForVerification && demo.FrameForVerification != first, "the demo did not advance");
        Motion.AnimationsForVerification = false; demo.RestartForVerification(); await Task.Delay(60);
        var held = QuickSetupDemos.Template(false);
        Check(!demo.RunningForVerification && demo.FrameForVerification == held.Keyframe && demo.CaptionForVerification == held.Frames[held.Keyframe].Caption, "reduced motion should hold the key frame");
        // Every frame of every demo, each option of each question, fits its scene.
        foreach (int option in new[] { 0, 1 })
        {
            view.ChooseForVerification(option == 0 ? QuickTemplate.Shanjie : QuickTemplate.Windows); await Task.Delay(20);
            var scene = view.DemoForVerification!; scene.UpdateLayout();
            for (int i = 0; i < scene.FrameCountForVerification; i++) { scene.HoldForVerification(i); scene.UpdateLayout(); Check(scene.FitsForVerification, $"template {option} frame {i} overflows"); }
        }
        view.NextForVerification(); await Task.Delay(60);
        foreach (string key in view.RowKeysForVerification.ToList())
        {
            view.ActivateForVerification(key); await Task.Delay(40);
            for (int option = 0; option < 4; option++)
            {
                if (!view.RowEnabledForVerification(key) || option >= view.RowOptionCountForVerification(key)) break;
                view.SelectForVerification(key, option); await Task.Delay(20);
                var scene = view.DemoForVerification!; scene.UpdateLayout();
                for (int i = 0; i < scene.FrameCountForVerification; i++) { scene.HoldForVerification(i); scene.UpdateLayout(); Check(scene.FitsForVerification, $"{key} option {option} frame {i} overflows"); }
            }
        }
        Motion.AnimationsForVerification = null;
        view.SkipForVerification(); await Closed(settings);
        Check(!File.Exists(ProfileStore.Path), "skipping after trying every option wrote settings");
    }

    /// Each step in both themes, for a fresh install and an upgrader, held at telling frames; also frames in the
    /// middle of a step change in each direction and of the finish.
    static async Task Capture(MainWindow settings, string output)
    {
        string folder = Path.Combine(output, "quick-setup"); Directory.CreateDirectory(folder);
        async Task Shot(string file) { await Task.Delay(120); await VisualVerifier.Capture(settings.CaptureElement, Path.Combine(folder, file + ".png")); }
        foreach (int theme in new[] { 1, 2 })
        {
            string name = theme == 1 ? "light" : "dark";
            File.Delete(ProfileStore.Path); settings.Vm.Load(); settings.Vm.Theme = theme;
            Motion.AnimationsForVerification = false;
            settings.OpenQuickSetup(); var view = settings.QuickSetup!; await Task.Delay(300);
            await Shot($"01-welcome-{name}");
            view.NextForVerification(); view.ChooseForVerification(QuickTemplate.Shanjie);
            await Shot($"02-template-{name}");
            view.NextForVerification();
            var choices = new (string Key, int Choice, string File)[] { ("space", 1, "03-space"), ("right", 2, "04-right"), ("mix", 1, "05-mix"), ("shift", 0, "06-shift"), ("learning", 0, "07-learning"), ("panel", 1, "08-panel-forced") };
            foreach (var (key, choice, file) in choices)
            {
                view.ActivateForVerification(key); view.SelectForVerification(key, choice);
                if (theme == 2 && file is not ("04-right" or "05-mix" or "08-panel-forced")) continue;
                await Shot($"{file}-{name}");
            }
            view.NextForVerification();
            await Shot($"09-finish-{name}");
            // The same moments with motion: a forward and a backward step change, then the finish, frozen part way.
            Motion.AnimationsForVerification = true;
            view.BackForVerification(); await Task.Delay(800);
            view.ActivateForVerification("shift"); await Task.Delay(800);
            view.NextForVerification(); Check(view.HoldForVerification(0.3f) > 0, "nothing moved between steps"); await Shot($"10-forward-mid-{name}");
            view.BackForVerification(); view.HoldForVerification(0.3f); await Shot($"11-back-mid-{name}");
            view.ActivateForVerification("panel"); await Task.Delay(100);
            view.NextForVerification();
            foreach (var (at, label) in new[] { (0.12f, "a"), (0.3f, "b"), (0.5f, "c"), (0.7f, "d") })
            {
                view.HoldForVerification(at); await Shot($"12-finish-mid-{label}-{name}");
            }
            view.HoldForVerification(1); await Shot($"13-finish-end-{name}");
            view.NextForVerification(); await Closed(settings);
            await Shot($"14-handoff-{name}");
            // An upgrader's template step offers keeping and says what a template replaces.
            Motion.AnimationsForVerification = false;
            File.WriteAllText(ProfileStore.Path, Tuned.Replace("theme=2", "theme=" + theme)); settings.Vm.Load();
            settings.OpenQuickSetup(); view = settings.QuickSetup!; await Task.Delay(300);
            view.NextForVerification(); view.ChooseForVerification(QuickTemplate.Windows);
            await Shot($"02-template-upgrade-{name}");
            view.SkipForVerification(); await Closed(settings);
            Motion.AnimationsForVerification = null;
        }
        settings.Vm.Theme = 0;
    }
}
