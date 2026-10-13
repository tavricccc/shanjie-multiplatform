using Microsoft.UI.Xaml;
namespace Shanjie_UI;
public partial class App : Application
{
    private Window? window;
    public App() => InitializeComponent();
    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        var arguments = Environment.GetCommandLineArgs();
        if (arguments.Length == 3 && arguments[1] == "--verify-reliability")
        {
            string output = Path.GetFullPath(arguments[2]); Directory.CreateDirectory(output);
            Environment.SetEnvironmentVariable("LOCALAPPDATA", Path.Combine(output, "profile"));
            Environment.SetEnvironmentVariable("SHANJIE_TEST_PROFILE", "1");
            try { window = new MainWindow(); await Verification.ReliabilityVerifier.Run((MainWindow)window, output); File.WriteAllText(Path.Combine(output, "result.txt"), "PASS"); }
            catch (Exception error) { File.WriteAllText(Path.Combine(output, "result.txt"), error.ToString()); }
            Exit();
        }
        else if (arguments.Length is 3 or 4 && arguments[1] == "--verify-dictionary")
        {
            string output = Path.GetFullPath(arguments[2]); Directory.CreateDirectory(output);
            Environment.SetEnvironmentVariable("LOCALAPPDATA", Path.Combine(output, "profile"));
            Environment.SetEnvironmentVariable("SHANJIE_TEST_PROFILE", "1");
            try {
                window = new MainWindow();
                await Verification.DictionaryVerifier.Run((MainWindow)window, output, arguments.Length == 4 ? arguments[3] : null);
                File.WriteAllText(Path.Combine(output, "result.txt"), "PASS");
            }
            catch (Exception error) { File.WriteAllText(Path.Combine(output, "result.txt"), error.ToString()); }
            Exit();
        }
        else if (arguments.Length == 3 && arguments[1] == "--verify-ui")
        {
            string output = Path.GetFullPath(arguments[2]); Directory.CreateDirectory(output);
            Environment.SetEnvironmentVariable("LOCALAPPDATA", Path.Combine(output, "profile"));
            Environment.SetEnvironmentVariable("SHANJIE_TEST_PROFILE", "1");
            DebugSettings.IsXamlResourceReferenceTracingEnabled = true;
            File.WriteAllText(Path.Combine(output, "result.txt"), "RUNNING");
            DebugSettings.XamlResourceReferenceFailed += (_, e) => File.AppendAllText(Path.Combine(output, "resources.txt"), e.Message + "\n");
            try { window = new MainWindow(); await Verification.VisualVerifier.Run((MainWindow)window, output); File.WriteAllText(Path.Combine(output, "result.txt"), "PASS"); }
            catch (Exception error) { File.WriteAllText(Path.Combine(output, "result.txt"), error.ToString() + "\n" + string.Join("\n", error.Data.Keys.Cast<object>().Select(key => key + ": " + error.Data[key]))); }
            Exit();
        }
        else { window = new MainWindow(); window.Activate(); }
    }
}
