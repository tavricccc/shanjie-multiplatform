using CommunityToolkit.Mvvm.ComponentModel;
using Shanjie_UI.Models;
namespace Shanjie_UI.ViewModels;
public partial class SettingsViewModel : ObservableObject
{
    [ObservableProperty] public partial int Interaction { get; set; }
    [ObservableProperty] public partial int Trigger { get; set; }
    [ObservableProperty] public partial int Anchor { get; set; }
    [ObservableProperty] public partial int Layout { get; set; }
    [ObservableProperty] public partial int Ranking { get; set; }
    [ObservableProperty] public partial bool ShiftToggle { get; set; }
    [ObservableProperty] public partial int ShiftSide { get; set; }
    [ObservableProperty] public partial bool CtrlSpaceToggle { get; set; }
    [ObservableProperty] public partial bool WidthToggle { get; set; }
    [ObservableProperty] public partial bool CtrlPunctuation { get; set; }
    [ObservableProperty] public partial bool InitialChinese { get; set; }
    [ObservableProperty] public partial bool InitialFullWidth { get; set; }
    [ObservableProperty] public partial bool CapsLockEnglish { get; set; }
    [ObservableProperty] public partial bool EscapeClears { get; set; }
    [ObservableProperty] public partial bool SpacePages { get; set; }
    [ObservableProperty] public partial bool BackquoteSymbols { get; set; }
    [ObservableProperty] public partial bool CodeInput { get; set; }
    [ObservableProperty] public partial bool Learning { get; set; }
    [ObservableProperty] public partial int Theme { get; set; }
    [ObservableProperty] public partial double FontSize { get; set; }
    [ObservableProperty] public partial string Status { get; set; } = "";
    public SettingsViewModel() => Load();
    public void Load()
    {
        Interaction = ProfileStore.Read("profile", "interaction", "windows") == "windows" ? 0 : 1;
        Trigger = Array.IndexOf(new[] { "down", "space", "both" }, ProfileStore.Read("candidates", "open", "down"));
        Anchor = ProfileStore.Read("candidates", "anchor", "right") == "right" ? 0 : 1;
        Layout = Array.IndexOf(new[] { "vertical", "horizontal", "grid" }, ProfileStore.Read("candidates", "layout", "vertical"));
        Ranking = ProfileStore.Read("engine", "ranking", "chat") == "chat" ? 0 : 1;
        ShiftToggle = ProfileStore.Flag("shortcuts", "shift_toggle", true); ShiftSide = ProfileStore.Number("shortcuts", "shift_side", 0);
        CtrlSpaceToggle = ProfileStore.Flag("shortcuts", "ctrl_space_toggle", true); WidthToggle = ProfileStore.Flag("shortcuts", "shift_space_toggle", true);
        CtrlPunctuation = ProfileStore.Flag("shortcuts", "ctrl_punctuation", true);
        InitialChinese = ProfileStore.Flag("input", "chinese", true); InitialFullWidth = ProfileStore.Flag("input", "full_width", false);
        CapsLockEnglish = ProfileStore.Flag("input", "caps_lock_english", true); EscapeClears = ProfileStore.Flag("input", "escape_clears", true);
        SpacePages = ProfileStore.Flag("candidates", "space_pages", true); BackquoteSymbols = ProfileStore.Flag("shortcuts", "backquote_symbols", true);
        CodeInput = ProfileStore.Flag("shortcuts", "code_input", true); Theme = ProfileStore.Number("appearance", "theme", 0); FontSize = ProfileStore.Number("appearance", "font_size", 18);
        Learning = ProfileStore.Flag("input", "learning", false);
        Status = "";
    }
    public void Preset(bool windows)
    {
        Interaction = windows ? 0 : 1; Trigger = windows ? 0 : 2; Anchor = windows ? 0 : 1; Layout = windows ? 0 : 1;
        ShiftToggle = CtrlSpaceToggle = WidthToggle = CtrlPunctuation = windows; InitialChinese = true; InitialFullWidth = false; CapsLockEnglish = windows;
        EscapeClears = SpacePages = true; BackquoteSymbols = CodeInput = windows; ShiftSide = Theme = Ranking = 0; FontSize = 18; Learning = false;
        Status = "已載入預設，按儲存後才會套用。";
    }
    public void Save()
    {
        static int Flag(bool value) => value ? 1 : 0;
        ProfileStore.Save($"""
            [profile]
            schema=2
            interaction={(Interaction == 0 ? "windows" : "shanjie")}
            [engine]
            ranking={(Ranking == 0 ? "chat" : "formal")}
            [input]
            chinese={Flag(InitialChinese)}
            full_width={Flag(InitialFullWidth)}
            caps_lock_english={Flag(CapsLockEnglish)}
            escape_clears={Flag(EscapeClears)}
            learning={Flag(Learning)}
            [candidates]
            layout={new[] { "vertical", "horizontal", "grid" }[Layout]}
            anchor={(Anchor == 0 ? "right" : "left")}
            open={new[] { "down", "space", "both" }[Trigger]}
            space_pages={Flag(SpacePages)}
            [shortcuts]
            shift_toggle={Flag(ShiftToggle)}
            shift_side={ShiftSide}
            ctrl_space_toggle={Flag(CtrlSpaceToggle)}
            shift_space_toggle={Flag(WidthToggle)}
            ctrl_punctuation={Flag(CtrlPunctuation)}
            backquote_symbols={Flag(BackquoteSymbols)}
            code_input={Flag(CodeInput)}
            [appearance]
            theme={Theme}
            font_size={(int)FontSize}
            """);
        Status = "已儲存。請切換到其他輸入法再切回善解；仍使用舊設定的程式需重新開啟。";
    }
}
