using System.Reflection;
using System.Text;
namespace Shanjie_UI;

/// Startup metadata only: never reads the profile, dictionary, learning or typed content.
internal static class StartupDiagnostics
{
    internal static string Describe(Exception error, string stage)
    {
        var text = new StringBuilder();
        text.AppendLine($"UTC: {DateTimeOffset.UtcNow:O}");
        text.AppendLine($"Stage: {stage}");
        text.AppendLine($"Version: {typeof(App).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion}");
        text.AppendLine($"Executable: {Environment.ProcessPath}");
        text.AppendLine($"Base directory: {AppContext.BaseDirectory}");
        text.AppendLine($"Working directory: {Environment.CurrentDirectory}");
        text.AppendLine($"Runtime resource base: {Environment.GetEnvironmentVariable("MICROSOFT_WINDOWSAPPRUNTIME_BASE_DIRECTORY")}");
        try { text.AppendLine($"Package identity: {Windows.ApplicationModel.Package.Current.Id.FullName}"); }
        catch (Exception) { text.AppendLine("Package identity: none"); }
        string pri = Path.Combine(AppContext.BaseDirectory, "Shanjie.UI.pri");
        text.AppendLine($"PRI: {pri}; exists={File.Exists(pri)}");
        try
        {
            foreach (System.Diagnostics.ProcessModule module in System.Diagnostics.Process.GetCurrentProcess().Modules)
                if (module.ModuleName is "Microsoft.UI.Xaml.dll" or "MRTCore.dll" or "Microsoft.WindowsAppRuntime.dll")
                    text.AppendLine($"Module: {module.ModuleName}; version={module.FileVersionInfo.FileVersion}; path={module.FileName}");
        }
        catch (Exception) { text.AppendLine("Loaded resource module metadata unavailable."); }
        text.AppendLine(error.ToString());
        for (Exception? current = error; current is not null; current = current.InnerException)
        {
            text.AppendLine($"HRESULT: 0x{current.HResult:X8}");
            foreach (string key in new[] { "Description", "RestrictedDescription", "RestrictedErrorReference" })
                if (current.Data.Contains(key)) text.AppendLine($"{key}: {current.Data[key]}");
        }
        return text.ToString();
    }

    internal static string? Write(Exception error, string stage)
    {
        try
        {
            // Do not depend on the native profile bridge: it may be the failed startup component.
            string local = Environment.GetEnvironmentVariable("SHANJIE_TEST_PROFILE") == "1"
                ? Environment.GetEnvironmentVariable("LOCALAPPDATA") ?? throw new InvalidOperationException("Missing isolated folder")
                : Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            if (string.IsNullOrEmpty(local)) return null;
            string folder = Path.Combine(local, "Shanjie", "Logs");
            Directory.CreateDirectory(folder);
            string path = Path.Combine(folder, $"ui-startup-{DateTime.UtcNow:yyyyMMdd-HHmmss}-{Guid.NewGuid():N}.log");
            File.WriteAllText(path, Describe(error, stage), new UTF8Encoding(false));
            return path;
        }
        catch (Exception) { return null; } // Never replace the original exception with a logging failure.
    }

    internal static void Report(Exception error, string stage, string title)
    {
        string? log = Write(error, stage);
        string message = error.Message + (log is null ? "\n\n診斷日誌無法寫入。" : "\n\n完整診斷日誌：\n" + log);
        SettingsInstance.MessageBox(0, message, title, 0x10);
    }
}
