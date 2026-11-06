namespace Shanjie_UI.Setup;

internal enum DemoLayout { Horizontal, Vertical, Grid }
internal sealed record DemoList(string[] Items, int Selected, DemoLayout Layout);
/// <summary>One moment of a demo: what the App and the candidate window show after a key.</summary>
internal sealed record DemoFrame(string Key, string Caption, int Ms = 1100)
{
    internal string Done { get; init; } = "";
    internal string Composing { get; init; } = "";
    /// Characters of the composition before the caret; -1 is its end.
    internal int Caret { get; init; } = -1;
    internal DemoList? List { get; init; }
    /// The reading panel's row and the marked part of it (faint accent pill, `candidate.rs`).
    internal string? Panel { get; init; }
    internal int MarkStart { get; init; } = -1;
    internal int MarkLength { get; init; }
    /// The 中／A indicator beside the caret.
    internal string? Mode { get; init; }
}
/// <param name="Keyframe">The frame that stands for the whole demo when Windows animations are off.</param>
internal sealed record DemoScript(IReadOnlyList<DemoFrame> Frames, int Keyframe);

/// <summary>
/// The demos. Every composition and candidate here was read from the real core on the standard layout (`s u 3 c l 3`
/// composes 你好 and opens 你好、妳好、好、郝; `g 4` opens 是…識, then 試…誓; `h e l l o` goes ㄘ, he, hel, hell, hello), and the
/// keys follow `windows/src/tsf/keys.rs`, `tests/windows_interaction.cpp` and docs/MICROSOFT-IME-DIFFS.md. Lists are cut to
/// fit. The keycaps show Zhuyin so they hold for both layouts; the mixing demos show letters because letters are the point.
/// </summary>
internal static class QuickSetupDemos
{
    static readonly string[] Nihao = ["你好", "妳好", "好"];
    static readonly string[] Shi1 = ["是", "事", "市", "式", "示", "視", "世", "士", "識"], Shi2 = ["試", "勢", "適", "室", "釋", "氏", "飾", "逝", "誓"];
    const string Sentence = "今天天氣很好";
    static DemoFrame Stroke(string key, string composing) => new(key, "打注音", 240) { Composing = composing };
    static List<DemoFrame> TypeNihao() => [Stroke("ㄋ", "ㄋ"), Stroke("ㄧ", "ㄋㄧ"), Stroke("ˇ", "你"), Stroke("ㄏ", "你ㄏ"), Stroke("ㄠ", "你ㄏㄠ"), Stroke("ˇ", "你好")];
    static List<DemoFrame> TypeShi() => [Stroke("ㄕ", "ㄕ"), Stroke("ˋ", "是")];
    static DemoLayout Layout(bool vertical) => vertical ? DemoLayout.Vertical : DemoLayout.Horizontal;
    static string[] Fit(string[] items, bool vertical) => vertical ? items[..5] : items[..7];

