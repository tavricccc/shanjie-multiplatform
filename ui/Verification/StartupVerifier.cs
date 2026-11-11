using Microsoft.Windows.ApplicationModel.Resources;
namespace Shanjie_UI.Verification;
internal static class StartupVerifier
{
    internal static void Resources(string output)
    {
        foreach (bool explicitPath in new[] { false, true })
        {
            try
            {
                var manager = explicitPath ? new ResourceManager(Path.Combine(AppContext.BaseDirectory, "Shanjie.UI.pri")) : new ResourceManager();
                var map = manager.MainResourceMap;
                File.AppendAllText(Path.Combine(output, "resources.txt"), $"explicit={explicitPath}; resources={map.ResourceCount}\n");
                for (uint i = 0; i < Math.Min(3u, map.ResourceCount); i++)
                    File.AppendAllText(Path.Combine(output, "resources.txt"), $"first-resource={map.GetValueByIndex(i).Key}\n");
                foreach (string key in new[] { "Files/App.xbf", "Files/MainWindow.xbf", "Files/Themes/Sj.xbf" })
                {
                    var resource = map.TryGetValue(key);
                    File.AppendAllText(Path.Combine(output, "resources.txt"), $"{key}: kind={resource?.Kind}; bytes={resource?.ValueAsBytes.Length}\n");
                }
            }
            catch (Exception error) { File.AppendAllText(Path.Combine(output, "resources.txt"), StartupDiagnostics.Describe(error, "resource-probe")); }
        }
    }
    internal static async Task Run(MainWindow settings, string output)
    {
        settings.AppWindow.Move(new Windows.Graphics.PointInt32(-12000, -12000)); settings.AppWindow.Show(false);
        await Task.Delay(100);
        if (settings.FindName("Nav") is null || settings.FindName("ModelSelector") is null)
            throw new InvalidDataException("Main settings resources did not load");
        var error = new InvalidOperationException("Public diagnostic fixture", new IOException("Public inner exception"));
        error.Data["RestrictedDescription"] = "Public resource failure";
        string log = StartupDiagnostics.Write(error, "startup-verifier") ?? throw new IOException("Startup log not written");
        string content = File.ReadAllText(log);
        if (!content.Contains("Public inner exception") || !content.Contains("RestrictedDescription: Public resource failure") || !content.Contains("HRESULT:") || !content.Contains("PRI:"))
            throw new InvalidDataException("Incomplete startup log");
        settings.Close();
    }
}
