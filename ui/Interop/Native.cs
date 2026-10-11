using System.Runtime.InteropServices;
namespace Shanjie_UI.Interop;
internal static class Native
{
    [DllImport("user32.dll")] internal static extern uint GetDpiForWindow(nint window);
    private const string Rust = "shanjie_windows.dll";
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_profile_load_text();
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] [return: MarshalAs(UnmanagedType.I1)] internal static extern bool sj_profile_save([MarshalAs(UnmanagedType.LPUTF8Str)] string content);
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_document_json([MarshalAs(UnmanagedType.LPUTF8Str)] string content);
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_profile_preset([MarshalAs(UnmanagedType.LPUTF8Str)] string content, [MarshalAs(UnmanagedType.I1)] bool windows);
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_dictionary_save([MarshalAs(UnmanagedType.LPUTF8Str)] string content, [MarshalAs(UnmanagedType.LPUTF8Str)] string folder);
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] [return: MarshalAs(UnmanagedType.I1)] internal static extern bool sj_clear_learning();
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] [return: MarshalAs(UnmanagedType.I1)] internal static extern bool sj_restart();
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_error();
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern void sj_string_free(nint value);
    internal static string Take(nint value) { if (value == 0) throw new InvalidDataException(Marshal.PtrToStringUTF8(sj_error())); try { return Marshal.PtrToStringUTF8(value)!; } finally { sj_string_free(value); } }
    internal static void Check(bool result) { if (!result) throw new InvalidDataException(Marshal.PtrToStringUTF8(sj_error())); }
}
