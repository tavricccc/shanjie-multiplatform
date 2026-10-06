using Microsoft.UI.Xaml;
namespace Shanjie_UI;
public partial class App : Application
{
    private Window? window;
    public App() => InitializeComponent();
    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        var arguments = Environment.GetCommandLineArgs();
        if (arguments.Length == 3 && arguments[1] == "--verify-ui")
        {
            string output = Path.GetFullPath(arguments[2]); Directory.CreateDirectory(output);
            Environment.SetEnvironmentVariable("LOCALAPPDATA", Path.Combine(output, "profile"));
            DebugSettings.IsXamlResourceReferenceTracingEnabled = true;
            File.WriteAllText(Path.Combine(output, "result.txt"), "RUNNING");
            DebugSettings.XamlResourceReferenceFailed += (_, e) => File.AppendAllText(Path.Combine(output, "resources.txt"), e.Message + "\n");
            try { window = new MainWindow(); await Verification.VisualVerifier.Run((MainWindow)window, output); File.WriteAllText(Path.Combine(output, "result.txt"), "PASS"); }
            catch (Exception error) { File.WriteAllText(Path.Combine(output, "result.txt"), error.ToString() + "\n" + string.Join("\n", error.Data.Keys.Cast<object>().Select(key => key + ": " + error.Data[key]))); }
            Exit();
        }
        else if (arguments.Length == 3 && arguments[1] == "--candidates") window = new CandidateWindow(nint.Parse(arguments[2]));
        else { window = new MainWindow(); window.Activate(); }
    }
}
