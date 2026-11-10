using Microsoft.UI.Xaml;
namespace Shanjie_UI;
public partial class App : Application
{
    private object? window;
    private SettingsInstance? instance;
    public App()
    {
        UnhandledException += (_, args) => StartupDiagnostics.Write(args.Exception, "unhandled-ui");
        InitializeComponent();
    }
    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        // Pop-ups (drop-downs, tool tips) do not inherit the window's font; they read the theme font. Resources are
        // only available once the app has launched.
        if (Glass.GlassWindow.Font is { } font) Resources["ContentControlThemeFontFamily"] = font;
        var arguments = Environment.GetCommandLineArgs();
        if (arguments.Length == 3 && arguments[1] == "--verify-startup")
        {
            string output = Path.GetFullPath(arguments[2]); Directory.CreateDirectory(output);
            Environment.SetEnvironmentVariable("LOCALAPPDATA", Path.Combine(output, "profile"));
            Environment.SetEnvironmentVariable("SHANJIE_TEST_PROFILE", "1");
            try { Verification.StartupVerifier.Resources(output); var main = new MainWindow(); window = main; await Verification.StartupVerifier.Run(main, output); File.WriteAllText(Path.Combine(output, "result.txt"), "PASS"); }
            catch (Exception error) { File.WriteAllText(Path.Combine(output, "result.txt"), StartupDiagnostics.Describe(error, "verify-startup")); }
            Environment.Exit(File.ReadAllText(Path.Combine(output, "result.txt")) == "PASS" ? 0 : 1);
        }
        else if (arguments.Length == 4 && arguments[1] is "--verify-models" or "--verify-model-v2")
        {
            string output=Path.GetFullPath(arguments[2]);Directory.CreateDirectory(output);
            Environment.SetEnvironmentVariable("LOCALAPPDATA",Path.Combine(output,"profile"));
            Environment.SetEnvironmentVariable("SHANJIE_TEST_PROFILE","1");
            int code=0;
            try{window=new MainWindow();if(arguments[1]=="--verify-model-v2")await Verification.ModelVerifier.RunVersionTwo((MainWindow)window,output,Path.GetFullPath(arguments[3]));else await Verification.ModelVerifier.Run((MainWindow)window,output,Path.GetFullPath(arguments[3]));File.WriteAllText(Path.Combine(output,"result.txt"),"PASS");}
            catch(Exception error){code=1;File.WriteAllText(Path.Combine(output,"result.txt"),error.ToString());}
            Environment.Exit(code);
        }
        else if (arguments.Length == 4 && arguments[1] == "--verify-instance")
        {
            string output=Path.GetFullPath(arguments[2]); Directory.CreateDirectory(output);
            Environment.SetEnvironmentVariable("LOCALAPPDATA",Path.Combine(output,"profile"));
            Environment.SetEnvironmentVariable("SHANJIE_TEST_PROFILE","1");
            Environment.SetEnvironmentVariable("SHANJIE_INSTANCE_TRACE",Path.Combine(output,$"instance-{Environment.ProcessId}.log"));
            try {
                instance=await SettingsInstance.Open();
                if (instance is null) { File.WriteAllText(Path.Combine(output,$"forwarded-{Environment.ProcessId}.txt"),"PASS"); Environment.Exit(0); return; }
                if (arguments[3]!="owner") throw new InvalidDataException("A secondary process created a duplicate owner");
                var main=new MainWindow(); window=main;
                main.AppWindow.Move(new Windows.Graphics.PointInt32(-12000,-12000)); main.AppWindow.Show(false);
                main.Vm.DictionaryText="ㄏㄨㄟˊ-ㄐㄧㄚ 回家"; main.Vm.HasChanges=true;
                instance.Listen(main); File.WriteAllText(Path.Combine(output,"ready.txt"),Environment.ProcessId.ToString());
                var until=DateTime.UtcNow.AddSeconds(35);
                while(instance.ActivationCount<4 && DateTime.UtcNow<until) await Task.Delay(50);
                if(instance.ActivationCount!=4 || main.Vm.DictionaryText!="ㄏㄨㄟˊ-ㄐㄧㄚ 回家" || !main.Vm.HasChanges)
                    throw new InvalidDataException($"Activation count={instance.ActivationCount}; draft={main.Vm.DictionaryText}; hasChanges={main.Vm.HasChanges}");
                File.WriteAllText(Path.Combine(output,"result.txt"),"PASS four forwards and unsaved draft"); main.Close();
            }
            catch(Exception error) { File.WriteAllText(Path.Combine(output,arguments[3]=="owner"?"result.txt":$"secondary-{Environment.ProcessId}-error.txt"),error.ToString()); instance?.Dispose(); }
            Environment.Exit(arguments[3]=="owner" && File.ReadAllText(Path.Combine(output,"result.txt")).StartsWith("PASS")?0:1);
        }
        else if (arguments.Length == 4 && arguments[1] == "--verify-model-startup")
        {
            string output = Path.GetFullPath(arguments[2]);
            Environment.SetEnvironmentVariable("LOCALAPPDATA", Path.Combine(output, "profile"));
            Environment.SetEnvironmentVariable("SHANJIE_TEST_PROFILE", "1");
            string result = Path.Combine(output, $"result-{arguments[3]}.txt");
            try { window = new MainWindow(); await Verification.ModelStartupVerifier.Run((MainWindow)window, arguments[3]); File.WriteAllText(result, "PASS"); }
            catch (Exception error) { File.WriteAllText(result, error.ToString()); }
            Environment.Exit(0);
        }
        else if (arguments.Length == 3 && arguments[1] == "--verify-reliability")
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
        else if (arguments.Length is 3 or 4 && arguments[1] == "--verify-ui")
        {
            string output = Path.GetFullPath(arguments[2]); Directory.CreateDirectory(output);
            Environment.SetEnvironmentVariable("LOCALAPPDATA", Path.Combine(output, "profile"));
            Environment.SetEnvironmentVariable("SHANJIE_TEST_PROFILE", "1");
            DebugSettings.IsXamlResourceReferenceTracingEnabled = true;
            File.WriteAllText(Path.Combine(output, "result.txt"), "RUNNING");
            DebugSettings.XamlResourceReferenceFailed += (_, e) => File.AppendAllText(Path.Combine(output, "resources.txt"), e.Message + "\n");
            try { window = new MainWindow(); await Verification.VisualVerifier.Run((MainWindow)window, output, arguments.Length == 4 ? arguments[3] : null); File.WriteAllText(Path.Combine(output, "result.txt"), "PASS"); }
            catch (Exception error) { File.WriteAllText(Path.Combine(output, "result.txt"), error.ToString() + "\n" + string.Join("\n", error.Data.Keys.Cast<object>().Select(key => key + ": " + error.Data[key]))); }
            Exit();
        }
        else if (arguments.Length == 4 && arguments[1] == "--verify-quick-setup")
        {
            string output = Path.GetFullPath(arguments[2]); Directory.CreateDirectory(output);
            Environment.SetEnvironmentVariable("LOCALAPPDATA", Path.Combine(output, "profile"));
            Environment.SetEnvironmentVariable("SHANJIE_TEST_PROFILE", "1");
            string result = Path.Combine(output, $"result-{arguments[3]}.txt");
            try { window = new MainWindow(); await Verification.QuickSetupVerifier.RunPhase((MainWindow)window, arguments[3], output); File.WriteAllText(result, "PASS"); }
            catch (Exception error) { File.WriteAllText(result, error.ToString()); }
            Environment.Exit(0);
        }
        else if (arguments.Length == 3 && arguments[1] == "--verify-setup")
        {
            string output = Path.GetFullPath(arguments[2]); Directory.CreateDirectory(output);
            int code = 0;
            try { await Verification.SetupVerifier.Run(output); File.WriteAllText(Path.Combine(output, "result.txt"), "PASS"); }
            catch (Exception error) { code = 1; File.WriteAllText(Path.Combine(output, "result.txt"), error.ToString()); }
            // Several hidden test windows are still open; Application.Exit does not end the process here.
            Environment.Exit(code);
        }
        else if ((arguments.Length == 2 && arguments[1] is "--setup" or "--uninstall") || (arguments.Length == 3 && arguments[1] == "--setup" && arguments[2] == "--auto-update"))
        {
            bool uninstall = arguments[1] == "--uninstall";
            try {
                // Uninstalling moves this version folder, so continue from a temporary copy.
                if (uninstall && !Setup.UninstallCopy.Running) { Setup.UninstallCopy.Relaunch(); Exit(); return; }
                var setup = new SetupWindow(uninstall); window = setup; setup.Host.Show(); await setup.LoadStatus();
                if (arguments.Length == 3) await setup.InstallUpdate();
            }
            catch (Exception error) { StartupDiagnostics.Report(error, uninstall ? "uninstall" : "setup", uninstall ? "解除安裝善解未能開始" : "安裝善解未能開始"); Exit(); }
        }
        else if (arguments.Length is 2 or 3 && arguments[1] == "--check-update")
        {
            // Runs unattended: anything going wrong just waits for the next scheduled check.
            try {
                var card = await Setup.UpdateNotice.Check(arguments.Length == 3 && arguments[2] == "--from-settings");
                if (card is null) { Exit(); return; }
                window = card; card.Host.Show();
            }
            catch (Exception) { Exit(); }
        }
        else {
            try {
                bool about = arguments.Length == 2 && arguments[1] == "--about";
                instance = await SettingsInstance.Open(about ? "ABOUT" : "SHOW");
                if (instance is null) { Exit(); return; }
                var main = new MainWindow(); window = main; instance.Listen(main);
                if (about) main.ShowSection(MainWindow.Pages.About);
                else if (arguments.Contains(Setup.QuickSetupGate.OpenArgument)) main.OpenQuickSetup(true);
                else if (Setup.QuickSetupGate.Due(arguments)) main.OpenQuickSetup();
                main.Host.Show();
                if (!about) Setup.UpdateNotice.CheckInBackground();
            }
            catch (Exception error) { StartupDiagnostics.Report(error,"settings","善解設定未能開啟"); instance?.Dispose(); Exit(); }
        }
    }
}
