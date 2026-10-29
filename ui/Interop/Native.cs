using System.Runtime.InteropServices;
namespace Shanjie_UI.Interop;
internal static class Native
{
    [DllImport("user32.dll")] internal static extern uint GetDpiForWindow(nint window);
    private const string Rust = "shanjie_windows.dll";
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_profile_load_text();
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_profile_path();
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] [return: MarshalAs(UnmanagedType.I1)] internal static extern bool sj_profile_save([MarshalAs(UnmanagedType.LPUTF8Str)] string content);
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_document_json([MarshalAs(UnmanagedType.LPUTF8Str)] string content);
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_profile_preset([MarshalAs(UnmanagedType.LPUTF8Str)] string content, [MarshalAs(UnmanagedType.I1)] bool windows);
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_profile_import([MarshalAs(UnmanagedType.LPUTF8Str)] string content, [MarshalAs(UnmanagedType.LPUTF8Str)] string preset);
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_profile_export([MarshalAs(UnmanagedType.LPUTF8Str)] string content, [MarshalAs(UnmanagedType.LPUTF8Str)] string name);
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_dictionary_save([MarshalAs(UnmanagedType.LPUTF8Str)] string content, [MarshalAs(UnmanagedType.LPUTF8Str)] string folder);
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_profile_save_dictionary([MarshalAs(UnmanagedType.LPUTF8Str)] string profile, [MarshalAs(UnmanagedType.LPUTF8Str)] string draft, [MarshalAs(UnmanagedType.LPUTF8Str)] string baseline);
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] [return: MarshalAs(UnmanagedType.I1)] internal static extern bool sj_clear_learning();
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern uint sj_installed_keyboards();
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] [return: MarshalAs(UnmanagedType.I1)] internal static extern bool sj_restart();
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern nint sj_error();
    [DllImport(Rust, CallingConvention = CallingConvention.Cdecl)] internal static extern void sj_string_free(nint value);
    [DllImport(Rust, CallingConvention=CallingConvention.Cdecl)] internal static extern nint sj_models_scan();
    [DllImport(Rust, CallingConvention=CallingConvention.Cdecl)] internal static extern nint sj_catalog_validate([MarshalAs(UnmanagedType.LPUTF8Str)] string content);
    [DllImport(Rust, CallingConvention=CallingConvention.Cdecl)] internal static extern nint sj_models_errors();
    [DllImport(Rust, CallingConvention=CallingConvention.Cdecl)] internal static extern nint sj_model_prepare([MarshalAs(UnmanagedType.LPUTF8Str)] string staging);
    internal static string Take(nint value) { if (value == 0) throw new InvalidDataException(Marshal.PtrToStringUTF8(sj_error())); try { return Marshal.PtrToStringUTF8(value)!; } finally { sj_string_free(value); } }
    internal static void Check(bool result) { if (!result) throw new InvalidDataException(Marshal.PtrToStringUTF8(sj_error())); }
}
