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
    [ObservableProperty] public partial bool Associations { get; set; }
    [ObservableProperty] public partial bool Alphanumeric { get; set; }
    [ObservableProperty] public partial int Keyboard { get; set; }
    [ObservableProperty] public partial bool Big5 { get; set; }
    [ObservableProperty] public partial bool ReadingCandidate { get; set; }
    [ObservableProperty] public partial string DictionaryText { get; set; } = "";
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
        CapsLockEnglish = ProfileStore.Flag("input", "caps_lock_english", Interaction == 0); EscapeClears = ProfileStore.Flag("input", "escape_clears", true);
        SpacePages = ProfileStore.Flag("candidates", "space_pages", true); BackquoteSymbols = ProfileStore.Flag("shortcuts", "backquote_symbols", Interaction == 0);
        CodeInput = ProfileStore.Flag("shortcuts", "code_input", Interaction == 0); Theme = ProfileStore.Number("appearance", "theme", 0); FontSize = ProfileStore.Number("appearance", "font_size", 18);
        Learning = ProfileStore.Flag("input", "learning", false);
        Associations = ProfileStore.Flag("input", "associations", Interaction == 0);
        Alphanumeric = ProfileStore.Flag("input", "alphanumeric", Interaction == 0);
        Keyboard = ProfileStore.Number("input", "keyboard", 0);
        Big5 = ProfileStore.Flag("candidates", "big5", false);
        ReadingCandidate = ProfileStore.Flag("candidates", "reading", Interaction == 0);
        var dictionary = Path.Combine(Path.GetDirectoryName(ProfileStore.Path)!, ProfileStore.Read("input", "dictionary", "dictionary.tsv"));
        DictionaryText = File.Exists(dictionary) ? File.ReadAllText(dictionary) : "";
        Status = "";
    }
    public void Preset(bool windows)
    {
        Interaction = windows ? 0 : 1; Trigger = windows ? 0 : 2; Anchor = windows ? 0 : 1; Layout = windows ? 0 : 1;
        ShiftToggle = CtrlSpaceToggle = WidthToggle = CtrlPunctuation = windows; InitialChinese = true; InitialFullWidth = false; CapsLockEnglish = windows;
        EscapeClears = SpacePages = true; BackquoteSymbols = CodeInput = windows; ShiftSide = Theme = Ranking = 0; FontSize = 18; Learning = false;
        Associations = Alphanumeric = windows; Keyboard = 0; Big5 = false;
        ReadingCandidate = windows;
        Status = "已載入預設，按儲存後才會套用。";
    }
    public void Save()
    {
        static int Flag(bool value) => value ? 1 : 0;
        var rows = new List<string>();
        foreach (string line in DictionaryText.Split('\n').Select(row => row.Trim()).Where(row => row.Length > 0))
        {
            var fields = line.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
            if (fields.Length is not (2 or 3) || !System.Text.RegularExpressions.Regex.IsMatch(fields[0], "^[ㄅ-ㄩˊˇˋ˙]+(-[ㄅ-ㄩˊˇˋ˙]+)*$") || fields[0].Split('-').Length != fields[1].EnumerateRunes().Count())
                throw new InvalidDataException("個人詞庫格式有誤：每行填「注音 字詞」，每個字的注音以 - 分隔。");
            string score = fields.Length == 3 ? fields[2] : "-0.01";
            if (!double.TryParse(score, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double value) || !double.IsFinite(value) || value > 0)
                throw new InvalidDataException("詞庫分數必須是有限的非正數。");
            rows.Add($"{fields[0]}\t{fields[1]}\t{score}");
        }
        Directory.CreateDirectory(Path.GetDirectoryName(ProfileStore.Path)!);
        var dictionaryContent = string.Join("\n", rows) + (rows.Count > 0 ? "\n" : "");
        string dictionaryName = "dictionary-" + Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(System.Text.Encoding.UTF8.GetBytes(dictionaryContent))).ToLowerInvariant() + ".tsv";
        var dictionary = Path.Combine(Path.GetDirectoryName(ProfileStore.Path)!, dictionaryName);
        if (!File.Exists(dictionary)) { File.WriteAllText(dictionary + ".tmp", dictionaryContent, new System.Text.UTF8Encoding(false)); File.Move(dictionary + ".tmp", dictionary); }
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
            associations={Flag(Associations)}
            alphanumeric={Flag(Alphanumeric)}
            keyboard={Keyboard}
            dictionary={dictionaryName}
            [candidates]
            layout={new[] { "vertical", "horizontal", "grid" }[Layout]}
            anchor={(Anchor == 0 ? "right" : "left")}
            open={new[] { "down", "space", "both" }[Trigger]}
            space_pages={Flag(SpacePages)}
            big5={Flag(Big5)}
            reading={Flag(ReadingCandidate)}
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
