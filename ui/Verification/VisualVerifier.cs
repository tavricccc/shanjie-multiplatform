using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;
using Shanjie_UI.Interop;
using Windows.Graphics;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;
namespace Shanjie_UI.Verification;
internal static class VisualVerifier
{
    internal static async Task Capture(FrameworkElement element, string path)
    {
        element.UpdateLayout();
        var bitmap = new RenderTargetBitmap(); await bitmap.RenderAsync(element);
        // An offscreen window needs a rendered frame before its native navigation transition settles.
        await Task.Delay(600); await bitmap.RenderAsync(element);
        var pixels = await bitmap.GetPixelsAsync();
        using var stream = new InMemoryRandomAccessStream();
        var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, stream);
        using var reader = DataReader.FromBuffer(pixels); byte[] bytes = new byte[pixels.Length]; reader.ReadBytes(bytes);
        encoder.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied, (uint)bitmap.PixelWidth, (uint)bitmap.PixelHeight, 96, 96, bytes);
        await encoder.FlushAsync(); stream.Seek(0);
        using var output = File.Create(path); await stream.AsStreamForRead().CopyToAsync(output);
    }
    /// A rescan after download rebuilds the list; the selector must still show the draft model, without prompting or saving.
    private static async Task VerifyModelRescans(MainWindow settings)
    {
        var installed = settings.Vm.AvailableModels.ToList();
        Models.ModelChoice a = new("fixture-a", "測試配套 A（已下載）"), b = new("fixture-b", "測試配套 B（已下載）");
        settings.Vm.RefreshModels([a, b]); settings.Vm.SelectModel(b.Id); settings.Vm.DictionaryText = "ㄇㄠ 貓";
        string draft = settings.Vm.DictionaryText; byte[] saved = File.ReadAllBytes(Models.ProfileStore.Path);
        async Task Expect(string id, string what)
        {
            await Task.Delay(60);
            if (settings.ShownModelForVerification != id || settings.Vm.ModelId != b.Id || settings.Vm.DictionaryText != draft || !settings.Vm.HasChanges)
                throw new InvalidDataException(what + $": shown={settings.ShownModelForVerification}, draft={settings.Vm.ModelId}");
        }
        await Expect(b.Id, "Selector after selection");
        settings.Vm.RefreshModels([a, b]); await Expect(b.Id, "Rescan with the same index");
        settings.ShowSection(MainWindow.Pages.Appearance); await Task.Delay(60); settings.Vm.RefreshModels([a, b]);
        settings.ShowSection(MainWindow.Pages.General); await Expect(b.Id, "Rescan while another category is shown");
        settings.Vm.RefreshModels([a, b]); settings.Vm.RefreshModels([a, b]); await Expect(b.Id, "Consecutive rescans");
        settings.Vm.RefreshModels([b, a]); await Expect(b.Id, "Reordered list");
        if (settings.Vm.ModelVariant != 1) throw new InvalidDataException("Reordered list kept a stale index");
        settings.Vm.RefreshModels([a]);
        if (settings.Vm.ModelId != b.Id || !settings.Vm.ModelHelp.Contains("不存在")) throw new InvalidDataException("Missing model lost the draft or its fallback notice");
        settings.Vm.RefreshModels([a, b]); await Expect(b.Id, "Model listed again");
        if (!File.ReadAllBytes(Models.ProfileStore.Path).SequenceEqual(saved)) throw new InvalidDataException("Rescans saved the profile");
        settings.Vm.RefreshModels(installed.Where(m => m.Id != Models.ModelCatalog.Builtin)); settings.Vm.Load();
    }
    public static async Task Run(MainWindow settings, string output)
    {
        // Keep verification away from the user's windows and never activate the test surfaces.
        settings.AppWindow.Move(new PointInt32(-12000, -12000)); settings.AppWindow.Show(false);
        await Task.Delay(500);
        Directory.CreateDirectory(Path.GetDirectoryName(Models.ProfileStore.Path)!);
        string legacy = "[profile]\nschema=2\ninteraction=windows\n[engine]\nmodel=2\nranking=formal\n[input]\nlearning=1\nassociations=0\nescape_clears=0\n[appearance]\nfont_size=22\n";
        File.WriteAllText(Models.ProfileStore.Path, legacy); settings.Vm.Load();
        if (settings.Vm.Model != 5 || !settings.Vm.Learning || settings.Vm.Associations || settings.Vm.Prediction || settings.Vm.Escape != 2 || !settings.Vm.ShiftPunctuation || File.ReadAllText(Models.ProfileStore.Path) != legacy)
            throw new InvalidDataException("Read-only schema 2 migration did not preserve settings or original file");
        settings.Vm.Save();
        if (File.ReadAllText(Models.ProfileStore.Path + ".schema2.bak") != legacy)
            throw new InvalidDataException("Explicit save did not retain the legacy profile backup");
        settings.Vm.Preset(true); settings.Vm.Save(); settings.Vm.Load();
        if (settings.Vm.Interaction != 0 || settings.Vm.Anchor != 0 || !settings.Vm.ShiftToggle || settings.Vm.CtrlSpaceToggle || settings.Vm.Model != 5 || !settings.Vm.Learning || settings.Vm.FontSize != 22 || settings.Vm.Associations || settings.Vm.Prediction || settings.Vm.ShiftPunctuation)
            throw new InvalidDataException("Windows preset/retained data round-trip failed");
        settings.Vm.Preset(false); settings.Vm.Save(); settings.Vm.Load();
        if (settings.Vm.Interaction != 1 || settings.Vm.ShiftToggle) throw new InvalidDataException("Shanjie profile round-trip failed");
        settings.LoadProfile(true);
        settings.Vm.Save(); settings.Vm.Load(); if (settings.Vm.Model != 5 || !File.ReadAllText(Models.ProfileStore.Path).Contains("layout=horizontal")) throw new InvalidDataException("Latest model/horizontal migration failed");
        foreach (int variant in Enumerable.Range(0,settings.Vm.AvailableModels.Count).Reverse()) {
            settings.SetModelForVerification(variant); await Task.Delay(60);
            if(settings.Vm.ModelVariant!=variant) throw new InvalidDataException("Model selector binding failed");
            settings.Vm.Save(); settings.Vm.Load();
            if(settings.Vm.ModelVariant!=variant || settings.Vm.ModelId!=settings.Vm.AvailableModels[variant].Id) throw new InvalidDataException("Model variant persistence failed");
            settings.Vm.Preset(true);
            if(settings.Vm.ModelVariant!=variant) throw new InvalidDataException("Operation preset replaced personal model choice");
        }
        await VerifyModelRescans(settings);
        // 0.6.0: per-key punctuation, vertical layout and a shared preset round-trip that keeps personal data.
        settings.Vm.SetAllPunctuation(false, false); settings.Vm.PlainKeys[0].IsOn = true; settings.Vm.CandidateLayout = 1; settings.Vm.Save(); settings.Vm.Load();
        if (!settings.Vm.PlainKeys[0].IsOn || settings.Vm.PlainKeys[2].IsOn || settings.Vm.CandidateLayout != 1) throw new InvalidDataException("Per-key punctuation/vertical persistence failed");
        string shared = settings.Vm.ExportPreset("驗證預設");
        if (shared.Contains("font_size") || shared.Contains("dictionary")) throw new InvalidDataException("Exported preset carries personal data");
        settings.LoadProfile(true); settings.Vm.ImportPreset(shared, "fallback"); settings.RefreshPresets();
        if (!settings.Vm.PlainKeys[0].IsOn || settings.Vm.PlainKeys[2].IsOn || settings.Vm.CandidateLayout != 1 || settings.Vm.FontSize != 22 || !File.Exists(Path.Combine(ViewModels.SettingsViewModel.PresetFolder, "驗證預設.ini")))
            throw new InvalidDataException("Shared preset import failed");
        bool rejected = false;
        try { settings.Vm.ImportPreset("[candidates]\nanchor=middle\n", "bad"); } catch (InvalidDataException) { rejected = true; }
        if (!rejected) throw new InvalidDataException("Invalid preset was accepted");
        settings.LoadProfile(true); settings.Vm.CandidateLayout = 0; settings.Vm.Save(); settings.Vm.Load();
        settings.ShowSection(MainWindow.Pages.Dictionary); settings.SetPredictionForVerification(true); await Task.Delay(60);
        if (!settings.Vm.Prediction) throw new InvalidDataException("Native toggle did not update the shared draft");
        settings.SetPredictionForVerification(false); await Task.Delay(60);
        if (settings.Vm.Prediction) throw new InvalidDataException("Native toggle off did not update the shared draft");
        settings.ShowSection(MainWindow.Pages.English); settings.SetCapsLockForVerification(false); await Task.Delay(60);
        if (settings.Vm.CapsLockEnglish) throw new InvalidDataException("Caps Lock toggle off did not update the shared draft");
        settings.SetCapsLockForVerification(true); await Task.Delay(60);
        if (!settings.Vm.CapsLockEnglish) throw new InvalidDataException("Caps Lock toggle on did not update the shared draft");
        foreach (var mode in new[] { 1, 2, 3, 0 })
        {
            settings.SetMixedModeForVerification(mode); await Task.Delay(60);
            if (settings.Vm.MixedMode != mode) throw new InvalidDataException($"Mixed mode {mode} binding failed");
            settings.Vm.Save(); settings.Vm.Load();
            if (settings.Vm.MixedMode != mode) throw new InvalidDataException($"Mixed mode {mode} persistence failed");
        }
        if (settings.Vm.SpaceCommits || settings.Vm.SpaceOperates) throw new InvalidDataException("Space should insert editable whitespace by default");
        foreach (bool enabled in new[] { true, false }) {
            settings.SetSpaceInputsZhuyinForVerification(enabled); await Task.Delay(60);
            if (settings.Vm.SpaceInputsZhuyin != enabled) throw new InvalidDataException("Space Zhuyin toggle binding failed");
            settings.ShowSection(MainWindow.Pages.Composition); settings.SetSpaceCommitsForVerification(enabled);
            settings.ShowSection(MainWindow.Pages.Candidates); settings.SetSpaceOperatesForVerification(enabled); await Task.Delay(60);
            if (settings.Vm.SpaceCommits != enabled || settings.Vm.SpaceOperates != enabled) throw new InvalidDataException("Space toggle binding failed");
            settings.Vm.Save(); settings.Vm.Load();
            if (settings.Vm.SpaceInputsZhuyin != enabled) throw new InvalidDataException("Space Zhuyin toggle persistence failed");
            if (settings.Vm.SpaceCommits != enabled || settings.Vm.SpaceOperates != enabled) throw new InvalidDataException("Space toggle persistence failed");
        }
        settings.Vm.Ranking = 0; settings.Vm.Learning = false; settings.Vm.FontSize = 18; settings.Vm.DictionaryText = "";
        settings.LoadProfile(true); settings.SaveDraft();
        foreach (int theme in new[] { 1, 2 })
        {
            settings.Vm.Theme = theme;
            for (int page = 0; page < MainWindow.Pages.Count; page++)
            {
                settings.ShowSection(page); await Task.Delay(350);
                await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-{page}.png"));
            }
            settings.ShowSection(0); settings.SaveDraft(); await Task.Delay(350);
            await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-saved.png"));
            settings.ShowSection(MainWindow.Pages.Dictionary); settings.Vm.DictionaryText = "invalid"; settings.SaveDraft(); await Task.Delay(350);
            await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-error.png"));
            settings.Vm.DictionaryText = ""; settings.LoadProfile(false); settings.ShowSection(MainWindow.Pages.Candidates); await Task.Delay(350);
            await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-shanjie.png"));
            settings.LoadProfile(true); await Task.Delay(350);
            await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-error-then-preset.png"));
            settings.Vm.Theme = theme; settings.ShowModelOptions(); await Task.Delay(350);
            await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-models.png"));
            settings.ShowAdvancedKeys(); await Task.Delay(350); await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-advanced-keys.png"));
            settings.ShowCandidateKeys(); await Task.Delay(350); await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-candidate-keys.png"));
            settings.ShowSpaceOptions(); await Task.Delay(350); await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-space.png"));
            settings.ShowAppearance(); await Task.Delay(350); await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-appearance.png"));
            settings.ShowDictionary(); await Task.Delay(350); await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-dictionary.png"));
        }
        settings.Close();
    }
}
