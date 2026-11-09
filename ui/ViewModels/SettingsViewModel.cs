using CommunityToolkit.Mvvm.ComponentModel;
using Shanjie_UI.Models;
namespace Shanjie_UI.ViewModels;
public partial class SettingsViewModel : ObservableObject
{
    [ObservableProperty, NotifyPropertyChangedFor(nameof(CandidateHelp)), NotifyPropertyChangedFor(nameof(RightActionLabel))] public partial int Interaction { get; set; }
    public string RightActionLabel => Interaction == 0 ? "Tab 與 → 右方向鍵" : "→ 右方向鍵";
    [ObservableProperty] public partial int Trigger { get; set; }
    [ObservableProperty] public partial int Anchor { get; set; }
    [ObservableProperty, NotifyPropertyChangedFor(nameof(RankingByApp))] public partial int Ranking { get; set; }
    public bool RankingByApp => Ranking == 0;
    /// `[engine] chat_apps`, one executable name per line.
    [ObservableProperty] public partial string ChatApps { get; set; } = "";
    public int Model => 5;
    [ObservableProperty, NotifyPropertyChangedFor(nameof(ModelHelp))] public partial int ModelVariant { get; set; }
    [ObservableProperty] public partial bool ShiftToggle { get; set; }
    [ObservableProperty] public partial int ShiftSide { get; set; }
    [ObservableProperty] public partial bool CtrlSpaceToggle { get; set; }
    [ObservableProperty] public partial bool WidthToggle { get; set; }
    [ObservableProperty] public partial bool CtrlPunctuation { get; set; }
    [ObservableProperty] public partial bool InitialChinese { get; set; }
    [ObservableProperty] public partial bool InitialFullWidth { get; set; }
    [ObservableProperty] public partial bool CapsLockEnglish { get; set; }
    [ObservableProperty] public partial int Escape { get; set; }
    [ObservableProperty] public partial bool SpaceAfterCommit { get; set; }
    [ObservableProperty] public partial bool SpaceCommits { get; set; }
    [ObservableProperty, NotifyPropertyChangedFor(nameof(CandidateHelp)), NotifyPropertyChangedFor(nameof(CanWrapSpacePages))] public partial bool SpaceOperates { get; set; }
    [ObservableProperty] public partial bool PinyinSpaceConverts { get; set; }
    [ObservableProperty] public partial bool SpaceInputsZhuyin { get; set; }
    [ObservableProperty] public partial bool ZhuyinOnly { get; set; }
    [ObservableProperty, NotifyPropertyChangedFor(nameof(CanWrapSpacePages))] public partial bool SpacePages { get; set; }
    public bool CanWrapSpacePages => (SpaceOperates && SpacePages) || RightAction == 1;
    [ObservableProperty] public partial bool WrapPages { get; set; }
    [ObservableProperty, NotifyPropertyChangedFor(nameof(CandidateHelp)), NotifyPropertyChangedFor(nameof(CanWrapSpacePages))] public partial int RightAction { get; set; }
    [ObservableProperty] public partial bool BackquoteSymbols { get; set; }
    [ObservableProperty] public partial bool CodeInput { get; set; }
    [ObservableProperty] public partial bool Learning { get; set; }
    [ObservableProperty] public partial bool Associations { get; set; }
    [ObservableProperty] public partial bool Prediction { get; set; }
    [ObservableProperty, NotifyPropertyChangedFor(nameof(RerankLevelEnabled))] public partial bool Demote { get; set; }
    [ObservableProperty] public partial bool AcgPack { get; set; }
    [ObservableProperty] public partial bool QuotePack { get; set; }
    [ObservableProperty] public partial bool ItPack { get; set; }
    /// Upstream makes no rerank job while 避免敏感詞 is off, so both rerank choices grey with it.
    [ObservableProperty, NotifyPropertyChangedFor(nameof(RerankLevelEnabled))] public partial bool Rerank { get; set; }
    [ObservableProperty] public partial int RerankLevel { get; set; } = 1;
    public bool RerankLevelEnabled => Demote && Rerank;
    [ObservableProperty] public partial bool FuzzyEnEng { get; set; }
    [ObservableProperty] public partial bool FuzzyZhZ { get; set; }
    [ObservableProperty] public partial bool FuzzyChC { get; set; }
    [ObservableProperty] public partial bool FuzzyShS { get; set; }
    [ObservableProperty] public partial bool FuzzyAdjacent { get; set; }
    /// Adjacent keys only exist for the standard layout (upstream fuzzy-input section 2.2).
    public bool SupportsFuzzyAdjacent => Keyboard is 0 or 1;
    [ObservableProperty] public partial bool EnglishKeyOrder { get; set; }
    [ObservableProperty] public partial bool PunctuationKeepsReading { get; set; }
    [ObservableProperty] public partial bool CjkSpacing { get; set; }
    [ObservableProperty] public partial bool UseContext { get; set; }
    [ObservableProperty] public partial bool Alphanumeric { get; set; }
    [ObservableProperty] public partial bool MixedTolerance { get; set; }
    [ObservableProperty] public partial bool AddWord { get; set; }
    [ObservableProperty] public partial int AddWordSide { get; set; }
    [ObservableProperty] public partial int AddWordShortcut { get; set; }
    [ObservableProperty] public partial double AddWordInterval { get; set; } = 450;
    [ObservableProperty] public partial int MixedMode { get; set; }
    [ObservableProperty] public partial bool EnglishOverwrite { get; set; }
    [ObservableProperty] public partial bool EnglishReversalGuard { get; set; }
    [ObservableProperty, NotifyPropertyChangedFor(nameof(CandidateHelp))] public partial bool EnterToCursor { get; set; }
    public bool SupportsUpstreamEnglish => Keyboard<2 && IsZhuyin;
    public bool UsesUpstreamEnglish => SupportsUpstreamEnglish && MixedMode == 2;
    partial void OnMixedModeChanged(int value) { OnPropertyChanged(nameof(UsesUpstreamEnglish)); NotifyReadingPanel(); }
    /// Display name of the mode stored as `[input] mixed_input`; change it here and every settings text follows.
    public const string RawKeyMixingName = "原鍵混打";
    public string RawKeyMixingLabel => RawKeyMixingName;
    public string MixedModeNote => $"{RawKeyMixingName}：按鍵排不成合法注音就當英文，組字區先顯示英文按鍵並提示注音，輸入聲調後轉成中文，Enter 保留英文；一定會開啟組字面板。上游判斷：注音不可能是想打的字時（重打同一個位置、按空白或聲調後音節不存在）整段變回原本的按鍵當英文，馬上按 Esc 可變回注音；ai、up 這類也是注音的字，按 ↓ 從候選最後兩項選英文。上游模式下 Shift＋字母可開始英文組字段；上游判斷僅支援標準注音。倚天可使用{RawKeyMixingName}；跟隨配置時依實際鍵盤生效。使用上游判斷時，數字列的鍵也會打出數字（例如 2026年、3.14），由前文判斷是數字還是注音。";
    public string MixedToleranceHelp => $"只修正{RawKeyMixingName}的相鄰重複鍵；上游英文段保持原樣。預設關閉，原始按鍵可由候選選回。";
    /// The mode needs the panel for its Zhuyin hint (`Document::reading_panel`). The stored choice survives, so leaving the mode restores it.
    public bool UsesRawKeyMixing => IsZhuyin && MixedMode == 1;
    public bool CanChooseReadingPanel => !UsesRawKeyMixing;
    public bool ReadingPanelShown { get => ReadingPanel || UsesRawKeyMixing; set { if (!UsesRawKeyMixing) ReadingPanel = value; } }
    public string ReadingPanelHelp => (UsesRawKeyMixing ? $"「{RawKeyMixingName}」要靠這個面板顯示注音提示，選用時固定開啟。" : "預設關閉，組字只顯示在 App 內，沒有候選時不顯示面板；可依需要開啟。")
        + "開啟後在候選窗位置顯示整句組字，游標移進句子時會標出游標所在的字（游標在句尾時不標）。";
    private void NotifyReadingPanel()
    {
        OnPropertyChanged(nameof(UsesRawKeyMixing)); OnPropertyChanged(nameof(CanChooseReadingPanel));
        OnPropertyChanged(nameof(ReadingPanelShown)); OnPropertyChanged(nameof(ReadingPanelHelp));
    }
    [ObservableProperty, NotifyPropertyChangedFor(nameof(ReadingPanelShown))] public partial bool ReadingPanel { get; set; }
    partial void OnKeyboardChanged(int value) => NotifyReadingPanel();
    [ObservableProperty, NotifyPropertyChangedFor(nameof(IsZhuyin)), NotifyPropertyChangedFor(nameof(SupportsUpstreamEnglish)), NotifyPropertyChangedFor(nameof(UsesUpstreamEnglish)), NotifyPropertyChangedFor(nameof(SupportsFuzzyAdjacent))] public partial int Keyboard { get; set; }
    public bool IsZhuyin => Keyboard < 3;
    [ObservableProperty] public partial bool StandardInstalled { get; set; } = true;
    [ObservableProperty] public partial bool EtenInstalled { get; set; } = true;
    [ObservableProperty] public partial string InstalledKeyboardHelp { get; set; } = "";
    [ObservableProperty] public partial bool SymbolKeyboard { get; set; }
    public Models.PunctuationKey[] ShiftKeys { get; } = Models.PunctuationKey.ShiftRow();
    public Models.PunctuationKey[] PlainKeys { get; } = Models.PunctuationKey.PlainRow();
    public bool ShiftPunctuation => ShiftKeys.Any(k => k.IsOn);
    [ObservableProperty] public partial bool StandaloneTones { get; set; }
    [ObservableProperty] public partial bool CtrlBackslash { get; set; }
    [ObservableProperty] public partial bool ForgetCandidate { get; set; }
    [ObservableProperty] public partial bool ModeIndicator { get; set; }
    [ObservableProperty, NotifyPropertyChangedFor(nameof(CandidateHelp))] public partial bool EnterCommits { get; set; }
    [ObservableProperty] public partial bool UpCycles { get; set; }
    [ObservableProperty] public partial bool BackspaceDeletes { get; set; }
    [ObservableProperty] public partial string DictionaryText { get; set; } = "";
    [ObservableProperty] public partial int Theme { get; set; }
    [ObservableProperty] public partial double FontSize { get; set; }
    /// <summary>Glass density (玻璃濃度): the tint layer's opacity in percent, default 42.</summary>
    [ObservableProperty] public partial double GlassTint { get; set; }
    [ObservableProperty] public partial string Status { get; set; } = "";
    [ObservableProperty, NotifyPropertyChangedFor(nameof(DraftHelp))] public partial bool HasChanges { get; set; }
    public string DraftHelp => HasChanges ? "尚未儲存" : "已載入設定";
    public string CandidateHelp => (RightAction switch {
        1 => Interaction == 0 ? "↑／↓ 或 ← 選字；Tab 或 → 換下一頁。" : "← 或 ↑／↓ 選字；→ 換下一頁。Tab 關閉候選並交給 App。",
        2 => Interaction == 0 ? "↑／↓ 逐項選字；Tab 或 → 展開三欄候選。" : "← 選字、→ 展開三欄候選；↑／↓ 逐項移動。Tab 關閉候選並交給 App。",
        _ => Interaction == 0 ? "↑／↓ 或 ←／→ 選字，Tab 移到下一候選。" : "←／→ 或 ↑／↓ 選字。Tab 關閉候選並交給 App。"
    })
        + (SpaceOperates ? "空白依下方設定操作候選。" : "空白不操作候選。")
        + (EnterCommits ? (EnterToCursor ? "Enter 選字並提交游標前的組字。" : "Enter 選字並提交整句。") : "Enter 選字後保留組字。");
    public string ModelHelp => AvailableModels.Any(m=>m.Id==ModelId)
        ? "儲存後先提交目前組字，再套用所選模型；個人詞庫及學習資料保留。內建配套供離線及失效備援使用。"
            +(ModelId==ModelCatalog.Builtin?"":"\n\n"+ModelCatalog.Notes(AvailableModels.First(m=>m.Id==ModelId).Catalog))
        : "所選配套目前不存在或未通過驗證，輸入時使用內建模型。原選擇已保留，可重新下載；選取其他模型並儲存可更換。";
}
