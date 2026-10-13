using Microsoft.UI.Xaml.Controls;
using Shanjie_UI.Models;
using Windows.Graphics;
namespace Shanjie_UI.Verification;
internal static class ReliabilityVerifier
{
    public static async Task Run(MainWindow settings, string output)
    {
        settings.AppWindow.Move(new PointInt32(-12000, -12000)); settings.AppWindow.Show(false); await Task.Delay(300);
        string folder = Path.GetDirectoryName(ProfileStore.Path)!; Directory.CreateDirectory(folder);
        File.WriteAllText(ProfileStore.Path, ProfileStore.Defaults(true).ToString());
        byte[] profile = File.ReadAllBytes(ProfileStore.Path);
        settings.Vm.DictionaryText = "ㄋㄧˇ 你\rㄏㄠˇ 好"; settings.Vm.HasChanges = true;
        string draft = settings.Vm.DictionaryText;
        settings.ShowSection(0);
        foreach (uint mask in new uint[] { 1, 2 }) {
            settings.SetInstalledForVerification(3); settings.Vm.Keyboard = mask == 1 ? 2 : 1;
            settings.SetInstalledForVerification(mask); await Task.Delay(80);
            var options = settings.InstalledOptionsForVerification;
            if (options.standard != (mask == 1) || options.eten != (mask == 2) || settings.Vm.Keyboard != 0)
                throw new InvalidDataException("Unavailable keyboard option or fallback binding is incorrect");
            if (!settings.Vm.HasChanges || !settings.Vm.Status.Contains("尚未安裝")) throw new InvalidDataException("Fallback was not explained");
            foreach (int theme in new[] { 1, 2 }) {
                settings.Vm.Theme = theme; await Task.Delay(180);
                await VisualVerifier.Capture(settings.CaptureElement, Path.Combine(output, $"keyboard-{mask}-theme-{theme}.png"));
            }
            settings.Vm.Keyboard = mask == 1 ? 2 : 1;
            bool rejected = false; try { settings.Vm.Save(); } catch (InvalidDataException) { rejected = true; }
            if (!rejected) throw new InvalidDataException("Unavailable keyboard preference was saved");
        }
        settings.SetInstalledForVerification(3); settings.ShowDictionary();
        var learning = Path.Combine(folder, "learning.tsv");
        File.WriteAllText(learning, "public fixture learning data");
        string name = "Local\\ShanjieLearning-" + folder.ToUpperInvariant().Replace('\\', '_');
        using (var mutex = new Mutex(false, name)) {
            if (!mutex.WaitOne(1000)) throw new TimeoutException("Fixture learning lock unavailable");
            try { await settings.ClearLearningConfirmed(); }
            finally { mutex.ReleaseMutex(); }
        }
        if (settings.FeedbackForVerification != InfoBarSeverity.Error || !settings.Vm.Status.Contains("其他程式正在更新"))
            throw new InvalidDataException("Native learning timeout escaped the handler or was reported as success");
        CheckDraft(); if (!File.Exists(learning)) throw new InvalidDataException("Busy clear changed learning data");
        await VisualVerifier.Capture(settings.CaptureElement, Path.Combine(output, "learning-timeout.png"));
        Directory.CreateDirectory(Path.Combine(folder, "learning.tsv.tmp"));
        await settings.ClearLearningConfirmed();
        if (settings.FeedbackForVerification != InfoBarSeverity.Error || !settings.Vm.Status.Contains("清除操作未完成"))
            throw new InvalidDataException("Partial clear failure was not reported");
        CheckDraft(); if (File.Exists(learning)) throw new InvalidDataException("Partial clear fixture did not reach the expected stage");
        var archive = Path.Combine(folder, ".deleted_files"); Directory.CreateDirectory(archive);
        Directory.Move(Path.Combine(folder, "learning.tsv.tmp"), Path.Combine(archive, "blocked-temporary-" + Guid.NewGuid().ToString("N")));
        await settings.ClearLearningConfirmed(); CheckDraft();
        if (settings.FeedbackForVerification != InfoBarSeverity.Success || !File.Exists(Path.Combine(folder, "learning.epoch")))
            throw new InvalidDataException("Clear retry did not complete or update the epoch");
        await VisualVerifier.Capture(settings.CaptureElement, Path.Combine(output, "learning-retry.png"));
        await DictionaryVerifier.Run(settings, Path.Combine(output, "dictionary"), null);
        File.WriteAllText(Path.Combine(output, "checks.txt"), "PASS keyboard masks and fallback; native learning timeout; partial clear and retry; draft and profile preservation; actual multiline TextBox round trip");
        void CheckDraft() {
            if (settings.Vm.DictionaryText != draft || !settings.Vm.HasChanges || !File.ReadAllBytes(ProfileStore.Path).SequenceEqual(profile))
                throw new InvalidDataException("Learning operation changed the unsaved draft or profile");
        }
    }
}
