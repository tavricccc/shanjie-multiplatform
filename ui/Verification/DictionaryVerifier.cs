using System.Text.Json;
using Shanjie_UI.Models;
using Windows.Graphics;

namespace Shanjie_UI.Verification;
internal static class DictionaryVerifier
{
    public static async Task Run(MainWindow settings, string output, string? input)
    {
        settings.AppWindow.Move(new PointInt32(-12000, -12000));
        settings.AppWindow.Show(false);
        await Task.Delay(300);
        settings.ShowDictionary();
        var results = new List<object>();
        string synthetic = "ㄋㄧˇ 你\nㄏㄠˇ 好";
        foreach (string newline in new[] { "\n", "\r\n", "\r" })
        {
            await Check(synthetic.Replace("\n", newline), "synthetic-" + newline.Length + "-" + (int)newline[0]);
        }
        settings.SetDictionaryForVerification("ㄋㄧˇ 你好\rㄏㄠˇ 好");
        await Task.Delay(80);
        bool rejected = false;
        try { PersonalDictionary.Save(settings.Vm.DictionaryText, Path.Combine(output, "invalid")); }
        catch (InvalidDataException) { rejected = true; }
        if (!rejected) throw new InvalidDataException("Invalid row was accepted");
        if (input is not null) await Check(File.ReadAllText(input), "external");
        File.WriteAllText(Path.Combine(output, "checks.json"), JsonSerializer.Serialize(results, new JsonSerializerOptions { WriteIndented = true }));
        settings.Close();

        async Task Check(string text, string label)
        {
            settings.SetDictionaryForVerification(text);
            await Task.Delay(80);
            string actual = settings.DictionaryForVerification;
            if (settings.Vm.DictionaryText != actual) throw new InvalidDataException("TextBox draft binding differs");
            var folder = Path.Combine(output, "dictionaries", label);
            string name = PersonalDictionary.Save(settings.Vm.DictionaryText, folder);
            string saved = File.ReadAllText(Path.Combine(folder, name));
            int expectedRows = text.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries).Count(line => !string.IsNullOrWhiteSpace(line));
            int savedRows = saved.Split('\n', StringSplitOptions.RemoveEmptyEntries).Length;
            if (savedRows != expectedRows || saved.Contains('\r')) throw new InvalidDataException("Dictionary rows changed during save");
            settings.SetDictionaryForVerification(saved);
            await Task.Delay(80);
            string again = PersonalDictionary.Save(settings.Vm.DictionaryText, folder);
            if (again != name) throw new InvalidDataException("Reload and save changed the canonical dictionary");
            results.Add(new { label, rows = savedRows, textboxCR = actual.Count(c => c == '\r'), textboxLF = actual.Count(c => c == '\n'), savedLF = saved.Count(c => c == '\n'), roundTrip = true });
        }
    }
}
