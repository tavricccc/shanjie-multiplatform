using Microsoft.UI.Xaml;
namespace Shanjie_UI;
public partial class App : Application
{
    private Window? window;
    private SettingsInstance? instance;
    public App() => InitializeComponent();
    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        var arguments = Environment.GetCommandLineArgs();
        if (arguments.Length == 4 && arguments[1] is "--verify-models" or "--verify-model-v2")
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
                instance.Listen(window); File.WriteAllText(Path.Combine(output,"ready.txt"),Environment.ProcessId.ToString());
                var until=DateTime.UtcNow.AddSeconds(35);
                while(instance.ActivationCount<4 && DateTime.UtcNow<until) await Task.Delay(50);
                if(instance.ActivationCount!=4 || main.Vm.DictionaryText!="ㄏㄨㄟˊ-ㄐㄧㄚ 回家" || !main.Vm.HasChanges)
                    throw new InvalidDataException($"Activation count={instance.ActivationCount}; draft={main.Vm.DictionaryText}; hasChanges={main.Vm.HasChanges}");
                File.WriteAllText(Path.Combine(output,"result.txt"),"PASS four forwards and unsaved draft"); main.Close();
            }
            catch(Exception error) { File.WriteAllText(Path.Combine(output,arguments[3]=="owner"?"result.txt":$"secondary-{Environment.ProcessId}-error.txt"),error.ToString()); instance?.Dispose(); }
            Environment.Exit(arguments[3]=="owner" && File.ReadAllText(Path.Combine(output,"result.txt")).StartsWith("PASS")?0:1);
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
        else if (arguments.Length == 3 && arguments[1] == "--verify-setup")
        {
            string output = Path.GetFullPath(arguments[2]); Directory.CreateDirectory(output);
            int code = 0;
            try { await Verification.SetupVerifier.Run(output); File.WriteAllText(Path.Combine(output, "result.txt"), "PASS"); }
            catch (Exception error) { code = 1; File.WriteAllText(Path.Combine(output, "result.txt"), error.ToString()); }
            // Several hidden test windows are still open; Application.Exit does not end the process here.
            Environment.Exit(code);
        }
        else if (arguments.Length == 2 && arguments[1] is "--setup" or "--uninstall")
        {
            bool uninstall = arguments[1] == "--uninstall";
            try {
                // Uninstalling moves this version folder, so continue from a temporary copy.
                if (uninstall && !Setup.UninstallCopy.Running) { Setup.UninstallCopy.Relaunch(); Exit(); return; }
                var setup = new SetupWindow(uninstall); window = setup; setup.Activate(); await setup.LoadStatus();
            }
            catch (Exception error) { SettingsInstance.MessageBox(0, error.Message, uninstall ? "解除安裝善解未能開始" : "安裝善解未能開始", 0x10); Exit(); }
        }
        else {
            try {
                instance = await SettingsInstance.Open();
                if (instance is null) { Exit(); return; }
                window = new MainWindow(); instance.Listen(window); window.Activate();
            }
            catch (Exception error) { SettingsInstance.MessageBox(0,error.Message,"善解設定未能開啟",0x10); instance?.Dispose(); Exit(); }
        }
    }
}
