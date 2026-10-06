using System.Runtime.InteropServices;
using System.Text;
namespace Shanjie_UI.Models;
public static class ProfileStore
{
    public static string Path => System.IO.Path.Combine(Environment.GetEnvironmentVariable("LOCALAPPDATA")!, "Shanjie", "profile.ini");
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern uint GetPrivateProfileString(string section, string key, string defaultValue, StringBuilder value, uint size, string path);
    public static string Read(string section, string key, string defaultValue)
    {
        var value = new StringBuilder(256);
        GetPrivateProfileString(section, key, defaultValue, value, 256, Path);
        return value.ToString();
    }
    public static bool Flag(string section, string key, bool defaultValue) => Read(section, key, defaultValue ? "1" : "0") == "1";
    public static int Number(string section, string key, int defaultValue) => int.Parse(Read(section, key, defaultValue.ToString()));
    public static void Save(string content)
    {
        Directory.CreateDirectory(System.IO.Path.GetDirectoryName(Path)!);
        var temporary = Path + ".tmp";
        File.WriteAllText(temporary, content.Replace("\r\n", "\n").Replace("\n", "\r\n"), new UTF8Encoding(false));
        File.Move(temporary, Path, true);
    }
}
