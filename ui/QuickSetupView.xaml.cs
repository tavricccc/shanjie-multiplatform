using System.Numerics;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Shanjie_UI.Controls;
using Shanjie_UI.Setup;
using Shanjie_UI.ViewModels;
using Windows.System;
namespace Shanjie_UI;

/// <summary>
/// Quick Setup: choose a template, adjust the few choices that matter, then try typing. It edits its own
/// <see cref="QuickSetupDraft"/>, not the settings page's draft, and writes only when the person moves past the details.
/// </summary>
public sealed partial class QuickSetupView : UserControl
{
    const int TemplateStep = 0, DetailsStep = 1, TryStep = 2, StepCount = 3;
    readonly QuickSetupDraft draft = new();
    readonly List<DetailRow> rows = [];
    readonly Action<bool> finished;
    readonly Action minimize, close;
    int step = -1;
    bool applied, syncing;
    DetailRow? active;

    internal QuickSetupView(bool reopened, Action<bool> finished, Action minimize, Action close)
    {
        this.finished = finished; this.minimize = minimize; this.close = close;
        InitializeComponent();
        TemplateLead.Text = draft.HadSettings ? "這個版本新增了快速設定。挑一種打字方式，或保留現在的設定；略過也不會改動任何東西。" : "先挑一種你習慣的打字方式，下一步還能再微調。";
        KeepCard.Visibility = draft.HadSettings ? Visibility.Visible : Visibility.Collapsed;
        (draft.Template switch { QuickTemplate.Shanjie => ShanjieCard, QuickTemplate.Windows => WindowsCard, _ => KeepCard }).IsChecked = true;
        SkipButton.Content = reopened ? "取消" : draft.HadSettings ? "略過，保留現在的設定" : "略過，先用預設值";
        for (int i = 0; i < StepCount; i++) Dots.Children.Add(new Border { Width = 7, Height = 7, CornerRadius = new CornerRadius(3.5) });
        ShanjieDemo.Play(QuickSetupDemos.Template(false));
        WindowsDemo.Play(QuickSetupDemos.Template(true));
        Show(TemplateStep);
        Loaded += (_, _) => FocusFirst();
    }
    internal FrameworkElement Caption => CaptionArea;
    internal QuickSetupDraft DraftForVerification => draft;
    internal int StepForVerification => step;
    internal TypingDemo DetailDemoForVerification => DetailDemo;
    internal TypingDemo TemplateDemoForVerification(bool windows) => windows ? WindowsDemo : ShanjieDemo;
    internal double TemplateOverflowForVerification { get { TemplatePage.UpdateLayout(); return TemplatePage.ScrollableHeight; } }
    internal bool SkipOfferedForVerification => SkipButton.Visibility == Visibility.Visible;
    internal bool KeepOfferedForVerification => KeepCard.Visibility == Visibility.Visible;
    internal string SkipTextForVerification => (string)SkipButton.Content;
    internal IEnumerable<string> RowKeysForVerification => rows.Select(row => row.Key);
    internal void ChooseForVerification(QuickTemplate template) => (template switch { QuickTemplate.Shanjie => ShanjieCard, QuickTemplate.Windows => WindowsCard, _ => KeepCard }).IsChecked = true;
    internal void ActivateForVerification(string key) => Activate(rows.First(row => row.Key == key));
    internal void SelectForVerification(string key, int index) => rows.First(row => row.Key == key).Choice.SelectedIndex = index;
    internal int RowOptionCountForVerification(string key) => rows.First(row => row.Key == key).Options().Length;
    internal bool RowEnabledForVerification(string key) => rows.First(row => row.Key == key).Choice.IsEnabled;
    internal string RowShownForVerification(string key) => rows.First(row => row.Key == key).Shown;
    internal string RowTitleForVerification(string key) => rows.First(row => row.Key == key).Title;
    internal void NextForVerification() => Next(this, new RoutedEventArgs());
    internal void BackForVerification() => Back(this, new RoutedEventArgs());
    internal void SkipForVerification() => Skip(this, new RoutedEventArgs());

