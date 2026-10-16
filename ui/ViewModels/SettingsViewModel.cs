using CommunityToolkit.Mvvm.ComponentModel;
namespace Shanjie_UI.ViewModels;
public partial class SettingsViewModel : ObservableObject
{
    [ObservableProperty, NotifyPropertyChangedFor(nameof(CandidateHelp))] public partial int Interaction { get; set; }
    [ObservableProperty] public partial int Trigger { get; set; }
    [ObservableProperty] public partial int Anchor { get; set; }
    [ObservableProperty] public partial int CandidateLayout { get; set; }
    [ObservableProperty] public partial int Ranking { get; set; }
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
    [ObservableProperty, NotifyPropertyChangedFor(nameof(CanWrapSpacePages))] public partial bool SpacePages { get; set; }
    public bool CanWrapSpacePages => SpaceOperates && SpacePages;
    [ObservableProperty] public partial bool WrapPages { get; set; }
    [ObservableProperty] public partial bool BackquoteSymbols { get; set; }
    [ObservableProperty] public partial bool CodeInput { get; set; }
    [ObservableProperty] public partial bool Learning { get; set; }
    [ObservableProperty] public partial bool Associations { get; set; }
    [ObservableProperty] public partial bool Prediction { get; set; }
    [ObservableProperty] public partial bool Demote { get; set; }
    [ObservableProperty] public partial bool UseContext { get; set; }
    [ObservableProperty] public partial bool Alphanumeric { get; set; }
    [ObservableProperty] public partial bool MixedTolerance { get; set; }
    [ObservableProperty] public partial bool AddWord { get; set; }
    [ObservableProperty] public partial int AddWordSide { get; set; }
    [ObservableProperty] public partial int AddWordShortcut { get; set; }
    [ObservableProperty] public partial double AddWordInterval { get; set; } = 450;
    [ObservableProperty] public partial int MixedMode { get; set; }
    [ObservableProperty] public partial bool EnglishInline { get; set; }
    [ObservableProperty] public partial bool EnterToCursor { get; set; }
    public bool SupportsUpstreamEnglish => Keyboard<2 && IsZhuyin;
    public bool UsesEnglishList => SupportsUpstreamEnglish && MixedMode >= 2;
    partial void OnMixedModeChanged(int value) => OnPropertyChanged(nameof(UsesEnglishList));
    [ObservableProperty, NotifyPropertyChangedFor(nameof(IsZhuyin)), NotifyPropertyChangedFor(nameof(SupportsUpstreamEnglish)), NotifyPropertyChangedFor(nameof(UsesEnglishList))] public partial int Keyboard { get; set; }
    public bool IsZhuyin => Keyboard < 3;
    [ObservableProperty] public partial bool StandardInstalled { get; set; } = true;
    [ObservableProperty] public partial bool EtenInstalled { get; set; } = true;
    [ObservableProperty] public partial string InstalledKeyboardHelp { get; set; } = "";
    [ObservableProperty] public partial bool Big5 { get; set; }
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
    [ObservableProperty] public partial string Status { get; set; } = "";
    [ObservableProperty, NotifyPropertyChangedFor(nameof(DraftHelp))] public partial bool HasChanges { get; set; }
    public string DraftHelp => HasChanges ? "尚未儲存" : "已載入設定";
    public string CandidateHelp => (Interaction == 0 ? "↑／↓ 或 ←／→ 選字。" : "←／→ 或 ↑／↓ 選字。")
        + "Tab 移到下一候選。" + (SpaceOperates ? "空白依下方設定操作候選。" : "空白不操作候選。")
        + (EnterCommits ? "Enter 選字並提交整句。" : "Enter 選字後保留組字。");
    public string ModelHelp => AvailableModels.Any(m=>m.Id==ModelId)
        ? "儲存後先提交目前組字，再套用所選模型；個人詞庫及學習資料保留。內建配套供離線及失效備援使用。"
        : "所選配套目前不存在或未通過驗證，輸入時使用內建模型。原選擇已保留，可重新下載；選取其他模型並儲存可更換。";
}
