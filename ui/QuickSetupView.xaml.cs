using System.Numerics;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Shanjie_UI.Controls;
using Shanjie_UI.Setup;
using Shanjie_UI.ViewModels;
using Windows.System;
namespace Shanjie_UI;

/// <summary>
/// Quick Setup, one question per screen: a welcome, the way of typing (template), each choice that matters, then a
/// finish with a try box. It edits its own <see cref="QuickSetupDraft"/>, not the settings page's draft, and writes
/// only when the person moves past the last question. Steps slide in from the side they come from, a few parts at a
/// time; with Windows animations off they simply appear.
/// </summary>
public sealed partial class QuickSetupView : UserControl
{
    const int WelcomeStep = 0, TemplateStep = 1;
    readonly QuickSetupDraft draft = new();
    readonly List<Question> questions = [];
    readonly Action<bool> finished;
    readonly Action minimize, close;
    readonly bool reopened;
    int step = -1;
    bool applied, leaving;
    Page? page;
    Choreography? motion;

    internal QuickSetupView(bool reopened, Action<bool> finished, Action minimize, Action close)
    {
        this.reopened = reopened; this.finished = finished; this.minimize = minimize; this.close = close;
        InitializeComponent();
        SkipButton.Content = reopened ? "取消" : draft.HadSettings ? "略過，保留現在的設定" : "略過，先用預設值";
        BuildQuestions();
        AutomationProperties.SetLiveSetting(StepText, AutomationLiveSetting.Polite);
        Loaded += (_, _) => { if (step < 0) Show(WelcomeStep); };
    }
    int FinishStep => TemplateStep + questions.Count + 1;
    internal FrameworkElement Caption => CaptionArea;

    internal QuickSetupDraft DraftForVerification => draft;
    internal int StepForVerification => step;
    internal int FinishStepForVerification => FinishStep;
    internal TypingDemo? DemoForVerification => page?.Demo;
    internal bool SkipOfferedForVerification => SkipButton.Visibility == Visibility.Visible;
    internal bool KeepOfferedForVerification => TemplateQuestion().Options().Length == 3;
    internal string SkipTextForVerification => (string)SkipButton.Content;
    internal string NextTextForVerification => (string)NextButton.Content;
    internal string TitleForVerification => page?.Title ?? "";
    internal IEnumerable<string> RowKeysForVerification => questions.Select(q => q.Key);
    internal double OverflowForVerification { get { if (page is null) return 0; page.Root.UpdateLayout(); return page.Scroll?.ScrollableHeight ?? 0; } }
    internal void ChooseForVerification(QuickTemplate template) => Answer(TemplateQuestion(), template switch { QuickTemplate.Shanjie => 0, QuickTemplate.Windows => 1, _ => 2 });
    /// Goes straight to a question, as if the person had walked there.
    internal void ActivateForVerification(string key) => Show(TemplateStep + 1 + questions.FindIndex(q => q.Key == key));
    internal void SelectForVerification(string key, int index) => Answer(questions.First(q => q.Key == key), index);
    internal int RowOptionCountForVerification(string key) => questions.First(q => q.Key == key).Options().Length;
    internal bool RowEnabledForVerification(string key) => questions.First(q => q.Key == key).Enabled();
    internal string RowShownForVerification(string key) => questions.First(q => q.Key == key).Shown;
    internal string RowTitleForVerification(string key) => questions.First(q => q.Key == key).Title();
    internal string[] RowOptionsForVerification(string key) => questions.First(q => q.Key == key).Options();
    internal void NextForVerification() => Next(this, new RoutedEventArgs());
    internal void BackForVerification() => Back(this, new RoutedEventArgs());
    internal void SkipForVerification() => Skip(this, new RoutedEventArgs());
    internal void KeyForVerification(VirtualKey key) => Handle(key, null);
    /// Freezes the running step change (or finish) at one point of its timeline, for a screenshot.
    internal int HoldForVerification(float progress) => motion?.FreezeForVerification(progress) ?? 0;
    internal float ProgressForVerification => Progress.FillForVerification;