    void MinimizeWindow(object sender, RoutedEventArgs args) => minimize();
    void CloseWindow(object sender, RoutedEventArgs args) => close();
    void TemplateChecked(object sender, RoutedEventArgs args)
    {
        if (sender == ShanjieCard) draft.Choose(QuickTemplate.Shanjie);
        else if (sender == WindowsCard) draft.Choose(QuickTemplate.Windows);
        else draft.Choose(QuickTemplate.Keep);
    }
    /// A radio group moves with the arrow keys; these cards are not inside a RadioButtons control.
    void CardKeyDown(object sender, KeyRoutedEventArgs args)
    {
        var cards = new[] { ShanjieCard, WindowsCard, KeepCard }.Where(card => card.Visibility == Visibility.Visible).ToList();
        int move = args.Key switch { VirtualKey.Left or VirtualKey.Up => -1, VirtualKey.Right or VirtualKey.Down => 1, _ => 0 };
        if (move == 0) return;
        var next = cards[(cards.IndexOf((RadioButton)sender) + move + cards.Count) % cards.Count];
        next.IsChecked = true; next.Focus(FocusState.Keyboard); args.Handled = true;
    }
    void Skip(object sender, RoutedEventArgs args) => finished(applied);
    void Back(object sender, RoutedEventArgs args) { if (step > TemplateStep) { Show(step - 1); FocusFirst(); } }
    void Next(object sender, RoutedEventArgs args)
    {
        switch (step)
        {
            case TemplateStep: BuildRows(); Show(DetailsStep); FocusFirst(); break;
            case DetailsStep: if (Apply()) { BuildSummary(); Show(TryStep); FocusFirst(); } break;
            default: finished(applied); break;
        }
    }
    bool Apply()
    {
        try { applied = draft.Apply(); ErrorText.Visibility = Visibility.Collapsed; return true; }
        catch (Exception error) when (error is InvalidDataException or IOException or UnauthorizedAccessException)
        {
            ErrorText.Text = "無法儲存設定：" + error.Message; ErrorText.Visibility = Visibility.Visible; return false;
        }
    }
    void Show(int next)
    {
        int previous = step; step = next;
        TemplatePage.Visibility = next == TemplateStep ? Visibility.Visible : Visibility.Collapsed;
        DetailsPage.Visibility = next == DetailsStep ? Visibility.Visible : Visibility.Collapsed;
        TryPage.Visibility = next == TryStep ? Visibility.Visible : Visibility.Collapsed;
        if (next != DetailsStep) DetailDemo.Stop();
        if (next != TemplateStep) { ShanjieDemo.Stop(); WindowsDemo.Stop(); }
        else if (previous >= 0) { ShanjieDemo.RestartForVerification(); WindowsDemo.RestartForVerification(); }
        BackButton.Visibility = next == TemplateStep ? Visibility.Collapsed : Visibility.Visible;
        // Once the choices are saved, skipping would no longer keep the earlier settings, so it stops being offered.
        SkipButton.Visibility = next == TryStep || applied ? Visibility.Collapsed : Visibility.Visible;
        NextButton.Content = next switch { TemplateStep => "下一步", DetailsStep => "套用，試打看看", _ => "開始使用" };
        ErrorText.Visibility = Visibility.Collapsed;
        for (int i = 0; i < Dots.Children.Count; i++) ((Border)Dots.Children[i]).Background = Motion.Resource(i == next ? "SjAccentBrush" : "SjText3Brush");
        AutomationProperties.SetName(Dots, $"第 {next + 1} 步，共 {StepCount} 步");
        if (next == TryStep) { TryTitle.Text = applied || !draft.HadSettings ? "都好了，試打看看" : "沒有改動，試打看看"; TryLead.Text = applied ? "設定已經存好，善解馬上套用。" : draft.HadSettings ? "你的設定維持原樣。" : "你選的就是預設值，善解已經照這樣運作。"; }
        if (previous >= 0) Slide(next > previous ? 1 : -1);
    }
    /// The new page drifts in from the side it came from; nothing moves with Windows animations off.
    void Slide(int direction)
    {
        if (!TypingDemo.Animated) return;
        ElementCompositionPreview.SetIsTranslationEnabled(Pages, true);
        var visual = ElementCompositionPreview.GetElementVisual(Pages);
        var c = visual.Compositor;
        var move = c.CreateVector3KeyFrameAnimation();
        move.InsertKeyFrame(0, new Vector3(24 * direction, 0, 0));
        move.InsertKeyFrame(1, Vector3.Zero, Motion.Ease(c, Motion.Smooth));
        move.Duration = Motion.Duration(Motion.Normal);
        visual.StartAnimation("Translation", move);
        var fade = c.CreateScalarKeyFrameAnimation();
        fade.InsertKeyFrame(0, 0); fade.InsertKeyFrame(1, 1, Motion.Ease(c, Motion.Smooth));
        fade.Duration = Motion.Duration(Motion.Normal);
        visual.StartAnimation("Opacity", fade);
    }
    void FocusFirst()
    {
        Control target = step switch
        {
            TemplateStep => draft.Template switch { QuickTemplate.Shanjie => ShanjieCard, QuickTemplate.Windows => WindowsCard, _ => KeepCard },
            DetailsStep => rows[0].Head,
            _ => TryBox,
        };
        target.Focus(FocusState.Programmatic);
    }

