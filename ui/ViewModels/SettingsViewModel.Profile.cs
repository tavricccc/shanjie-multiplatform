using Shanjie_UI.Models;
namespace Shanjie_UI.ViewModels;
public partial class SettingsViewModel
{
    private ProfileDocument profile = new();
    private bool loading;
    public SettingsViewModel()
    {
        Load(); PropertyChanged += (_, e) =>
        {
            if (!loading && e.PropertyName is not (nameof(Status) or nameof(HasChanges) or nameof(DraftHelp) or nameof(CandidateHelp) or nameof(ModelHelp))) HasChanges = true;
        };
    }
    public void Load() { profile = ProfileStore.Load(); ApplyDocument(true); HasChanges = false; Status = ""; }
    private void ApplyDocument(bool readDictionary)
    {
        loading = true;
        try
        {
            Interaction = profile.Read("profile", "interaction", "windows") == "windows" ? 0 : 1;
            Trigger = Array.IndexOf(new[] { "down", "space", "both" }, profile.Read("candidates", "open", "down"));
            Anchor = profile.Read("candidates", "anchor", "right") == "right" ? 0 : 1;
            Ranking = profile.Read("engine", "ranking", "chat") == "chat" ? 0 : 1;
            Prediction = profile.Flag("engine", "prediction", Interaction == 1); Demote = profile.Flag("engine", "demote", true);
            ShiftToggle = profile.Flag("shortcuts", "shift_toggle", Interaction == 0); ShiftSide = profile.Number("shortcuts", "shift_side", 0);
            CtrlSpaceToggle = profile.Flag("shortcuts", "ctrl_space_toggle", false); WidthToggle = profile.Flag("shortcuts", "shift_space_toggle", Interaction == 0);
            CtrlPunctuation = profile.Flag("shortcuts", "ctrl_punctuation", Interaction == 0);
            InitialChinese = profile.Flag("input", "chinese", true); InitialFullWidth = profile.Flag("input", "full_width", false);
            CapsLockEnglish = profile.Flag("input", "caps_lock_english", Interaction == 0);
            Escape = Array.IndexOf(new[] { "staged", "clear", "ignore" }, profile.Read("input", "escape", "staged"));
            SpaceAfterCommit = profile.Flag("input", "space_after_commit", true); PinyinSpaceConverts = profile.Flag("input", "pinyin_space_converts", Interaction == 0);
            SpacePages = profile.Flag("candidates", "space_pages", true); WrapPages = profile.Flag("candidates", "wrap_pages", true);
            BackquoteSymbols = profile.Flag("shortcuts", "backquote_symbols", Interaction == 0); CodeInput = profile.Flag("shortcuts", "code_input", Interaction == 0);
            SymbolKeyboard = profile.Flag("shortcuts", "symbol_keyboard", Interaction == 0); CtrlBackslash = profile.Flag("shortcuts", "ctrl_backslash", Interaction == 1);
            ForgetCandidate = profile.Flag("shortcuts", "forget_candidate", false);
            Theme = profile.Number("appearance", "theme", 0); FontSize = profile.Number("appearance", "font_size", 16); ModeIndicator = profile.Flag("appearance", "mode_indicator", true);
            Learning = profile.Flag("input", "learning", false); Associations = profile.Flag("input", "associations", false);
            Alphanumeric = profile.Flag("input", "alphanumeric", Interaction == 0); Keyboard = profile.Number("input", "keyboard", 0);
            MixedInput = profile.Flag("input", "mixed_input", false);
            UseContext = profile.Flag("input", "context", true); Big5 = profile.Flag("candidates", "big5", false);
            ShiftPunctuation = profile.Flag("input", "shift_punctuation", Interaction == 1); PlainPunctuation = profile.Flag("input", "plain_punctuation", Interaction == 1);
            StandaloneTones = profile.Flag("input", "standalone_tones", Interaction == 1);
            EnterCommits = profile.Flag("candidates", "enter_commits", Interaction == 0); UpCycles = profile.Flag("candidates", "up_cycles", Interaction == 0);
            BackspaceDeletes = profile.Flag("candidates", "backspace_deletes", false);
            if (readDictionary)
            {
                var dictionary = Path.Combine(Path.GetDirectoryName(ProfileStore.Path)!, profile.Read("input", "dictionary", "dictionary.tsv"));
                DictionaryText = File.Exists(dictionary) ? File.ReadAllText(dictionary) : "";
            }
        }
        finally { loading = false; }
    }
    public void Preset(bool windows)
    {
        WriteDocument(); var preset = ProfileDocument.Parse(Interop.Native.Take(Interop.Native.sj_profile_preset(profile.ToString(), windows)));
        profile = preset; ApplyDocument(false); HasChanges = true;
        Status = "已載入操作預設，按儲存後套用。詞庫、學習與外觀已保留。";
    }
    private void WriteDocument()
    {
        profile.Set("profile", "schema", 5); profile.Set("profile", "interaction", Interaction == 0 ? "windows" : "shanjie");
        profile.Set("engine", "ranking", Ranking == 0 ? "chat" : "formal"); profile.Set("engine", "model", 5);
        foreach (var (section, key, value) in new (string, string, object)[] {
            ("engine","prediction",Prediction), ("engine","demote",Demote),
            ("input","chinese",InitialChinese), ("input","full_width",InitialFullWidth), ("input","caps_lock_english",CapsLockEnglish),
            ("input","space_after_commit",SpaceAfterCommit), ("input","pinyin_space_converts",PinyinSpaceConverts),
            ("input","mixed_input",MixedInput),
            ("input","learning",Learning), ("input","associations",Associations), ("input","alphanumeric",Alphanumeric), ("input","keyboard",Keyboard), ("input","context",UseContext),
            ("input","shift_punctuation",ShiftPunctuation), ("input","plain_punctuation",PlainPunctuation), ("input","standalone_tones",StandaloneTones),
            ("candidates","space_pages",SpacePages), ("candidates","wrap_pages",WrapPages), ("candidates","big5",Big5), ("candidates","enter_commits",EnterCommits),
            ("candidates","up_cycles",UpCycles), ("candidates","backspace_deletes",BackspaceDeletes),
            ("shortcuts","shift_toggle",ShiftToggle), ("shortcuts","shift_side",ShiftSide), ("shortcuts","ctrl_space_toggle",CtrlSpaceToggle), ("shortcuts","shift_space_toggle",WidthToggle),
            ("shortcuts","ctrl_punctuation",CtrlPunctuation), ("shortcuts","backquote_symbols",BackquoteSymbols), ("shortcuts","code_input",CodeInput),
            ("shortcuts","symbol_keyboard",SymbolKeyboard), ("shortcuts","ctrl_backslash",CtrlBackslash), ("shortcuts","forget_candidate",ForgetCandidate),
            ("appearance","theme",Theme), ("appearance","font_size",(int)FontSize), ("appearance","mode_indicator",ModeIndicator)
        }) profile.Set(section, key, value);
        profile.Set("input", "escape", new[] { "staged", "clear", "ignore" }[Escape]);
        profile.Set("candidates", "layout", "horizontal");
        profile.Set("candidates", "anchor", Anchor == 0 ? "right" : "left");
        profile.Set("candidates", "open", new[] { "down", "space", "both" }[Trigger]);
    }
    public void Save()
    {
        if (Trigger is < 0 or > 2 || Escape is < 0 or > 2 || !double.IsFinite(FontSize) || FontSize is < 14 or > 32)
            throw new InvalidDataException("請選擇有效的操作選項與 14–32 的候選文字大小。");
        string dictionary = PersonalDictionary.Save(DictionaryText, Path.GetDirectoryName(ProfileStore.Path)!);
        WriteDocument(); profile.Set("input", "dictionary", dictionary); ProfileStore.Save(profile.ToString()); HasChanges = false;
        Status = "已儲存並通知善解立即套用。切換鍵盤或詞庫時會先提交目前組字。";
    }
}
