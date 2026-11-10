using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
namespace Shanjie_UI;

/// A loose executable launched by a packaged host must not inherit that host's resource identity.
internal static class DesktopActivation
{
    private const string Relaunch = "SHANJIE_UI_DESKTOP_RELAUNCH";
    [StructLayout(LayoutKind.Sequential)] private struct Startup
    {
        internal uint Size;
        internal nint Reserved, Desktop, Title;
        internal uint X, Y, Width, Height, CharsX, CharsY, Fill, Flags;
        internal ushort Show, ReservedSize;
        internal nint ReservedData, Input, Output, Error;
    }
    [StructLayout(LayoutKind.Sequential)] private struct StartupEx { internal Startup Info; internal nint Attributes; }
    [StructLayout(LayoutKind.Sequential)] private struct Child { internal nint Process, Thread; internal uint Pid, Tid; }
    [DllImport("kernel32.dll")] private static extern int GetCurrentPackageFullName(ref uint length, nint name);
    [DllImport("kernel32.dll", SetLastError=true)] private static extern bool InitializeProcThreadAttributeList(nint list, int count, int flags, ref nint size);
    [DllImport("kernel32.dll", SetLastError=true)] private static extern bool UpdateProcThreadAttribute(nint list, uint flags, nint attribute, nint value, nint size, nint previous, nint returned);
    [DllImport("kernel32.dll")] private static extern void DeleteProcThreadAttributeList(nint list);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    private static extern bool CreateProcessW(string application, StringBuilder command, nint processSecurity, nint threadSecurity,
        bool inherit, uint flags, nint environment, string directory, ref StartupEx startup, out Child child);
    [DllImport("kernel32.dll")] private static extern bool CloseHandle(nint handle);
    [DllImport("user32.dll")] private static extern bool AllowSetForegroundWindow(uint process);
    [DllImport("kernel32.dll", SetLastError=true)] private static extern uint WaitForSingleObject(nint handle, uint timeout);
    [DllImport("kernel32.dll", SetLastError=true)] private static extern bool GetExitCodeProcess(nint process, out uint code);

    internal static bool RelaunchIfPackaged(out int exitCode)
    {
        exitCode = 0;
        uint length = 0;
        int status = GetCurrentPackageFullName(ref length, 0);
        if (status == 15700) return false; // APPMODEL_ERROR_NO_PACKAGE: normal desktop startup.
        if (status != 122) throw new Win32Exception(status);
        if (Environment.GetEnvironmentVariable(Relaunch) == "1")
            throw new InvalidOperationException("設定程式無法離開宿主的套件資源環境；已停止重複啟動。");
        nint size = 0;
        InitializeProcThreadAttributeList(0, 1, 0, ref size);
        nint list = Marshal.AllocHGlobal(size), policy = Marshal.AllocHGlobal(4);
        bool initialized = false;
        string? saved = Environment.GetEnvironmentVariable(Relaunch);
        try
        {
            if (!InitializeProcThreadAttributeList(list, 1, 0, ref size)) throw new Win32Exception();
            initialized = true;
            Marshal.WriteInt32(policy, 1); // PROCESS_CREATION_DESKTOP_APP_BREAKAWAY_ENABLE_PROCESS_TREE
            if (!UpdateProcThreadAttribute(list, 0, 0x20012, policy, 4, 0, 0)) throw new Win32Exception();
            var startup = new StartupEx { Info = new Startup { Size = (uint)Marshal.SizeOf<StartupEx>() }, Attributes = list };
            Environment.SetEnvironmentVariable(Relaunch, "1");
            if (!CreateProcessW(Environment.ProcessPath!, new StringBuilder(Environment.CommandLine), 0, 0, false,
                    0x80000, 0, Environment.CurrentDirectory, ref startup, out Child child)) throw new Win32Exception();
            CloseHandle(child.Thread);
            AllowSetForegroundWindow(child.Pid);
            try
            {
                // Preserve command-line callers' wait and exit-code semantics, including setup and verification.
                if (WaitForSingleObject(child.Process, uint.MaxValue) != 0 || !GetExitCodeProcess(child.Process, out uint code))
                    throw new Win32Exception();
                exitCode = unchecked((int)code);
                return true;
            }
            finally { CloseHandle(child.Process); }
        }
        finally
        {
            Environment.SetEnvironmentVariable(Relaunch, saved);
            if (initialized) DeleteProcThreadAttributeList(list);
            Marshal.FreeHGlobal(list); Marshal.FreeHGlobal(policy);
        }
    }
}
