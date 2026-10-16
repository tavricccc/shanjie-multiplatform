using Shanjie_UI.Models;
namespace Shanjie_UI.ViewModels;
public partial class SettingsViewModel
{
    private ProfileDocument profile = new();
    private bool loading;
    private string dictionaryBaseline = "";
    public SettingsViewModel()
    {
        foreach (var key in ShiftKeys.Concat(PlainKeys)) key.PropertyChanged += (_, _) => { if (!loading) HasChanges = true; };
        Load(); PropertyChanged += (_, e) =>
        {
            if (!loading && e.PropertyName is not (nameof(Status) or nameof(HasChanges) or nameof(DraftHelp) or nameof(CandidateHelp) or nameof(ModelHelp))) HasChanges = true;
        };
    }
    public void Load() { profile = ProfileStore.Load(); ApplyDocument(true); HasChanges = false; Status = ""; ApplyInstalledKeyboards(Interop.Native.sj_installed_keyboards()); }
    internal void ApplyInstalledKeyboards(uint mask)
    {
        loading = true;
        try {
            int previousKeyboard = Keyboard;
            StandardInstalled = (mask & 1) != 0; EtenInstalled = (mask & 2) != 0;
            InstalledKeyboardHelp = mask switch { 1 => "本機只安裝標準注音。要改用倚天，請執行安裝包的「重新選擇鍵盤.cmd」。", 2 => "本機只安裝倚天注音。要改用標準注音，請執行安裝包的「重新選擇鍵盤.cmd」。", 0 => "鍵盤註冊狀態不一致，請重新執行安裝以修復。", _ => "本機提供標準注音與倚天注音。" };
            if (previousKeyboard == 1 && !StandardInstalled || previousKeyboard == 2 && !EtenInstalled) { Keyboard = 0; HasChanges = true; Status = "先前的鍵盤配置尚未安裝，已改為跟隨已安裝配置；按儲存後更新偏好。"; }
        }
        finally { loading = false; }
    }
    private void ApplyDocument(bool readDictionary)
    {
        loading = true;
        try
        {
            Interaction = profile.Read("profile", "interaction", "windows") == "windows" ? 0 : 1;
            Trigger = Array.IndexOf(new[] { "down", "space", "both" }, profile.Read("candidates", "open", "down"));
            Anchor = profile.Read("candidates", "anchor", "right") == "right" ? 0 : 1;
            CandidateLayout = profile.Read("candidates", "layout", "horizontal") == "vertical" ? 1 : 0;
            Ranking = profile.Read("engine", "ranking", "chat") == "chat" ? 0 : 1;
            ModelVariant=profile.Number("engine","class_table",5)==4?1:0;
            Prediction = profile.Flag("engine", "prediction", Interaction == 1); Demote = profile.Flag("engine", "demote", true);
            ShiftToggle = profile.Flag("shortcuts", "shift_toggle", Interaction == 0); ShiftSide = profile.Number("shortcuts", "shift_side", 0);
            CtrlSpaceToggle = profile.Flag("shortcuts", "ctrl_space_toggle", false); WidthToggle = profile.Flag("shortcuts", "shift_space_toggle", Interaction == 0);
            CtrlPunctuation = profile.Flag("shortcuts", "ctrl_punctuation", Interaction == 0);
            InitialChinese = profile.Flag("input", "chinese", true); InitialFullWidth = profile.Flag("input", "full_width", false);
            CapsLockEnglish = profile.Flag("input", "caps_lock_english", Interaction == 0);
            Escape = Array.IndexOf(new[] { "staged", "clear", "ignore" }, profile.Read("input", "escape", "staged"));
            SpaceAfterCommit = profile.Flag("input", "space_after_commit", true); PinyinSpaceConverts = profile.Flag("input", "pinyin_space_converts", Interaction == 0);
            SpaceCommits = profile.Flag("input", "space_commits", false); SpaceOperates = profile.Flag("candidates", "space_operates", false);
            SpacePages = profile.Flag("candidates", "space_pages", true); WrapPages = profile.Flag("candidates", "wrap_pages", true);
            BackquoteSymbols = profile.Flag("shortcuts", "backquote_symbols", Interaction == 0); CodeInput = profile.Flag("shortcuts", "code_input", Interaction == 0);
            SymbolKeyboard = profile.Flag("shortcuts", "symbol_keyboard", Interaction == 0); CtrlBackslash = profile.Flag("shortcuts", "ctrl_backslash", Interaction == 1);
            ForgetCandidate = profile.Flag("shortcuts", "forget_candidate", false);
            Theme = profile.Number("appearance", "theme", 0); FontSize = profile.Number("appearance", "font_size", 16); ModeIndicator = profile.Flag("appearance", "mode_indicator", true);
            Learning = profile.Flag("input", "learning", false); Associations = profile.Flag("input", "associations", false);
            Alphanumeric = profile.Flag("input", "alphanumeric", Interaction == 0); Keyboard = profile.Number("input", "keyboard", 0);
            MixedInput = profile.Flag("input", "mixed_input", false);
            MixedTolerance = profile.Flag("input", "mixed_tolerance", false);
            AddWord = profile.Flag("shortcuts", "add_word", true);
            AddWordSide = profile.Number("shortcuts", "add_word_side", 0);
            AddWordShortcut = profile.Number("shortcuts", "add_word_shortcut", 0);
            AddWordInterval = Math.Clamp(profile.Number("shortcuts", "add_word_interval", 450), 200, 1000);
            UseContext = profile.Flag("input", "context", true); Big5 = profile.Flag("candidates", "big5", false);
            foreach (var key in ShiftKeys.Concat(PlainKeys)) key.IsOn = profile.Flag("punctuation", key.Key, Interaction == 1);
            StandaloneTones = profile.Flag("input", "standalone_tones", Interaction == 1);
            SpaceInputsZhuyin = profile.Flag("input", "space_inputs_zhuyin", true);
            EnterCommits = profile.Flag("candidates", "enter_commits", Interaction == 0); UpCycles = profile.Flag("candidates", "up_cycles", Interaction == 0);
            BackspaceDeletes = profile.Flag("candidates", "backspace_deletes", false);
            if (readDictionary)
            {
                var dictionary = Path.Combine(Path.GetDirectoryName(ProfileStore.Path)!, profile.Read("input", "dictionary", "dictionary.tsv"));
                DictionaryText = File.Exists(dictionary) ? File.ReadAllText(dictionary) : "";
                dictionaryBaseline = DictionaryText;
            }
        }
        finally { loading = false; }
    }
    public void Preset(bool windows)
    {
        WriteDocument(); var preset = ProfileDocument.Parse(Interop.Native.Take(Interop.Native.sj_profile_preset(profile.ToString(), windows)));
        profile = preset; ApplyDocument(false); HasChanges = true;
        Status = "已載入操作預設，按儲存後套用。詞庫、學習與外觀已保留。";
        ApplyInstalledKeyboards(Interop.Native.sj_installed_keyboards());
    }
    public static string PresetFolder => Path.Combine(Path.GetDirectoryName(ProfileStore.Path)!, "presets");
    /// Saved presets shown under 一般, as (display name, file path).
    public static List<(string Name, string File)> SavedPresets()
    {
        var list = new List<(string, string)>();
        if (!Directory.Exists(PresetFolder)) return list;
        foreach (var file in Directory.GetFiles(PresetFolder, "*.ini").Order(StringComparer.OrdinalIgnoreCase))
        {
            try { list.Add((ProfileDocument.Parse(File.ReadAllText(file)).Read("preset", "name", Path.GetFileNameWithoutExtension(file)), file)); }
            catch (InvalidDataException) { } // A damaged file is skipped, not fatal for the settings page.
        }
        return list;
    }
    /// Validates someone else's preset, keeps a copy under presets\, and loads it as an unsaved draft.
    public string ImportPreset(string text, string fallbackName)
    {
        WriteDocument();
        var merged = Interop.Native.Take(Interop.Native.sj_profile_import(profile.ToString(), text));
        var name = ProfileDocument.Parse(merged).Read("preset", "name", fallbackName);
        var clean = Interop.Native.Take(Interop.Native.sj_profile_export(merged, name));
        Directory.CreateDirectory(PresetFolder);
        var safe = string.Concat(name.Select(c => Path.GetInvalidFileNameChars().Contains(c) ? '_' : c)).Trim();
        File.WriteAllText(Path.Combine(PresetFolder, (safe.Length == 0 ? "preset" : safe) + ".ini"), clean);
        LoadDraft(merged, $"已匯入「{name}」，按儲存後套用。詞庫、學習、鍵盤配置與外觀已保留。");
        return name;
    }
    public void ApplySavedPreset(string file, string name)
    {
        WriteDocument();
        LoadDraft(Interop.Native.Take(Interop.Native.sj_profile_import(profile.ToString(), File.ReadAllText(file))), $"已載入「{name}」，按儲存後套用。詞庫、學習與外觀已保留。");
    }
    /// The current draft's operation keys only; personal dictionary and appearance are never exported.
    public string ExportPreset(string name) { WriteDocument(); return Interop.Native.Take(Interop.Native.sj_profile_export(profile.ToString(), name)); }
    private void LoadDraft(string content, string status)
    {
        profile = ProfileDocument.Parse(content); ApplyDocument(false); HasChanges = true; Status = status;
        ApplyInstalledKeyboards(Interop.Native.sj_installed_keyboards());
    }
    private void WriteDocument()
    {
        profile.Set("profile", "schema", 6); profile.Set("profile", "interaction", Interaction == 0 ? "windows" : "shanjie");
        profile.Set("engine", "ranking", Ranking == 0 ? "chat" : "formal"); profile.Set("engine", "model", 5);
        foreach (var (section, key, value) in new (string, string, object)[] {
            ("engine","prediction",Prediction), ("engine","demote",Demote),
            ("engine","class_table",ModelVariant==1?4:5),
            ("input","chinese",InitialChinese), ("input","full_width",InitialFullWidth), ("input","caps_lock_english",CapsLockEnglish),
            ("input","space_after_commit",SpaceAfterCommit), ("input","pinyin_space_converts",PinyinSpaceConverts),
            ("input","space_commits",SpaceCommits), ("candidates","space_operates",SpaceOperates),
            ("input","mixed_input",MixedInput),
            ("input","mixed_tolerance",MixedTolerance),
            ("shortcuts","add_word",AddWord), ("shortcuts","add_word_side",AddWordSide),
            ("shortcuts","add_word_shortcut",AddWordShortcut), ("shortcuts","add_word_interval",(int)AddWordInterval),
            ("input","learning",Learning), ("input","associations",Associations), ("input","alphanumeric",Alphanumeric), ("input","keyboard",Keyboard), ("input","context",UseContext),
            ("input","standalone_tones",StandaloneTones),
            ("input","space_inputs_zhuyin",SpaceInputsZhuyin),
            ("candidates","space_pages",SpacePages), ("candidates","wrap_pages",WrapPages), ("candidates","big5",Big5), ("candidates","enter_commits",EnterCommits),
            ("candidates","up_cycles",UpCycles), ("candidates","backspace_deletes",BackspaceDeletes),
            ("shortcuts","shift_toggle",ShiftToggle), ("shortcuts","shift_side",ShiftSide), ("shortcuts","ctrl_space_toggle",CtrlSpaceToggle), ("shortcuts","shift_space_toggle",WidthToggle),
            ("shortcuts","ctrl_punctuation",CtrlPunctuation), ("shortcuts","backquote_symbols",BackquoteSymbols), ("shortcuts","code_input",CodeInput),
            ("shortcuts","symbol_keyboard",SymbolKeyboard), ("shortcuts","ctrl_backslash",CtrlBackslash), ("shortcuts","forget_candidate",ForgetCandidate),
            ("appearance","theme",Theme), ("appearance","font_size",(int)FontSize), ("appearance","mode_indicator",ModeIndicator)
        }) profile.Set(section, key, value);
        foreach (var key in ShiftKeys.Concat(PlainKeys)) profile.Set("punctuation", key.Key, key.IsOn);
        profile.Set("input", "escape", new[] { "staged", "clear", "ignore" }[Escape]);
        profile.Set("candidates", "layout", CandidateLayout == 1 ? "vertical" : "horizontal");
        profile.Set("candidates", "anchor", Anchor == 0 ? "right" : "left");
        profile.Set("candidates", "open", new[] { "down", "space", "both" }[Trigger]);
    }
    public void SetAllPunctuation(bool shift, bool on) { foreach (var key in shift ? ShiftKeys : PlainKeys) key.IsOn = on; }
    public void Save()
    {
        if (Keyboard is < 0 or > 4 || Trigger is < 0 or > 2 || Escape is < 0 or > 2 || !double.IsFinite(FontSize) || FontSize is < 14 or > 32)
            throw new InvalidDataException("請選擇有效的操作選項與 14–32 的候選文字大小。");
        if (Keyboard == 1 && !StandardInstalled || Keyboard == 2 && !EtenInstalled)
            throw new InvalidDataException("此鍵盤配置尚未安裝，請選擇可用配置。");
        if (ModelVariant is < 0 or > 1 || AddWordSide is < 0 or > 2 || AddWordShortcut is < 0 or > 1 || !double.IsFinite(AddWordInterval) || AddWordInterval is < 200 or > 1000)
            throw new InvalidDataException("請選擇有效的加詞快捷鍵與 200–1000 毫秒間隔。");
        WriteDocument();
        string dictionary = Interop.Native.Take(Interop.Native.sj_profile_save_dictionary(profile.ToString(), DictionaryText, dictionaryBaseline));
        profile.Set("input", "dictionary", dictionary);
        loading = true;
        try { DictionaryText = dictionaryBaseline = File.ReadAllText(Path.Combine(Path.GetDirectoryName(ProfileStore.Path)!, dictionary)); }
        finally { loading = false; }
        HasChanges = false;
        Status = "已儲存並通知善解立即套用。切換鍵盤或詞庫時會先提交目前組字。";
    }
}