    /// 善解: Space opens, → steps, Enter keeps composing. 微軟注音風格: ↓ opens and steps, Enter sends.
    internal static DemoScript Template(bool windows)
    {
        var frames = TypeNihao(); int key = frames.Count;
        DemoList List(int selected) => new(Nihao, selected, Layout(windows));
        if (windows) frames.AddRange([
            new("↓", "按 ↓ 打開選字") { Composing = "你好", List = List(0) },
            new("↓", "往下選") { Composing = "你好", List = List(1) },
            new("Enter", "選好就直接送出", 1800) { Done = "妳好" }]);
        else frames.AddRange([
            new("空白", "按空白打開選字") { Composing = "你好", List = List(0) },
            new("→", "移到想要的字") { Composing = "你好", List = List(1) },
            new("Enter", "選好了，還能繼續打") { Composing = "妳好" },
            new("Enter", "再按一次才送出", 1800) { Done = "妳好" }]);
        return new(frames, key);
    }
    internal static DemoScript Space(SpaceChoice choice, bool vertical, bool spaceAfterCommit)
    {
        switch (choice)
        {
            case SpaceChoice.OpenList:
            {
                var frames = TypeShi(); int key = frames.Count;
                frames.Add(new("空白", "按空白打開選字") { Composing = "是", List = new(Fit(Shi1, vertical), 0, Layout(vertical)) });
                frames.Add(new("空白", "再按一次換下一頁", 1800) { Composing = "是", List = new(Fit(Shi2, vertical), 0, Layout(vertical)) });
                return new(frames, key);
            }
            case SpaceChoice.Commit:
            {
                var frames = TypeNihao(); int key = frames.Count;
                frames.Add(new("空白", spaceAfterCommit ? "字送出去，再補一個空格" : "字直接送出去", 2200) { Done = "你好" + (spaceAfterCommit ? " " : "") });
                return new(frames, key);
            }
            case SpaceChoice.Custom: return new([new("", "維持你現在的空白鍵設定", 2000)], 0);
            default:
            {
                var frames = TypeNihao(); int key = frames.Count;
                frames.Add(new("空白", "多一個空格，還能繼續打", 2200) { Composing = "你好 " });
                return new(frames, key);
            }
        }
    }
    internal static DemoScript Direction(bool vertical)
    {
        var frames = TypeShi(); int key = frames.Count;
        frames.Add(new("↓", vertical ? "清單直著排" : "清單橫著排") { Composing = "是", List = new(Fit(Shi1, vertical), 0, Layout(vertical)) });
        frames.Add(new("↓", "往下一個", 1800) { Composing = "是", List = new(Fit(Shi1, vertical), 1, Layout(vertical)) });
        return new(frames, key);
    }
    /// The action's key: Tab shares it in the Windows interaction only (`input::right_action_key`).
    internal static DemoScript Right(int action, bool windows, bool vertical)
    {
        string key = windows ? "Tab" : "→";
        var frames = TypeShi();
        frames.Add(new("↓", "打開選字") { Composing = "是", List = new(Fit(Shi1, vertical), 0, Layout(vertical)) });
        int first = frames.Count;
        switch (action)
        {
            case 1: frames.Add(new(key, "翻到下一頁", 2000) { Composing = "是", List = new(Fit(Shi2, vertical), 0, Layout(vertical)) }); break;
            case 2: frames.Add(new(key, "展開三欄，一次看更多", 2200) { Composing = "是", List = new(Shi1, 0, DemoLayout.Grid) }); break;
            default:
                frames.Add(new(key, "移到下一個") { Composing = "是", List = new(Fit(Shi1, vertical), 1, Layout(vertical)) });
                frames.Add(new(key, "再下一個", 1600) { Composing = "是", List = new(Fit(Shi1, vertical), 2, Layout(vertical)) });
                break;
        }
        return new(frames, first);
    }
    /// The upstream judgement: `h` is still ㄘ, `e` cannot follow it, and the run turns back into its keys.
    internal static DemoScript MixAutomatic() => new(new DemoFrame[] {
        new("h", "照常打注音", 800) { Composing = "ㄘ" },
        new("e", "拼不成注音，自動變英文", 1600) { Composing = "he" },
        new("l", "繼續打英文", 300) { Composing = "hel" },
        new("l", "繼續打英文", 300) { Composing = "hell" },
        new("o", "繼續打英文", 700) { Composing = "hello" },
        new("Enter", "送出英文", 1800) { Done = "hello" } }, 1);
    /// Raw-key mixing (`tests/windows_interaction.cpp`): `su` shows ㄋㄧ, 3 makes 你, `gj` shows ㄕㄨ and Enter keeps the letters.
    internal static DemoScript MixRawKeys() => new(new DemoFrame[] {
        new("s", "先顯示字母，旁邊提示注音", 700) { Composing = "s", Panel = "ㄋ", MarkStart = 0, MarkLength = 1 },
        new("u", "先顯示字母，旁邊提示注音", 1300) { Composing = "su", Panel = "ㄋㄧ", MarkStart = 0, MarkLength = 2 },
        new("ˇ", "按聲調就變中文") { Composing = "你", Panel = "你" },
        new("Enter", "送出") { Done = "你" },
        new("g", "再打一段", 500) { Done = "你", Composing = "g", Panel = "ㄕ", MarkStart = 0, MarkLength = 1 },
        new("j", "再打一段", 1000) { Done = "你", Composing = "gj", Panel = "ㄕㄨ", MarkStart = 0, MarkLength = 2 },
        new("Enter", "按 Enter 就留下英文", 1800) { Done = "你gj" } }, 1);
    internal static DemoScript MixUnchanged() => new([new("", "維持你現在的混打設定", 2000)], 0);
    internal static DemoScript Shift(bool on) => on
        ? new(new DemoFrame[] {
            new("Shift", "單按一下，切到英文") { Mode = "A" },
            new("h", "打出來就是英文", 400) { Done = "h" },
            new("i", "打出來就是英文", 900) { Done = "hi" },
            new("Shift", "再按一下，切回中文") { Done = "hi", Mode = "中" },
            new("ㄋ", "繼續打注音", 1600) { Done = "hi", Composing = "ㄋ" } }, 0)
        : new(new DemoFrame[] {
            new("Shift", "單按沒有反應，還是中文", 1600),
            new("ㄋ", "繼續打注音", 1600) { Composing = "ㄋ" } }, 0);
    /// `enter_commits` decides whether one Enter or two send it. The word chosen before comes back under the same context (upstream S4 learning).
    internal static DemoScript Learning(bool on, bool enterCommits, bool vertical)
    {
        var frames = TypeNihao();
        frames.Add(new("↓", "打開選字") { Composing = "你好", List = new(Nihao, 0, Layout(vertical)) });
        frames.Add(new("↓", "改選「妳好」") { Composing = "你好", List = new(Nihao, 1, Layout(vertical)) });
        if (!enterCommits) frames.Add(new("Enter", "選好了", 700) { Composing = "妳好" });
        frames.Add(new("Enter", "送出", 900) { Done = "妳好" });
        frames.Add(new("", "下次打同樣的字…", 800));
        int key = frames.Count;
        frames.Add(new("", on ? "記住了，先給你「妳好」" : "不記，還是「你好」", 2200) { Composing = on ? "妳好" : "你好" });
        return new(frames, key);
    }
    /// `candidate.rs`: the panel row marks the character at the cursor (right of it under anchor=right, left of it otherwise) and nothing at the end.
    internal static DemoScript Panel(bool shown, bool anchorRight)
    {
        if (!shown) return new(new DemoFrame[] {
            new("", "字就打在 App 裡，沒有多的視窗", 1600) { Composing = Sentence },
            new("←", "移動游標也一樣", 1000) { Composing = Sentence, Caret = 5 },
            new("←", "移動游標也一樣", 1800) { Composing = Sentence, Caret = 4 } }, 2);
        int Mark(int caret) => anchorRight ? caret : caret - 1;
        return new(new DemoFrame[] {
            new("", "游標在句尾，不標字", 1800) { Composing = Sentence, Panel = Sentence },
            new("←", "游標移進句子，標出那個字", 1300) { Composing = Sentence, Caret = 5, Panel = Sentence, MarkStart = Mark(5), MarkLength = 1 },
            new("←", "游標移進句子，標出那個字", 1300) { Composing = Sentence, Caret = 4, Panel = Sentence, MarkStart = Mark(4), MarkLength = 1 },
            new("←", "游標移進句子，標出那個字", 2000) { Composing = Sentence, Caret = 3, Panel = Sentence, MarkStart = Mark(3), MarkLength = 1 } }, 2);
    }
}
