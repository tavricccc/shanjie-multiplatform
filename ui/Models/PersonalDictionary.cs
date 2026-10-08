using Shanjie_UI.Interop;
namespace Shanjie_UI.Models;
internal static class PersonalDictionary
{
    public static string Save(string text, string folder) => Native.Take(Native.sj_dictionary_save(text, folder));
}
