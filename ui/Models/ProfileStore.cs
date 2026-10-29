using Shanjie_UI.Interop;
namespace Shanjie_UI.Models;
public static class ProfileStore
{
    public static string Path => Native.Take(Native.sj_profile_path());
    public static ProfileDocument Defaults(bool windows)
    {
        using var resource = typeof(ProfileStore).Assembly.GetManifestResourceStream($"Profiles.{(windows ? "windows" : "shanjie")}.ini")!;
        using var reader = new StreamReader(resource); return ProfileDocument.Parse(reader.ReadToEnd());
    }
    public static ProfileDocument Load() => ProfileDocument.Parse(Native.Take(Native.sj_profile_load_text()));
    public static void Save(string content) => Native.Check(Native.sj_profile_save(content));
    public static void ClearLearning() => Native.Check(Native.sj_clear_learning());
    public static void Restart() => Native.Check(Native.sj_restart());
}
