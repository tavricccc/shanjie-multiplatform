using System.Diagnostics;
using System.IO.Pipes;
using System.Security.Cryptography;
using System.Text;
namespace Shanjie_UI.Setup;

internal enum SetupOutcome { Succeeded, Cancelled, Failed }
internal sealed record SetupResult(SetupOutcome Outcome, string Message);

/// <summary>
/// Runs shanjie-manager unelevated, as install.cmd does: it asks for UAC itself, and only its
/// unelevated half can start the user's core afterwards. The elevated half reports steps over
/// a pipe this window owns; the window never runs elevated.
/// </summary>
internal static class SetupRunner
{
    internal static string Manager => Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", "tools", "shanjie-manager.exe"));

    private static ProcessStartInfo Start(string arguments) => new(Manager, arguments)
    {
        UseShellExecute = false, CreateNoWindow = true,
        RedirectStandardOutput = true, RedirectStandardError = true,
        StandardOutputEncoding = Encoding.UTF8, StandardErrorEncoding = Encoding.UTF8,
    };

    internal static async Task<SetupStatus> Status()
    {
        using var process = Process.Start(Start("install-status")) ?? throw new InvalidOperationException("無法啟動安裝工具。");
        var output = process.StandardOutput.ReadToEndAsync();
        var error = process.StandardError.ReadToEndAsync();
        await process.WaitForExitAsync();
        if (process.ExitCode != 0) throw new InvalidDataException((await error).Trim());
        return SetupStatus.Parse(await output);
    }

    internal static Task<SetupResult> Install(uint keyboard, Action<SetupEvent> report) =>
        Run($"--elevate install --keyboard {(keyboard == 2 ? "eten" : "standard")}", report, true);
    internal static Task<SetupResult> Uninstall(bool deleteUserData, Action<SetupEvent> report) =>
        Run($"--elevate uninstall {(deleteUserData ? "--delete-user-data" : "--keep-user-data")}", report, false);

    private static async Task<SetupResult> Run(string arguments, Action<SetupEvent> report, bool runtime)
    {
        string name = "ShanjieSetup-" + RandomNumberGenerator.GetHexString(32, true);
        // One instance, so nothing else can take the name once the window owns it.
        using var pipe = new NamedPipeServerStream(name, PipeDirection.In, 1, PipeTransmissionMode.Byte, PipeOptions.Asynchronous);
        using var process = Process.Start(Start($"{arguments} --progress \\\\.\\pipe\\{name}")) ?? throw new InvalidOperationException("無法啟動安裝工具。");
        var output = process.StandardOutput.ReadToEndAsync();
        var error = process.StandardError.ReadToEndAsync();
        var exited = process.WaitForExitAsync();
        SetupEvent? result = null;
        using (var stop = new CancellationTokenSource())
        {
            // UAC declined or a failure before elevation: the manager exits without connecting.
            var connected = pipe.WaitForConnectionAsync(stop.Token);
            if (await Task.WhenAny(connected, exited) == connected && connected.IsCompletedSuccessfully)
            {
                using var reader = new StreamReader(pipe, Encoding.UTF8);
                while (await reader.ReadLineAsync() is { } line)
                {
                    if (SetupEvent.Parse(line) is not { } item) continue;
                    if (item.Stage == "Result") result = item;
                    report(item);
                }
            }
            else stop.Cancel();
            try { await connected; } catch (OperationCanceledException) { } catch (IOException) { }
        }
        if (runtime && result?.Status == "succeeded") report(new(SetupStages.Runtime, "started", "", null, null));
        await exited;
        string stdout = (await output).Trim(), stderr = (await error).Trim();
        if (result?.Status == "cancelled") return new(SetupOutcome.Cancelled, result.Message);
        if (process.ExitCode == 0 && result?.Status == "succeeded")
        {
            if (runtime) report(new(SetupStages.Runtime, "succeeded", stdout, null, null));
            // The core and shell restarts are best effort and print their own notes after the result.
            string notes = string.Join("\n", stdout.Split('\n').Select(l => l.Trim()).Where(l => l.Length > 0 && !result.Message.Contains(l)));
            return new(SetupOutcome.Succeeded, notes.Length == 0 ? result.Message : result.Message + "\n" + notes + (stderr.Length == 0 ? "" : "\n" + stderr));
        }
        // ERROR_CANCELLED from ShellExecuteEx: the user declined the UAC prompt.
        if (result is null && stderr.Contains("0x800704C7", StringComparison.OrdinalIgnoreCase))
            return new(SetupOutcome.Cancelled, "已取消系統管理員權限要求，沒有修改任何內容。");
        string message = result?.Status == "failed" ? result.Message : stderr.Length > 0 ? stderr : stdout;
        return new(SetupOutcome.Failed, message.Length > 0 ? message : $"安裝工具結束碼 {process.ExitCode}。");
    }
}
