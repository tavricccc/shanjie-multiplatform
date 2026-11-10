namespace Shanjie_UI;
internal static class Program
{
    [STAThread]
    private static void Main()
    {
        try
        {
            if (DesktopActivation.RelaunchIfPackaged(out int exitCode)) { Environment.Exit(exitCode); return; }
            WinRT.ComWrappersSupport.InitializeComWrappers();
            Microsoft.UI.Xaml.Application.Start(parameters =>
            {
                var context = new Microsoft.UI.Dispatching.DispatcherQueueSynchronizationContext(
                    Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread());
                SynchronizationContext.SetSynchronizationContext(context);
                try { _ = new App(); }
                catch (Exception error) { StartupDiagnostics.Report(error, "application-resources", "善解設定未能開啟"); Environment.Exit(1); }
            });
        }
        catch (Exception error) { StartupDiagnostics.Report(error, "application-start", "善解設定未能開啟"); }
    }
}
