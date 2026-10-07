using System.Runtime.InteropServices;
namespace Shanjie_UI.Interop;
internal static class Native
{
    [DllImport("user32.dll")] internal static extern uint GetDpiForWindow(nint window);
}