    sealed class DetailRow
    {
        internal required string Key, Title;
        internal required Func<string[]> Options;
        internal required Func<int> Read;
        internal required Action<int> Write;
        internal required Func<string> Help;
        internal required Func<DemoScript> Script;
        internal Func<bool> Enabled = () => true;
        internal Button Head = null!;
        internal Segmented Choice = null!;
        internal TextBlock Value = null!, HelpText = null!;
        internal Border Card = null!;
        internal StackPanel Body = null!;
        internal string Shown => Options()[Math.Clamp(Read(), 0, Options().Length - 1)];
    }
    /// The words are checked against what each key does in `windows/src/tsf/keys.rs` and the help in SettingsViewModel.
    IEnumerable<DetailRow> Definitions()
    {
        bool windows = draft.WindowsInteraction;
        SpaceChoice[] spaces = draft.Space == SpaceChoice.Custom ? [SpaceChoice.Insert, SpaceChoice.OpenList, SpaceChoice.Commit, SpaceChoice.Custom] : [SpaceChoice.Insert, SpaceChoice.OpenList, SpaceChoice.Commit];
        yield return new DetailRow
        {
            Key = "space", Title = "打完字按空白鍵",
            Options = () => spaces.Select(s => s switch { SpaceChoice.Insert => "插入空格", SpaceChoice.OpenList => "打開選字", SpaceChoice.Commit => "直接送出", _ => "維持現在的" }).ToArray(),
            Read = () => Array.IndexOf(spaces, draft.Space),
            Write = index => draft.Space = spaces[index],
            Help = () => (draft.Space switch
            {
                SpaceChoice.Insert => "句子裡多一個空格，可以繼續打；要選字按 ↓。",
                SpaceChoice.OpenList => "打開選字清單，再按一次換下一頁。",
                SpaceChoice.Commit => draft.SpaceAfterCommit ? "字直接送到 App，再補一個空格。" : "字直接送到 App。",
                _ => "你現在的空白鍵是自訂組合，這裡不會改動。",
            }) + "注音還沒打完時，空白鍵都是第一聲。",
            Script = () => QuickSetupDemos.Space(draft.Space, draft.Vertical, draft.SpaceAfterCommit),
        };
        yield return new DetailRow
        {
            Key = "direction", Title = "選字清單的方向",
            Options = () => ["橫向", "直向"],
            Read = () => draft.Vertical ? 1 : 0,
            Write = index => draft.Vertical = index == 1,
            Help = () => draft.Vertical ? "一項一行，容易一眼掃過，和微軟注音一樣。" : "排成一排，占的高度少，比較不會遮到文字。",
            Script = () => QuickSetupDemos.Direction(draft.Vertical),
        };
        yield return new DetailRow
        {
            Key = "right", Title = windows ? "選字時按 Tab 或 →" : "選字時按 →",
            Options = () => ["移到下一個", "翻下一頁", "展開大表"],
            Read = () => draft.RightAction,
            Write = index => draft.RightAction = index,
            Help = () => draft.RightAction switch
            {
                1 => "直接翻到下一頁。",
                2 => "展開成三欄，一次看更多字。",
                _ => "往下一個字移動。",
            } + (windows ? "" : "Tab 在這個打字方式會送出字，再把 Tab 交給 App。"),
            Script = () => QuickSetupDemos.Right(draft.RightAction, windows, draft.Vertical),
        };
        if (draft.IsZhuyin)
        {
            // Off and both are only ever the person's own earlier choice; they stay until another option is picked.
            MixChoice[] mixes = draft.SupportsAutomatic ? [MixChoice.Automatic, MixChoice.RawKeys] : [MixChoice.Off, MixChoice.RawKeys];
            // Without the automatic judgement the stored upstream flag does nothing, so it reads as off.
            MixChoice Shown() => draft.Mix == MixChoice.Automatic && !draft.SupportsAutomatic ? MixChoice.Off : draft.Mix == MixChoice.Both && !draft.SupportsAutomatic ? MixChoice.RawKeys : draft.Mix;
            if (!mixes.Contains(Shown())) mixes = [.. mixes, Shown()];
            yield return new DetailRow
            {
                Key = "mix", Title = "中英文混著打",
                Options = () => mixes.Select(m => m switch { MixChoice.Automatic => "自動判斷", MixChoice.RawKeys => SettingsViewModel.RawKeyMixingName, MixChoice.Off => draft.SupportsAutomatic ? "維持關閉" : "不混打", _ => "維持兩者並用" }).ToArray(),
                Read = () => Array.IndexOf(mixes, Shown()),
                Write = index => { if (mixes[index] is MixChoice.Automatic or MixChoice.RawKeys) draft.Mix = mixes[index]; else if (!draft.SupportsAutomatic) draft.Mix = MixChoice.Automatic; },
                Help = () => Shown() switch
                {
                    MixChoice.Automatic => "平常打注音；拼不成注音時，那段自動變回英文，馬上按 Esc 可以變回注音。",
                    MixChoice.RawKeys => "先顯示你按的字母，旁邊提示注音；按聲調變中文，按 Enter 留英文。會固定開啟組字面板。",
                    MixChoice.Off => draft.SupportsAutomatic ? "你現在關閉了混打，這裡不會改動。" : "倚天注音用不到自動判斷，想混打請選" + SettingsViewModel.RawKeyMixingName + "。",
                    _ => $"你現在兩種並用：{SettingsViewModel.RawKeyMixingName}先判斷，其餘自動判斷。這裡不會改動。",
                },
                Script = () => Shown() switch { MixChoice.RawKeys => QuickSetupDemos.MixRawKeys(), MixChoice.Automatic => QuickSetupDemos.MixAutomatic(), _ => QuickSetupDemos.MixUnchanged() },
            };
        }
        yield return new DetailRow
        {
            Key = "shift", Title = "單按 Shift",
            Options = () => ["切換中英", "不切換"],
            Read = () => draft.ShiftToggle ? 0 : 1,
            Write = index => draft.ShiftToggle = index == 0,
            Help = () => draft.ShiftToggle ? "單按一下就在中文、英文之間切換。" : "單按 Shift 不做任何事；要切換請按工作列的「中／A」。",
            Script = () => QuickSetupDemos.Shift(draft.ShiftToggle),
        };
        yield return new DetailRow
        {
            Key = "learning", Title = "記住我選的字",
            Options = () => ["記住", "不記"],
            Read = () => draft.Learning ? 0 : 1,
            Write = index => draft.Learning = index == 0,
            Help = () => draft.Learning ? "你改選過的字，下次遇到同樣的前文會優先出現。只存在這台電腦。" : "每次都照內建的順序，不記你選過什麼。",
            Script = () => QuickSetupDemos.Learning(draft.Learning, draft.EnterCommits, draft.Vertical),
        };
        yield return new DetailRow
        {
            Key = "panel", Title = "組字面板",
            Options = () => ["關閉", "開啟"],
            Read = () => draft.PanelShown ? 1 : 0,
            Write = index => draft.PanelChoice = index == 1,
            Enabled = () => !draft.PanelForced,
            Help = () => draft.PanelForced ? $"{SettingsViewModel.RawKeyMixingName}要靠它提示注音，所以固定開啟；換回其他混打方式就恢復你原本的選擇。"
                : draft.PanelShown ? "選字清單的位置多一條整句。游標移進句子時會標出游標所在的字；游標在句尾時不標。"
                : "預設關閉，字就打在 App 裡。開啟的好處：游標移進句子時會標出游標所在的字（在句尾時不標）。",
            Script = () => QuickSetupDemos.Panel(draft.PanelShown, draft.AnchorRight),
        };
    }
    /// Built when the page opens, because a template decides the labels and the starting values.
    void BuildRows()
    {
        string? keep = active?.Key;
        rows.Clear(); Rows.Children.Clear(); active = null;
        foreach (var row in Definitions())
        {
            var title = new TextBlock { Text = row.Title, FontSize = 13, VerticalAlignment = VerticalAlignment.Center, Foreground = Motion.Resource("SjText1Brush") };
            row.Value = new TextBlock { FontSize = 12, VerticalAlignment = VerticalAlignment.Center, Foreground = Motion.Resource("SjText2Brush") };
            var line = new Grid { ColumnSpacing = 12 };
            line.ColumnDefinitions.Add(new ColumnDefinition()); line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            line.Children.Add(title); Grid.SetColumn(row.Value, 1); line.Children.Add(row.Value);
            row.Head = new Button { Style = (Style)Root.Resources["RowHead"], Content = line };
            AutomationProperties.SetName(row.Head, row.Title);
            row.Choice = new Segmented { Options = string.Join('|', row.Options()), MinWidth = 0 };
            AutomationProperties.SetName(row.Choice, row.Title);
            row.HelpText = new TextBlock { FontSize = 12, LineHeight = 18, TextWrapping = TextWrapping.Wrap, Foreground = Motion.Resource("SjText2Brush") };
            row.Body = new StackPanel { Spacing = 8, Padding = new Thickness(14, 0, 14, 12), Visibility = Visibility.Collapsed };
            row.Body.Children.Add(row.Choice); row.Body.Children.Add(row.HelpText);
            var stack = new StackPanel(); stack.Children.Add(row.Head); stack.Children.Add(row.Body);
            row.Card = new Border { CornerRadius = new CornerRadius(12), BorderThickness = new Thickness(1), BorderBrush = new SolidColorBrush(Microsoft.UI.Colors.Transparent), Child = stack };
            row.Head.Click += (_, _) => Activate(row);
            row.Head.GotFocus += (_, _) => Activate(row);
            row.Choice.RegisterPropertyChangedCallback(Segmented.SelectedIndexProperty, (_, _) => Chosen(row));
            rows.Add(row); Rows.Children.Add(row.Card);
        }
        Refresh();
        Activate(rows.FirstOrDefault(row => row.Key == keep) ?? rows[0]);
    }
    void Chosen(DetailRow row)
    {
        if (syncing || row.Choice.SelectedIndex == row.Read() || row.Choice.SelectedIndex < 0) return;
        row.Write(row.Choice.SelectedIndex);
        Refresh();
        if (active is { } current) DetailDemo.Play(current.Script());
    }
    void Refresh()
    {
        syncing = true;
        foreach (var row in rows)
        {
            row.Choice.SelectedIndex = row.Read();
            row.Choice.IsEnabled = row.Enabled();
            row.Value.Text = row.Shown;
            row.HelpText.Text = row.Help();
        }
        syncing = false;
    }
    void Activate(DetailRow row)
    {
        bool changed = active != row;
        active = row;
        foreach (var other in rows)
        {
            bool open = other == row;
            other.Body.Visibility = open ? Visibility.Visible : Visibility.Collapsed;
            other.Value.Visibility = open ? Visibility.Collapsed : Visibility.Visible;
            other.Card.Background = open ? Motion.Resource("SjFillControlBrush") : new SolidColorBrush(Microsoft.UI.Colors.Transparent);
            other.Card.BorderBrush = open ? Motion.Resource("SjStrokeBrush") : new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        }
        DetailsLead.Text = $"以「{TemplateName}」為基礎，每一項都已經選好，想改再改。右邊的動畫會跟著你的選擇變。";
        if (changed) DetailDemo.Play(row.Script());
    }
    string TemplateName => draft.Template switch { QuickTemplate.Shanjie => "善解", QuickTemplate.Windows => "微軟注音風格", _ => "你現在的設定" };
    void BuildSummary()
    {
        Summary.Children.Clear(); Summary.RowDefinitions.Clear();
        void Line(string name, string value)
        {
            int row = Summary.RowDefinitions.Count;
            Summary.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var label = new TextBlock { Text = name, FontSize = 12, Foreground = Motion.Resource("SjText3Brush") };
            var shown = new TextBlock { Text = value, FontSize = 12, TextWrapping = TextWrapping.Wrap, Foreground = Motion.Resource("SjText1Brush") };
            Grid.SetRow(label, row); Grid.SetRow(shown, row); Grid.SetColumn(shown, 1);
            Summary.Children.Add(label); Summary.Children.Add(shown);
        }
        Line("打字方式", TemplateName);
        foreach (var row in rows) Line(row.Title, row.Shown);
    }
}