    void MinimizeWindow(object sender, RoutedEventArgs args) => minimize();
    void CloseWindow(object sender, RoutedEventArgs args) => close();
    void Skip(object sender, RoutedEventArgs args) => Leave();
    void Back(object sender, RoutedEventArgs args) { if (step > WelcomeStep && !leaving) Show(step - 1); }
    void Next(object sender, RoutedEventArgs args)
    {
        if (leaving) return;
        if (step == TemplateStep) BuildQuestions();
        if (step == FinishStep - 1 && !Apply()) return;
        if (step == FinishStep) Leave();
        else Show(step + 1);
    }
    /// Enter goes on, Esc goes back; inside the try box both belong to the text.
    void RootKeyDown(object sender, KeyRoutedEventArgs args) { if (!args.Handled) args.Handled = Handle(args.Key, args.OriginalSource); }
    bool Handle(VirtualKey key, object? source)
    {
        if (source is TextBox || (key == VirtualKey.Enter && source is Button)) return false;
        if (key == VirtualKey.Enter) { Next(this, new RoutedEventArgs()); return true; }
        if (key == VirtualKey.Escape && step > WelcomeStep) { Back(this, new RoutedEventArgs()); return true; }
        return false;
    }
    /// Skipping, closing the flow and starting to type all hand back to the settings: the flow fades and settles away.
    void Leave()
    {
        if (leaving) return;
        leaving = true;
        var t = new Choreography(ElementCompositionPreview.GetElementVisual(Root).Compositor, 280);
        var v = Choreography.Of(Root);
        v.CenterPoint = new Vector3((float)Root.ActualWidth / 2, (float)Root.ActualHeight / 2, 0);
        t.Scalar(v, "Opacity", 1, 0, 0, 260, Motion.Exit);
        t.Vector3(v, "Scale", Vector3.One, new Vector3(0.985f, 0.985f, 1), 0, 280, Motion.Exit);
        motion = t;
        t.Then(() => finished(applied));
    }
    bool Apply()
    {
        try { applied = draft.Apply(); ErrorText.Visibility = Visibility.Collapsed; return true; }
        catch (Exception error) when (error is InvalidDataException or IOException or UnauthorizedAccessException)
        {
            ErrorText.Text = "無法儲存設定：" + error.Message; ErrorText.Visibility = Visibility.Visible; return false;
        }
    }

    /// <summary>One screen: its root, the parts that arrive one after another, and its demo if it has one.</summary>
    sealed class Page
    {
        internal required FrameworkElement Root;
        internal required List<UIElement> Parts;
        internal required string Title;
        internal TypingDemo? Demo;
        internal ChoiceCards? Cards;
        internal Control? Focus;
        internal FinishMark? Mark;
        internal ScrollViewer? Scroll;
        internal FrameworkElement? Hero;
    }

