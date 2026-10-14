using CommunityToolkit.Mvvm.ComponentModel;
namespace Shanjie_UI.Models;
/// One `[punctuation] <Key>` switch; Key matches the Rust SHIFT_KEYS/PLAIN_KEYS names.
public sealed partial class PunctuationKey(string key, string label) : ObservableObject
{
    public string Key => key;
    public string Label => label;
    [ObservableProperty] public partial bool IsOn { get; set; }
    public static PunctuationKey[] ShiftRow() => [
        new("shift_comma", "，  Shift＋,"), new("shift_period", "。  Shift＋."), new("shift_slash", "？  Shift＋/"),
        new("shift_1", "！  Shift＋1"), new("shift_semicolon", "：  Shift＋;"), new("shift_lbracket", "『  Shift＋["),
        new("shift_rbracket", "』  Shift＋]"), new("shift_9", "（  Shift＋9"), new("shift_0", "）  Shift＋0"),
        new("shift_backquote", "～  Shift＋`"), new("shift_backslash", "｜  Shift＋\\"), new("shift_quote", "“  Shift＋'"),
        new("shift_equal", "＋  Shift＋="),
    ];
    public static PunctuationKey[] PlainRow() => [
        new("plain_lbracket", "「  ["), new("plain_rbracket", "」  ]"), new("plain_backslash", "、  \\"),
        new("plain_quote", "‘  '"), new("plain_equal", "＝  ="), new("plain_backquote", "·  `"),
    ];
}
