using System.Runtime.InteropServices;
using System.Text;
namespace Shanjie_UI.Models;
public static class ProfileStore
{
    public static string Path => System.IO.Path.Combine(Environment.GetEnvironmentVariable("LOCALAPPDATA")!, "Shanjie", "profile.ini");
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern uint RegisterWindowMessage(string name);
    [DllImport("user32.dll")] private static extern bool SendNotifyMessage(nint window, uint message, nuint wp, nint lp);
    public static ProfileDocument Defaults(bool windows)
    {
        using var resource = typeof(ProfileStore).Assembly.GetManifestResourceStream($"Profiles.{(windows ? "windows" : "shanjie")}.ini")!;
        using var reader = new StreamReader(resource); return ProfileDocument.Parse(reader.ReadToEnd());
    }
    public static ProfileDocument Load()
    {
        if (!File.Exists(Path)) return Defaults(true);
        var profile = ProfileDocument.Parse(File.ReadAllText(Path)); int schema = profile.Number("profile", "schema", 1);
        if (schema >= 3) return profile;
        bool windows = profile.Read("profile", "interaction", "windows") == "windows"; var legacy = Defaults(windows);
        legacy.Set("engine", "model", 1); legacy.Set("engine", "prediction", false); legacy.Set("input", "associations", windows);
        foreach (string key in new[] { "shift_toggle", "ctrl_space_toggle", "shift_space_toggle", "ctrl_punctuation" }) legacy.Set("shortcuts", key, true);
        legacy.Set("candidates", "layout", "vertical"); legacy.Set("candidates", "anchor", "right"); legacy.Set("candidates", "open", "down");
        foreach (string key in new[] { "shift_punctuation", "plain_punctuation", "standalone_tones" }) legacy.Set("input", key, true);
        legacy.Set("input", "pinyin_space_converts", false); legacy.Set("shortcuts", "symbol_keyboard", true);
        legacy.Set("input", "caps_lock_english", windows); legacy.Set("input", "learning", false);
        legacy.Set("input", "space_after_commit", true); legacy.Set("shortcuts", "backquote_symbols", windows);
        legacy.Set("input", "escape", profile.Flag("input", "escape_clears", true) ? "staged" : "ignore");
        profile.FillMissing(legacy); profile.Set("profile", "schema", 3);
        profile.Remove("input", "escape_clears"); profile.Remove("candidates", "reading");
        string backup = Path + $".schema{schema}.bak"; if (!File.Exists(backup)) File.Copy(Path, backup);
        Save(profile.ToString()); return profile;
    }
    public static void Save(string content)
    {
        Directory.CreateDirectory(System.IO.Path.GetDirectoryName(Path)!);
        var temporary = Path + ".tmp";
        File.WriteAllText(temporary, content.Replace("\r\n", "\n").Replace("\n", "\r\n"), new UTF8Encoding(false));
        File.Move(temporary, Path, true);
        SendNotifyMessage((nint)0xffff, RegisterWindowMessage("Shanjie.ProfileChanged.v1"), 0, 0);
    }
    public static void ClearLearning()
    {
        string folder = System.IO.Path.GetDirectoryName(Path)!;
        using var gate = new Mutex(false, "Local\\ShanjieLearning-" + folder.ToUpperInvariant().Replace('\\', '_'));
        bool acquired;
        try { acquired = gate.WaitOne(TimeSpan.FromSeconds(2)); }
        catch (AbandonedMutexException) { acquired = true; }
        if (!acquired) throw new TimeoutException("其他程式正在更新選字學習，請稍後再試。");
        try
        {
            Directory.CreateDirectory(folder);
            foreach (string file in new[] { "learning.tsv", "learning.tsv.tmp", "learning.tsv.corrupt" }) File.Delete(System.IO.Path.Combine(folder, file));
            File.WriteAllText(System.IO.Path.Combine(folder, "learning.epoch"), Guid.NewGuid().ToString());
        }
        finally { gate.ReleaseMutex(); }
    }
}