    void Show(int next)
    {
        int previous = step; step = next;
        int direction = previous < 0 || next >= previous ? 1 : -1;
        var old = page;
        page = next == WelcomeStep ? Welcome() : next == FinishStep ? Finish() : Ask(next == TemplateStep ? TemplateQuestion() : questions[next - TemplateStep - 1]);
        // A frozen or unfinished earlier change may have left more than one page behind.
        foreach (var stale in Pages.Children.Where(child => child != old?.Root).ToList()) Pages.Children.Remove(stale);
        Pages.Children.Add(page.Root);
        old?.Demo?.Stop();
        if (old is not null) old.Root.IsHitTestVisible = false;
        AutomationProperties.SetName(page.Root, page.Title);

        bool finish = next == FinishStep;
        var t = new Choreography(ElementCompositionPreview.GetElementVisual(Pages).Compositor, finish ? 2100 : 700);
        if (old is not null) t.Leave(old.Root, new Vector3(-48 * direction, 0, 0), 0, 240);
        double start = old is null ? 60 : 80;
        if (finish) page.Mark!.Play(t, start);
        if (page.Hero is { } hero)
        {
            var v = Choreography.Of(hero, false);
            v.CenterPoint = new Vector3((float)hero.Width / 2, (float)hero.Height / 2, 0);
            t.Vector3(v, "Scale", new Vector3(0.72f, 0.72f, 1), Vector3.One, start, 720, Motion.Spring);
        }
        for (int i = 0; i < page.Parts.Count; i++)
        {
            // Questions drift in from the side they come from; the welcome and the finish rise.
            var offset = finish || next == WelcomeStep ? new Vector3(0, 18, 0) : new Vector3(44 * direction, 0, 0);
            t.Enter(page.Parts[i], offset, (finish ? start + 880 : start) + i * 40, 480);
        }
        int count = FinishStep - TemplateStep;
        Progress.Set(next <= TemplateStep ? 0 : (double)(next - TemplateStep) / count, previous < 0 ? null : t);
        ProgressArea.Opacity = next == WelcomeStep ? 0 : 1;
        StepText.Text = next == WelcomeStep ? "" : $"{next - TemplateStep + 1}／{count + 1}";
        AutomationProperties.SetName(ProgressArea, next == WelcomeStep ? "" : $"第 {next - TemplateStep + 1} 步，共 {count + 1} 步");
        motion = t;
        t.Then(() => { if (old is not null && page != old) Pages.Children.Remove(old.Root); });

        BackButton.Visibility = next == WelcomeStep ? Visibility.Collapsed : Visibility.Visible;
        // Once the choices are saved, skipping would no longer keep the earlier settings, so it stops being offered.
        SkipButton.Visibility = finish || applied ? Visibility.Collapsed : Visibility.Visible;
        NextButton.Content = next == WelcomeStep ? "開始" : next == FinishStep ? "開始使用" : next == FinishStep - 1 ? "套用" : "繼續";
        ErrorText.Visibility = Visibility.Collapsed;
        if (page.Focus is { } focus) { var shown = page; shown.Root.Loaded += (_, _) => { if (page == shown) focus.Focus(FocusState.Programmatic); }; }
        if (step != previous && AutomationPeer.ListenerExists(AutomationEvents.LiveRegionChanged))
            FrameworkElementAutomationPeer.FromElement(StepText)?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);
    }

    static TextBlock Line(string text, double size, string brush, Windows.UI.Text.FontWeight? weight = null, double? height = null) => new()
    {
        Text = text, FontSize = size, TextWrapping = TextWrapping.Wrap, Foreground = Motion.Resource(brush), FontWeight = weight ?? FontWeights.Normal,
        LineHeight = height ?? 0, IsTextSelectionEnabled = false,
    };
    static ScrollViewer Scrolling(UIElement content) => new() { Content = content, HorizontalScrollMode = ScrollMode.Disabled, HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled, VerticalScrollBarVisibility = ScrollBarVisibility.Auto };

    Page Welcome()
    {
        var hero = new Grid { Width = 168, Height = 120, HorizontalAlignment = HorizontalAlignment.Center };
        if (!Motion.HighContrast)
        {
            var glow = new Microsoft.UI.Xaml.Shapes.Ellipse { Width = 168, Height = 120, Fill = Glow() };
            hero.Children.Add(glow);
        }
        var logo = new Image { Source = new BitmapImage(new Uri("ms-appx:///Assets/Square150x150Logo.scale-200.png")), Width = 76, Height = 76, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
        AutomationProperties.SetAccessibilityView(logo, AccessibilityView.Raw);
        hero.Children.Add(logo);
        string title = reopened ? "快速設定" : draft.HadSettings ? "善解多了快速設定" : "歡迎使用善解";
        string lead = reopened ? "從已儲存的設定開始，一步一步重新挑選。按「取消」不會改動任何東西。"
            : draft.HadSettings ? "回答幾個小問題，重新挑選順手的打字方式。略過的話，現在的設定完全不變。"
            : "回答幾個小問題，善解就照你的習慣打字。大約一分鐘，隨時可以略過。";
        var heading = Line(title, 32, "SjText1Brush", FontWeights.SemiBold); heading.TextAlignment = TextAlignment.Center;
        var body = Line(lead, 15, "SjText2Brush", null, 24); body.TextAlignment = TextAlignment.Center; body.MaxWidth = 460;
        var hint = Line("Enter 繼續　Esc 上一步　方向鍵選擇", 12, "SjText3Brush"); hint.TextAlignment = TextAlignment.Center; hint.Margin = new Thickness(0, 14, 0, 0);
        var column = new StackPanel { Spacing = 10, VerticalAlignment = VerticalAlignment.Center, HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(40, 0, 40, 24) };
        column.Children.Add(hero); column.Children.Add(heading); column.Children.Add(body); column.Children.Add(hint);
        return new Page { Root = new Grid { Children = { column } }, Parts = [hero, heading, body, hint], Title = title, Hero = logo };
    }
    /// A soft accent light behind the logo.
    static Brush Glow()
    {
        var accent = Motion.AccentColor;
        var brush = new RadialGradientBrush();
        brush.GradientStops.Add(new GradientStop { Offset = 0, Color = Motion.WithAlpha(accent, 0.30) });
        brush.GradientStops.Add(new GradientStop { Offset = 0.6, Color = Motion.WithAlpha(accent, 0.08) });
        brush.GradientStops.Add(new GradientStop { Offset = 1, Color = Motion.WithAlpha(accent, 0) });
        return brush;
    }

    /// A question: title and short copy with the answers on the left, the demo of the chosen answer on the right.
    Page Ask(Question q)
    {
        var eyebrow = Line(q.Eyebrow, 12, "SjText2Brush", FontWeights.SemiBold);
        eyebrow.Foreground = new SolidColorBrush(Motion.AccentColor);
        var title = Line(q.Title(), 28, "SjText1Brush", FontWeights.SemiBold, 38);
        var lead = Line(q.Lead(), 14, "SjText2Brush", null, 22);
        var cards = new ChoiceCards((Style)Root.Resources["QuickChoice"], q.Options().Select((o, i) => (o, q.Describe(i))).ToList(), Math.Max(0, q.Read()), q.Title()) { Margin = new Thickness(0, 14, 0, 0) };
        cards.SetEnabled(q.Enabled());
        var note = Line(q.Note() ?? "", 12, "SjText3Brush", null, 18);
        note.Visibility = q.Note() is null ? Visibility.Collapsed : Visibility.Visible;
        var left = new StackPanel { Spacing = 6, Padding = new Thickness(0, 0, 6, 8) };
        foreach (var part in new UIElement[] { eyebrow, title, lead, cards, note }) left.Children.Add(part);
        var demo = new TypingDemo { Height = 300 };
        demo.Play(q.Script());
        var aside = new StackPanel { Spacing = 8, VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, 30, 0, 0) };
        aside.Children.Add(demo);
        aside.Children.Add(new TextBlock { Text = "動畫是示意，實際的字依你的詞庫而定。", Style = (Style)Application.Current.Resources["SjNote"], Margin = new Thickness(2, 0, 2, 0) });
        var grid = new Grid { ColumnSpacing = 40, Padding = new Thickness(44, 26, 36, 4) };
        grid.ColumnDefinitions.Add(new ColumnDefinition()); grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(360) });
        grid.Children.Add(left); Grid.SetColumn(aside, 1); grid.Children.Add(aside);
        // The whole step scrolls, so parts sliding in are clipped only at the window's edge, not at a column's.
        var scroll = Scrolling(grid);
        var page = new Page { Root = scroll, Parts = [eyebrow, title, lead, cards, note, aside], Title = q.Title(), Demo = demo, Cards = cards, Focus = cards.Focusable, Scroll = scroll };
        cards.Chosen += index =>
        {
            Answer(q, index, fromCards: true);
            note.Text = q.Note() ?? ""; note.Visibility = q.Note() is null ? Visibility.Collapsed : Visibility.Visible;
        };
        return page;
    }
    /// Writes an answer and shows what it does. From the harness it also moves the cards when that question is on screen.
    void Answer(Question q, int index, bool fromCards = false)
    {
        if (index < 0 || index >= q.Options().Length || index == q.Read() || !q.Enabled()) return;
        q.Write(index);
        if (page?.Title != q.Title() || page.Cards is null) return;
        if (!fromCards) page.Cards.Choose(index);
        page.Demo?.Play(q.Script());
    }

    Page Finish()
    {
        var mark = new FinishMark { HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 34, 0, 10) };
        string title = applied || !draft.HadSettings ? "都好了，試打看看" : "沒有改動，試打看看";
        string lead = applied ? "設定已經存好，善解馬上套用。" : draft.HadSettings ? "你的設定維持原樣。" : "你選的就是預設值，善解已經照這樣運作。";
        var heading = Line(title, 28, "SjText1Brush", FontWeights.SemiBold); heading.TextAlignment = TextAlignment.Center;
        var body = Line(lead, 14, "SjText2Brush"); body.TextAlignment = TextAlignment.Center;
        var tryBox = new TextBox { AcceptsReturn = true, TextWrapping = TextWrapping.Wrap, MinHeight = 68, FontSize = 15, PlaceholderText = "在這裡打打看，例如「今天天氣很好」" };
        AutomationProperties.SetName(tryBox, "試打區，這裡的字不會被儲存");
        var tip = new TextBlock { Text = "這裡打的字不會被儲存。沒看到注音的話，按 Win＋空白鍵切換到善解。", Style = (Style)Application.Current.Resources["SjNote"] };
        // Two pairs a line, so the whole finish fits the window.
        var summary = new Grid { ColumnSpacing = 14, RowSpacing = 5 };
        foreach (var width in new[] { GridLength.Auto, new GridLength(1, GridUnitType.Star), GridLength.Auto, new GridLength(1, GridUnitType.Star) }) summary.ColumnDefinitions.Add(new ColumnDefinition { Width = width });
        int entries = 0;
        void Row(string name, string value)
        {
            int row = entries / 2, column = entries % 2 * 2; entries++;
            if (summary.RowDefinitions.Count <= row) summary.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var label = Line(name, 12, "SjText3Brush"); var shown = Line(value, 12, "SjText1Brush");
            Grid.SetRow(label, row); Grid.SetRow(shown, row); Grid.SetColumn(label, column); Grid.SetColumn(shown, column + 1);
            summary.Children.Add(label); summary.Children.Add(shown);
        }
        Row("打字方式", TemplateName);
        foreach (var q in questions) Row(q.Topic, q.Shown);
        var card = new Border { Background = Motion.Resource("SjFillControlBrush"), BorderBrush = Motion.Resource("SjStrokeBrush"), BorderThickness = new Thickness(1), CornerRadius = new CornerRadius(14), Padding = new Thickness(16, 12, 16, 12) };
        var inside = new StackPanel { Spacing = 8 }; inside.Children.Add(Line("你的選擇", 12, "SjText2Brush", FontWeights.SemiBold)); inside.Children.Add(summary);
        card.Child = inside;
        var later = new TextBlock { Text = "之後想改，到設定的「一般」按「重新開啟快速設定」，或在各分頁逐項調整。", Style = (Style)Application.Current.Resources["SjNote"] };
        var column = new StackPanel { Spacing = 10, MaxWidth = 520, Padding = new Thickness(0, 0, 0, 12), HorizontalAlignment = HorizontalAlignment.Center };
        var parts = new UIElement[] { heading, body, tryBox, tip, card, later };
        column.Children.Add(mark); foreach (var part in parts) column.Children.Add(part);
        var scroll = Scrolling(column); scroll.Padding = new Thickness(40, 0, 40, 0);
        return new Page { Root = scroll, Parts = [.. parts], Title = title, Mark = mark, Focus = tryBox, Scroll = scroll };
    }

    sealed class Question
    {
        internal required string Key, Topic, Eyebrow;
        internal required Func<string> Title, Lead;
        internal required Func<string[]> Options;
        internal required Func<int, string> Describe;
        internal required Func<int> Read;
        internal required Action<int> Write;
        internal required Func<DemoScript> Script;
        internal Func<string?> Note = () => null;
        internal Func<bool> Enabled = () => true;
        internal string Shown => Options()[Math.Clamp(Read(), 0, Options().Length - 1)];
    }
    string TemplateName => draft.Template switch { QuickTemplate.Shanjie => "善解", QuickTemplate.Windows => "微軟注音風格", _ => "你現在的設定" };
    /// Picking a template starts a new settings file from it, so an upgrader is told what stays and what goes back.
    Question TemplateQuestion()
    {
        QuickTemplate[] templates = draft.HadSettings ? [QuickTemplate.Shanjie, QuickTemplate.Windows, QuickTemplate.Keep] : [QuickTemplate.Shanjie, QuickTemplate.Windows];
        return new Question
        {
            Key = "template", Topic = "打字方式", Eyebrow = "打字方式",
            Title = () => "你習慣怎麼打字？",
            Lead = () => draft.HadSettings ? "換一種打字方式，或保留現在的設定。" : "先挑一種順手的，接下來每一步都還能微調。",
            Options = () => templates.Select(t => t switch { QuickTemplate.Shanjie => "善解", QuickTemplate.Windows => "微軟注音風格", _ => "保留我現在的設定" }).ToArray(),
            Describe = i => templates[i] switch
            {
                QuickTemplate.Shanjie => "和原版善解一樣：按空白打開選字，選好字還能接著打，打字時會預測下一個詞。",
                QuickTemplate.Windows => "按鍵跟 Windows 內建注音差不多：按 ↓ 打開選字，選好就送出，打字時不預測。",
                _ => "不套用範本，只改接下來你動到的項目。",
            },
            Read = () => Array.IndexOf(templates, draft.Template),
            Write = i => draft.Choose(templates[i]),
            Note = () => draft.HadSettings && draft.Template != QuickTemplate.Keep
                ? "選範本會換成一份新的設定：語言模型、詞包、個人詞典檔、鍵盤配置和聊天 App 清單會留著，外觀、字級、模糊音等其他調整回到範本的預設。原本的設定檔會先備份。"
                : "你的詞庫和學到的字都不會受影響。",
            Script = () => draft.Template == QuickTemplate.Keep ? QuickSetupDemos.Keep() : QuickSetupDemos.Template(draft.Template == QuickTemplate.Windows),
        };
    }
    void BuildQuestions() { questions.Clear(); questions.AddRange(Definitions()); }
    /// The words are checked against what each key does in `windows/src/tsf/keys.rs` and the help in SettingsViewModel.
    IEnumerable<Question> Definitions()
    {
        bool windows = draft.WindowsInteraction;
        SpaceChoice[] spaces = draft.Space == SpaceChoice.Custom ? [SpaceChoice.Insert, SpaceChoice.OpenList, SpaceChoice.Commit, SpaceChoice.Custom] : [SpaceChoice.Insert, SpaceChoice.OpenList, SpaceChoice.Commit];
        yield return new Question
        {
            Key = "space", Topic = "打完字按空白鍵", Eyebrow = "空白鍵",
            Title = () => "按空白鍵，要做什麼？",
            Lead = () => "字已經出現在句子裡時，按空白鍵會怎樣。注音還沒打完時，空白鍵都是第一聲。",
            Options = () => spaces.Select(s => s switch { SpaceChoice.Insert => "插入空格", SpaceChoice.OpenList => "打開選字", SpaceChoice.Commit => "直接送出", _ => "維持現在的" }).ToArray(),
            Describe = i => spaces[i] switch
            {
                SpaceChoice.Insert => "句子裡多一個空格，可以繼續打；要選字按 ↓。",
                SpaceChoice.OpenList => "打開選字清單，再按一次換下一頁。",
                SpaceChoice.Commit => draft.SpaceAfterCommit ? "字直接送到 App，再補一個空格。" : "字直接送到 App。",
                _ => "你現在的空白鍵是自訂組合，這裡不會改動。",
            },
            Read = () => Array.IndexOf(spaces, draft.Space),
            Write = index => draft.Space = spaces[index],
            Script = () => QuickSetupDemos.Space(draft.Space, draft.SpaceAfterCommit),
        };
        yield return new Question
        {
            Key = "right", Topic = windows ? "選字時按 Tab 或 →" : "選字時按 →", Eyebrow = windows ? "Tab 與 → 鍵" : "→ 鍵",
            Title = () => windows ? "選字時按 Tab 或 →，要做什麼？" : "選字時按 →，要做什麼？",
            Lead = () => "選字清單打開時，用它看下一個字或更多字。",
            Options = () => ["移到下一個", "翻下一頁", "展開大表"],
            Describe = i => i switch { 1 => "直接翻到下一頁。", 2 => "展開成三欄，一次看更多字。", _ => "往下一個字移動。" },
            Read = () => draft.RightAction,
            Write = index => draft.RightAction = index,
            Note = () => windows ? null : "Tab 在這個打字方式會送出字，再把 Tab 交給 App。",
            Script = () => QuickSetupDemos.Right(draft.RightAction, windows),
        };
        if (draft.IsZhuyin)
        {
            // Off is only ever the person's own earlier choice; it stays until another option is picked. Both modes at
            // once is shown as 原鍵混打, which decides first, and is left as it is unless another option is picked.
            MixChoice[] mixes = draft.SupportsAutomatic ? [MixChoice.Automatic, MixChoice.RawKeys] : [MixChoice.Off, MixChoice.RawKeys];
            // Without the automatic judgement the stored upstream flag does nothing, so it reads as off.
            MixChoice Shown() => draft.Mix switch { MixChoice.Both => MixChoice.RawKeys, MixChoice.Automatic when !draft.SupportsAutomatic => MixChoice.Off, var mix => mix };
            if (!mixes.Contains(Shown())) mixes = [.. mixes, Shown()];
            yield return new Question
            {
                Key = "mix", Topic = "中英文混著打", Eyebrow = "中英混打",
                Title = () => "中英文要怎麼混著打？",
                Lead = () => "打中文時夾幾個英文字，可以不必先切換。",
                Options = () => mixes.Select(m => m switch { MixChoice.Automatic => "自動判斷", MixChoice.RawKeys => SettingsViewModel.RawKeyMixingName, _ => draft.SupportsAutomatic ? "維持關閉" : "不混打" }).ToArray(),
                Describe = i => mixes[i] switch
                {
                    MixChoice.Automatic => "平常打注音；拼不成注音時，那段自動變回英文，馬上按 Esc 可以變回注音。",
                    MixChoice.RawKeys => "先顯示你按的字母，旁邊提示注音；按聲調變中文，按 Enter 留英文。會固定開啟組字面板。",
                    _ => draft.SupportsAutomatic ? "你現在關閉了混打，這裡不會改動。" : "照常打注音，要打英文先切換。",
                },
                Note = () => draft.SupportsAutomatic ? null : "目前的鍵盤配置用不到自動判斷，想混打請選" + SettingsViewModel.RawKeyMixingName + "。",
                Read = () => Array.IndexOf(mixes, Shown()),
                Write = index => { if (mixes[index] is MixChoice.Automatic or MixChoice.RawKeys) draft.Mix = mixes[index]; else if (!draft.SupportsAutomatic) draft.Mix = MixChoice.Automatic; },
                Script = () => Shown() switch { MixChoice.RawKeys => QuickSetupDemos.MixRawKeys(), MixChoice.Automatic => QuickSetupDemos.MixAutomatic(), _ => QuickSetupDemos.MixUnchanged() },
            };
        }
        yield return new Question
        {
            Key = "shift", Topic = "單按 Shift", Eyebrow = "Shift 鍵",
            Title = () => "要用 Shift 切換中英嗎？",
            Lead = () => "只按一下 Shift、沒有同時按其他鍵時才算。",
            Options = () => ["切換中英", "不切換"],
            Describe = i => i == 0 ? "單按一下就在中文、英文之間切換。" : "單按 Shift 不做任何事；要切換請按工作列的「中／A」。",
            Read = () => draft.ShiftToggle ? 0 : 1,
            Write = index => draft.ShiftToggle = index == 0,
            Script = () => QuickSetupDemos.Shift(draft.ShiftToggle),
        };
        yield return new Question
        {
            Key = "learning", Topic = "記住我選的字", Eyebrow = "選字學習",
            Title = () => "要記住你選的字嗎？",
            Lead = () => "記下來的只存在這台電腦。",
            Options = () => ["記住", "不記"],
            Describe = i => i == 0 ? "你改選過的字，下次遇到同樣的前文會優先出現。" : "每次都照內建的順序，不記你選過什麼。",
            Read = () => draft.Learning ? 0 : 1,
            Write = index => draft.Learning = index == 0,
            Script = () => QuickSetupDemos.Learning(draft.Learning, draft.EnterCommits),
        };
        yield return new Question
        {
            Key = "panel", Topic = "組字面板", Eyebrow = "組字面板",
            Title = () => "要顯示組字面板嗎？",
            Lead = () => "組字面板會在選字清單的位置多顯示一條整句。",
            Options = () => ["關閉", "開啟"],
            Describe = i => i == 0 ? "字就打在 App 裡，畫面最乾淨。" : "游標移進句子時，會標出游標所在的字；游標在句尾時不標。",
            Read = () => draft.PanelShown ? 1 : 0,
            Write = index => draft.PanelChoice = index == 1,
            Enabled = () => !draft.PanelForced,
            Note = () => draft.PanelForced ? $"{SettingsViewModel.RawKeyMixingName}要靠它提示注音，所以固定開啟；換回其他混打方式就恢復你原本的選擇。" : null,
            Script = () => QuickSetupDemos.Panel(draft.PanelShown, draft.AnchorRight),
        };
    }
}
