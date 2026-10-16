using System.IO.Pipes;
using System.Security.Cryptography;
using System.Security.Principal;
using System.Security.AccessControl;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
namespace Shanjie_UI;

// Stable across version directories; verification and alternate profiles are isolated.
internal sealed class SettingsInstance : IDisposable
{
    private static void Trace(string text) { string? path=Environment.GetEnvironmentVariable("SHANJIE_INSTANCE_TRACE"); if(Environment.GetEnvironmentVariable("SHANJIE_TEST_PROFILE")=="1" && path is not null) File.AppendAllText(path,text+"\n"); }
    private readonly Mutex mutex;
    private readonly string pipeName;
    private readonly CancellationTokenSource stop = new();
    private bool owns;
    private bool disposed;
    internal int ActivationCount { get; private set; }
    private SettingsInstance(string scope)
    {
        pipeName = "Shanjie.Settings.v1." + scope;
        mutex = new Mutex(false, "Local\\" + pipeName);
    }
    private bool Acquire()
    {
        try { return owns = mutex.WaitOne(0); }
        catch (AbandonedMutexException) { return owns = true; }
    }
    public static async Task<SettingsInstance?> Open()
    {
        // 0.8.x has no forwarding protocol. Preserve its unsaved draft and bring
        // that window back until the user closes it; do not create a second one.
        if (Environment.GetEnvironmentVariable("SHANJIE_TEST_PROFILE") != "1")
        {
            string installed = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "Shanjie") + Path.DirectorySeparatorChar;
            foreach (var process in System.Diagnostics.Process.GetProcessesByName("Shanjie.UI"))
            {
                using (process) try
                {
                    if (process.Id == Environment.ProcessId || process.SessionId != System.Diagnostics.Process.GetCurrentProcess().SessionId) continue;
                    if(!OpenProcessToken(process.Handle,8,out var token)) continue;
                    using(token) using(var identity=new WindowsIdentity(token.DangerousGetHandle()))
                        if(identity.User!=WindowsIdentity.GetCurrent().User) continue;
                    string file = process.MainModule?.FileName ?? "";
                    if (!file.StartsWith(installed, StringComparison.OrdinalIgnoreCase)) continue;
                    var version = System.Diagnostics.FileVersionInfo.GetVersionInfo(file);
                    if (version.FileMajorPart != 0 || version.FileMinorPart >= 9) continue;
                    nint handle = process.MainWindowHandle;
                    if (handle == 0) throw new IOException("舊版設定正在啟動或關閉，請稍後重試。未儲存的修改仍保留。 ");
                    AllowSetForegroundWindow((uint)process.Id); if (IsIconic(handle)) ShowWindow(handle, 9); SetForegroundWindow(handle); return null;
                }
                catch (Exception e) when (e is System.ComponentModel.Win32Exception or InvalidOperationException) { }
            }
        }
        string scope = WindowsIdentity.GetCurrent().User!.Value + ":" + System.Diagnostics.Process.GetCurrentProcess().SessionId
            + ":" + Path.GetFullPath(Models.ProfileStore.Path).ToUpperInvariant();
        var instance = new SettingsInstance(Convert.ToHexString(SHA256.HashData(System.Text.Encoding.UTF8.GetBytes(scope))));
        Trace("START "+instance.pipeName);
        string request="SHOW " + Guid.NewGuid().ToString("D");
        var until=DateTime.UtcNow.AddSeconds(10);
        for (int attempt = 0; attempt < 40 && DateTime.UtcNow<until; attempt++)
        {
            if (instance.Acquire()) { Trace("OWNER "+instance.pipeName); return instance; }
            try
            {
                using var pipe = new NamedPipeClientStream(".", instance.pipeName, PipeDirection.InOut, PipeOptions.Asynchronous,TokenImpersonationLevel.Identification);
                using var timeout = new CancellationTokenSource(500);
                await pipe.ConnectAsync(timeout.Token);
                if (pipe.GetAccessControl().GetOwner(typeof(SecurityIdentifier)) != WindowsIdentity.GetCurrent().User)
                    throw new UnauthorizedAccessException("設定轉送端點的擁有者不符。");
                using var reader = new StreamReader(pipe, leaveOpen: true);
                using var writer = new StreamWriter(pipe, leaveOpen: true) { AutoFlush = true };
                string? owner = await reader.ReadLineAsync(timeout.Token);
                if (!uint.TryParse(owner, out uint pid)) throw new IOException("Invalid settings owner");
                AllowSetForegroundWindow(pid);
                await writer.WriteLineAsync(request.AsMemory(), timeout.Token);
                if (await reader.ReadLineAsync(timeout.Token) == "OK") { Trace("FORWARDED"); instance.Dispose(); return null; }
            }
            catch (Exception e) when (e is IOException or OperationCanceledException) { Trace("CLIENT "+e.GetType().Name+" "+e.Message); }
            await Task.Delay(50);
        }
        instance.Dispose();
        throw new IOException("既有設定視窗尚未回應，請稍後再試；尚未儲存的修改仍保留在原視窗。");
    }
    public void Listen(Window window)
    {
        var queue = DispatcherQueue.GetForCurrentThread();
        nint handle = WinRT.Interop.WindowNative.GetWindowHandle(window);
        _ = Task.Run(async () =>
        {
            var accepted=new HashSet<Guid>(); var order=new Queue<Guid>();
            while (!stop.IsCancellationRequested)
            {
                try
                {
                    using var pipe = CreatePipe();
                    Trace("LISTEN");
                    await pipe.WaitForConnectionAsync(stop.Token);
                    bool sameUser=false; string ownerUser=WindowsIdentity.GetCurrent().User!.Value;
                    pipe.RunAsClient(() => sameUser=WindowsIdentity.GetCurrent().User?.Value==ownerUser);
                    if(!sameUser) continue;
                    using var timeout = CancellationTokenSource.CreateLinkedTokenSource(stop.Token); timeout.CancelAfter(1500);
                    using var reader = new StreamReader(pipe, leaveOpen: true);
                    using var writer = new StreamWriter(pipe, leaveOpen: true) { AutoFlush = true };
                    await writer.WriteLineAsync(Environment.ProcessId.ToString().AsMemory(), timeout.Token);
                    string? request=await reader.ReadLineAsync(timeout.Token);
                    if (request is null || !request.StartsWith("SHOW ") || !Guid.TryParse(request[5..],out Guid id)) continue;
                    bool duplicate=accepted.Contains(id);
                    if (duplicate || queue.TryEnqueue(() => {
                        ActivationCount++;
                        Trace("ACTIVATE "+ActivationCount);
                        if (Environment.GetEnvironmentVariable("SHANJIE_TEST_PROFILE")=="1") return;
                        if (IsIconic(handle)) ShowWindow(handle, 9);
                        window.Activate(); SetForegroundWindow(handle);
                    })) {
                        if (!duplicate) { accepted.Add(id); order.Enqueue(id); if(order.Count>256) accepted.Remove(order.Dequeue()); }
                        await writer.WriteLineAsync("OK".AsMemory(), timeout.Token);
                    }
                }
                catch (Exception e) when (e is IOException or OperationCanceledException) { Trace("SERVER "+e.GetType().Name+" "+e.Message); }
            }
        });
        window.Closed += (_, _) => Dispose();
    }
    public void Dispose()
    {
        if (disposed) return; disposed = true;
        if (!stop.IsCancellationRequested) stop.Cancel();
        if (owns) { owns = false; mutex.ReleaseMutex(); }
        mutex.Dispose();
    }
    private NamedPipeServerStream CreatePipe()
    {
        string user=WindowsIdentity.GetCurrent().User!.Value;
        // Creation with a mandatory label needs no audit-SACL access. The managed
        // ACL factory requests ACCESS_SYSTEM_SECURITY for any SACL, which fails
        // for ordinary users. Match the native core's explicit creation contract.
        if(!ConvertStringSecurityDescriptorToSecurityDescriptor($"O:{user}D:P(A;;GA;;;{user})(A;;GRGW;;;AC)(A;;GRGW;;;S-1-15-2-2)S:(ML;;NW;;;LW)",1,out nint descriptor,out _))
            throw new IOException("Unable to create settings endpoint security",new System.ComponentModel.Win32Exception());
        try {
            var attributes=new SecurityAttributes { Length=Marshal.SizeOf<SecurityAttributes>(), Descriptor=descriptor };
            var handle=CreateNamedPipe("\\\\.\\pipe\\"+pipeName,3|0x40000000|0x80000,8,1,4096,4096,0,ref attributes);
            if(handle.IsInvalid) { var error=new System.ComponentModel.Win32Exception(); handle.Dispose(); throw new IOException("Unable to create settings endpoint",error); }
            return new NamedPipeServerStream(PipeDirection.InOut,true,false,handle);
        } finally { LocalFree(descriptor); }
    }
    [StructLayout(LayoutKind.Sequential)] private struct SecurityAttributes { public int Length; public nint Descriptor; public int Inherit; }
    [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] private static extern bool ConvertStringSecurityDescriptorToSecurityDescriptor(string value,uint revision,out nint descriptor,out uint size);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] private static extern SafePipeHandle CreateNamedPipe(string name,uint access,uint mode,uint instances,uint output,uint input,uint timeout,ref SecurityAttributes attributes);
    [DllImport("kernel32.dll")] private static extern nint LocalFree(nint memory);
    [DllImport("advapi32.dll",SetLastError=true)] private static extern bool OpenProcessToken(nint process,uint access,out SafeAccessTokenHandle token);
    [DllImport("user32.dll")] private static extern bool AllowSetForegroundWindow(uint pid);
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(nint hwnd);
    [DllImport("user32.dll")] private static extern bool IsIconic(nint hwnd);
    [DllImport("user32.dll")] private static extern bool ShowWindow(nint hwnd, int command);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] internal static extern int MessageBox(nint hwnd, string text, string caption, uint flags);
}
