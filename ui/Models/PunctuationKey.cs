using CommunityToolkit.Mvvm.ComponentModel;
namespace Shanjie_UI.Models;
/// One `[punctuation] <Key>` switch; Key matches the Rust SHIFT_KEYS/PLAIN_KEYS names.
public sealed partial class PunctuationKey(string key, string label, string? mcbopomofoLabel = null) : ObservableObject
{
    private readonly string appleLabel = label;
    public string Key => key;
    [ObservableProperty] public partial string Label { get; set; } = label;
    [ObservableProperty] public partial bool IsOn { get; set; }
    [ObservableProperty] public partial bool Available { get; set; } = true;
    internal void SetLayout(bool mcbopomofo) => Label = mcbopomofo ? mcbopomofoLabel ?? appleLabel : appleLabel;
    public static PunctuationKey[] ShiftRow() => [
        new("shift_comma", "，  Shift＋,"), new("shift_period", "。  Shift＋."), new("shift_slash", "？  Shift＋/"),
        new("shift_1", "！  Shift＋1"), new("shift_semicolon", "：  Shift＋;"), new("shift_lbracket", "『  Shift＋["),
        new("shift_rbracket", "』  Shift＋]"), new("shift_9", "（  Shift＋9"), new("shift_0", "）  Shift＋0"),
        new("shift_backquote", "～  Shift＋`"), new("shift_backslash", "｜  Shift＋\\"), new("shift_quote", "“  Shift＋'", "；  Shift＋'"),
        new("shift_equal", "＋  Shift＋="),
    ];
    // Bit order matches the upstream half-width keycap mask.
    public static PunctuationKey[] HalfWidthRow() => [
        new("half_0", "~  Shift＋`"), new("half_1", "!  Shift＋1"), new("half_2", "@  Shift＋2"),
        new("half_3", "#  Shift＋3"), new("half_4", "$  Shift＋4"), new("half_5", "%  Shift＋5"),
        new("half_6", "^  Shift＋6"), new("half_7", "&  Shift＋7"), new("half_8", "*  Shift＋8"),
        new("half_9", "(  Shift＋9"), new("half_10", ")  Shift＋0"), new("half_11", "_  Shift＋-"),
        new("half_12", "+  Shift＋="), new("half_13", "{  Shift＋["), new("half_14", "}  Shift＋]"),
        new("half_15", "|  Shift＋\\"), new("half_16", ":  Shift＋;"), new("half_17", "\"  Shift＋'"),
        new("half_18", "<  Shift＋,"), new("half_19", ">  Shift＋."), new("half_20", "?  Shift＋/"),
        new("half_21", "`"), new("half_22", "="), new("half_23", "["),
        new("half_24", "]"), new("half_25", "\\"), new("half_26", "'"),
    ];
    public static PunctuationKey[] PlainRow() => [
        new("plain_lbracket", "「  ["), new("plain_rbracket", "」  ]"), new("plain_backslash", "、  \\", "＼  \\"),
        new("plain_quote", "‘  '", "、  '"), new("plain_equal", "＝  ="), new("plain_backquote", "·  `"),
    ];
}
