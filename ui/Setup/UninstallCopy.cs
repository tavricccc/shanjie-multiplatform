using System.Diagnostics;
using System.Security.Cryptography;
namespace Shanjie_UI.Setup;

/// <summary>
/// Uninstalling moves the version folder this window runs from, so the window first copies
/// itself and the manager to a temporary folder and continues from there.
/// </summary>
internal static class UninstallCopy
{
    private const string Prefix = "ShanjieUninstall-";
    private const string Marker = "shanjie-uninstall-copy.txt";
    private static string Root => Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, ".."));
    internal static bool Running => File.Exists(Path.Combine(Root, Marker));

    /// <summary>Copies ui and the manager, starts the copy and returns; the caller exits.</summary>
    internal static void Relaunch()
    {
        string copy = Path.Combine(Path.GetTempPath(), Prefix + RandomNumberGenerator.GetHexString(16, true));
        CopyTree(AppContext.BaseDirectory, Path.Combine(copy, "ui"));
        Directory.CreateDirectory(Path.Combine(copy, "tools"));
        File.Copy(SetupRunner.Manager, Path.Combine(copy, "tools", "shanjie-manager.exe"));
        File.WriteAllText(Path.Combine(copy, Marker), Root);
        Process.Start(new ProcessStartInfo(Path.Combine(copy, "ui", Path.GetFileName(Environment.ProcessPath!)), "--uninstall") { UseShellExecute = false });
    }

    private static void CopyTree(string source, string destination)
    {
        Directory.CreateDirectory(destination);
        foreach (var file in Directory.EnumerateFiles(source))
            File.Copy(file, Path.Combine(destination, Path.GetFileName(file)));
        foreach (var folder in Directory.EnumerateDirectories(source))
            if (!new DirectoryInfo(folder).Attributes.HasFlag(FileAttributes.ReparsePoint))
                CopyTree(folder, Path.Combine(destination, Path.GetFileName(folder)));
    }

    /// <summary>
    /// Removes this temporary copy once the window has exited. Only a folder this class created
    /// under the user's temp directory qualifies; the install itself is never touched here.
    /// </summary>
    internal static void RemoveAfterExit()
    {
        if (!Running) return;
        string root = Root.TrimEnd(Path.DirectorySeparatorChar);
        string temp = Path.GetFullPath(Path.GetTempPath()).TrimEnd(Path.DirectorySeparatorChar);
        if (!string.Equals(Path.GetDirectoryName(root), temp, StringComparison.OrdinalIgnoreCase) || !Path.GetFileName(root).StartsWith(Prefix, StringComparison.Ordinal)) return;
        string cmd = Path.Combine(Environment.SystemDirectory, "cmd.exe");
        Process.Start(new ProcessStartInfo(cmd, $"/d /c ping -n 4 127.0.0.1 >nul & rd /s /q \"{root}\"") { UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = temp });
    }
}
