using CommunityToolkit.Mvvm.ComponentModel;
namespace Shanjie_UI.ViewModels;
public partial class SettingsViewModel : ObservableObject
{
    [ObservableProperty, NotifyPropertyChangedFor(nameof(CandidateHelp))] public partial int Interaction { get; set; }
    [ObservableProperty] public partial int Trigger { get; set; }
    [ObservableProperty] public partial int Anchor { get; set; }
    [ObservableProperty] public partial int Ranking { get; set; }
    [ObservableProperty, NotifyPropertyChangedFor(nameof(ModelHelp))] public partial int Model { get; set; }
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
    [ObservableProperty] public partial bool PinyinSpaceConverts { get; set; }
    [ObservableProperty] public partial bool SpacePages { get; set; }
    [ObservableProperty] public partial bool WrapPages { get; set; }
    [ObservableProperty] public partial bool BackquoteSymbols { get; set; }
    [ObservableProperty] public partial bool CodeInput { get; set; }
    [ObservableProperty] public partial bool Learning { get; set; }
    [ObservableProperty] public partial bool Associations { get; set; }
    [ObservableProperty] public partial bool Prediction { get; set; }
    [ObservableProperty] public partial bool Demote { get; set; }
    [ObservableProperty] public partial bool UseContext { get; set; }
    [ObservableProperty] public partial bool Alphanumeric { get; set; }
    [ObservableProperty] public partial int Keyboard { get; set; }
    [ObservableProperty] public partial bool Big5 { get; set; }
    [ObservableProperty] public partial bool SymbolKeyboard { get; set; }
    [ObservableProperty] public partial bool ShiftPunctuation { get; set; }
    [ObservableProperty] public partial bool PlainPunctuation { get; set; }
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
        + "Tab 移到下一候選。空白可翻頁。"
        + (EnterCommits ? "Enter 選字並提交整句。" : "Enter 選字後保留組字。");
    public string ModelHelp => Model switch
    {
        0 => "原始模型，未使用詞類表。",
        1 => "整理簡繁轉換與詞頻計數，未使用詞類表。",
        2 => "改善臺灣常用字形，與 classes-v1 詞類表一起使用。",
        3 => "與 classes-v2 詞類表一起使用。上游量測 v3／v4 的選字與預測結果相同；v4 另含模型來源指紋。",
        _ => "請選擇一個語言模型。"
    };
}
