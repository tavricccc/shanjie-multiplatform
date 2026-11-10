using System.Numerics;
using Microsoft.UI;
using Microsoft.UI.Composition;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Shanjie_UI.Setup;
using Windows.UI;
namespace Shanjie_UI.Controls;

/// <summary>
/// A small looping scene of typing: an App's text field with its composition and caret, the candidate window in the
/// palette and proportions of `windows/src/candidate.rs`, and the key just pressed. Plain XAML elements on a timer;
/// the movement between frames is composition animation: the window opens from the caret side as the real one does,
/// the selection glides, a new page settles in, the 中／A indicator pops and a commit lands in the text.
/// With Windows animations off it holds the script's key frame and does not tick.
/// </summary>
public sealed partial class TypingDemo : UserControl
{
    internal static bool Animated => Motion.Animated;
    const double ItemHeight = 24, WordSize = 14, LabelSize = 9;
    readonly TextBlock done = Text(), before = Text(true), after = Text(true), keyText = new() { FontSize = 12, FontWeight = FontWeights.SemiBold, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
    readonly TextBlock caption = new() { FontSize = 12, VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis };
    readonly Rectangle caret = new() { Width = 1.5, Height = 18, Margin = new Thickness(1, 0, 1, 0), VerticalAlignment = VerticalAlignment.Center };
    readonly Border field = new() { Height = 40, CornerRadius = new CornerRadius(8), BorderThickness = new Thickness(1), Padding = new Thickness(10, 0, 10, 0), VerticalAlignment = VerticalAlignment.Top };
    readonly Border keycap = new() { MinWidth = 30, Height = 24, CornerRadius = new CornerRadius(6), BorderThickness = new Thickness(1), Padding = new Thickness(8, 0, 8, 0) };
    readonly Canvas floating = new() { IsHitTestVisible = false };
    readonly Grid stage = new();
    readonly Border surface = new() { BorderThickness = new Thickness(0.5), Padding = new Thickness(3), Visibility = Visibility.Collapsed };
    readonly Border mode = new() { Width = 28, Height = 28, CornerRadius = new CornerRadius(8), BorderThickness = new Thickness(0.5), Visibility = Visibility.Collapsed };
    readonly TextBlock modeText = new() { FontSize = 13, FontWeight = FontWeights.SemiBold, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer timer;
    DemoScript? script;
    DemoFrame? previous;
    int index;
    string shownKey = "";

    public TypingDemo()
    {
        IsTabStop = false;
        timer = DispatcherQueue.CreateTimer(); timer.IsRepeating = false; timer.Tick += (_, _) => Tick();
        var line = new StackPanel { Orientation = Orientation.Horizontal, VerticalAlignment = VerticalAlignment.Center };
        line.Children.Add(done); line.Children.Add(before); line.Children.Add(caret); line.Children.Add(after);
        field.Child = line;
        mode.Child = modeText;
        floating.Children.Add(surface); floating.Children.Add(mode);
        stage.Children.Add(field); stage.Children.Add(floating);
        keycap.Child = keyText;
        var bar = new Grid { Margin = new Thickness(0, 8, 0, 0), MinHeight = 24 };
        bar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto }); bar.ColumnDefinitions.Add(new ColumnDefinition());
        bar.Children.Add(keycap); Grid.SetColumn(caption, 1); bar.Children.Add(caption);
        var layout = new Grid { Padding = new Thickness(12) };
        layout.RowDefinitions.Add(new RowDefinition()); layout.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        layout.Children.Add(stage); Grid.SetRow(bar, 1); layout.Children.Add(bar);
        Content = new Border { CornerRadius = new CornerRadius(12), BorderThickness = new Thickness(1), Child = layout, Background = Motion.Resource("SjFillControlBrush"), BorderBrush = Motion.Resource("SjStrokeBrush") };
        field.Background = Motion.Resource("SjFillSelectedBrush"); field.BorderBrush = Motion.Resource("SjStrokeBrush");
        keycap.Background = Motion.Resource("SjFillSelectedBrush"); keycap.BorderBrush = Motion.Resource("SjStrokeBrush");
        keyText.Foreground = Motion.Resource("SjText1Brush"); caption.Foreground = Motion.Resource("SjText2Brush");
        done.Foreground = before.Foreground = after.Foreground = Motion.Resource("SjText1Brush"); caret.Fill = Motion.Accent;
        AutomationProperties.SetAccessibilityView(stage, AccessibilityView.Raw);
        AutomationProperties.SetAccessibilityView(bar, AccessibilityView.Raw);
        stage.SizeChanged += (_, _) => Place();
        ActualThemeChanged += (_, _) => Render();
        Loaded += (_, _) => Start();
        Unloaded += (_, _) => timer.Stop();
    }
    static TextBlock Text(bool underline = false) => new() { FontSize = 15, VerticalAlignment = VerticalAlignment.Center, TextDecorations = underline ? Windows.UI.Text.TextDecorations.Underline : Windows.UI.Text.TextDecorations.None };

    internal void Play(DemoScript next)
    {
        bool replacing = script is not null && script != next;
        script = next;
        AutomationProperties.SetName(this, "示範動畫：" + next.Frames[next.Keyframe].Caption);
        Start();
        // Another choice: the scene settles in again rather than cutting.
        if (replacing && IsLoaded && Animated) Settle(stage, 10, Motion.Slow);
    }
    internal void Stop() => timer.Stop();
    void Start()
    {
        if (script is null || !IsLoaded) return;
        timer.Stop();
        index = Animated ? 0 : script.Keyframe;
        shownKey = ""; previous = null;
        Blink();
        Render();
        if (Animated) { timer.Interval = TimeSpan.FromMilliseconds(script.Frames[index].Ms); timer.Start(); }
    }
    void Tick()
    {
        if (script is null) return;
        // Turned off while running: settle on the key frame.
        if (!Animated) { Start(); return; }
        index = (index + 1) % script.Frames.Count;
        Render();
        timer.Interval = TimeSpan.FromMilliseconds(script.Frames[index].Ms);
        timer.Start();
    }
    internal int FrameForVerification => index;
    internal int FrameCountForVerification => script?.Frames.Count ?? 0;
    internal bool RunningForVerification => timer.IsRunning;
    internal string CaptionForVerification => caption.Text;
    /// Shows one frame and holds it, for a screenshot.
    internal void HoldForVerification(int frame) { timer.Stop(); index = frame; previous = null; Render(); }
    internal void RestartForVerification() => Start();
    /// The floating window stays inside the scene, above the key bar.
    internal bool FitsForVerification
    {
        get
        {
            Place();
            if (surface.Visibility != Visibility.Visible) return true;
            return Canvas.GetLeft(surface) + surface.DesiredSize.Width <= stage.ActualWidth + 0.5 && Canvas.GetTop(surface) + surface.DesiredSize.Height <= stage.ActualHeight + 0.5;
        }
    }

    (Color Background, Color Border, Color Text, Color Label, Color Number, Color Mark, Color Divider) Palette() => ActualTheme == ElementTheme.Dark
        ? (Argb(0.97, 0x1c1c20), Argb(0.08, 0xffffff), Argb(0.94, 0xffffff), Argb(0.64, 0xffffff), Argb(0.4, 0xffffff), Argb(0.3, 0x5874c9), Argb(0.08, 0xffffff))
        : (Argb(0.97, 0xf8f8fa), Argb(0.08, 0x000000), Argb(0.92, 0x141418), Argb(0.62, 0x141418), Argb(0.42, 0x141418), Argb(0.14, 0x5874c9), Argb(0.06, 0x000000));
    static Color Argb(double alpha, int rgb) => Color.FromArgb((byte)(alpha * 255), (byte)(rgb >> 16), (byte)(rgb >> 8), (byte)rgb);
    static SolidColorBrush Brush(Color color) => new(color);

    void Render()
    {
        if (script is null) return;
        var frame = script.Frames[index];
        // A trailing space has no width in a TextBlock.
        static string Shown(string text) => text.Replace(' ', ' ');
        int split = frame.Caret < 0 ? frame.Composing.Length : frame.Caret;
        done.Text = Shown(frame.Done);
        before.Text = Shown(frame.Composing[..split]);
        after.Text = Shown(frame.Composing[split..]);
        caption.Text = frame.Caption;
        keyText.Text = frame.Key;
        keycap.Visibility = frame.Key.Length == 0 ? Visibility.Collapsed : Visibility.Visible;
        caption.Margin = new Thickness(frame.Key.Length == 0 ? 0 : 8, 0, 0, 0);
        if (frame.Key.Length > 0) Press();
        shownKey = frame.Key;
        var palette = Palette();
        surface.Background = Brush(palette.Background); surface.BorderBrush = Brush(palette.Border);
        mode.Background = Brush(palette.Background); mode.BorderBrush = Brush(palette.Border); modeText.Foreground = Brush(palette.Text);
        var content = new StackPanel();
        if (frame.Panel is { } panel) content.Children.Add(PanelRow(frame, panel, palette.Label, palette.Mark));
        if (frame.Panel is not null && frame.List is not null) content.Children.Add(new Border { Height = 1, Margin = new Thickness(9, 1, 9, 1), Background = Brush(palette.Divider) });
        if (frame.List is { } list) content.Children.Add(ListView(list, palette.Text, palette.Number));
        surface.Child = content;
        surface.CornerRadius = new CornerRadius(14);
        surface.Visibility = frame.List is null && frame.Panel is null ? Visibility.Collapsed : Visibility.Visible;
        modeText.Text = frame.Mode ?? "";
        mode.Visibility = frame.Mode is null ? Visibility.Collapsed : Visibility.Visible;
        Place();
        Animate(previous, frame, content);
        previous = frame;
    }
    /// What changed since the last frame decides the movement; the first frame of a loop just appears.
    void Animate(DemoFrame? last, DemoFrame frame, StackPanel content)
    {
        if (!Animated || last is null) return;
        bool open = frame.List is not null || frame.Panel is not null, wasOpen = last.List is not null || last.Panel is not null;
        if (open && !wasOpen) Open();
        else if (open && frame.List is { } list && last.List is { } before)
        {
            if (list.Items.SequenceEqual(before.Items) && list.Layout == before.Layout && list.Selected != before.Selected && list.Layout == DemoLayout.Rows)
                Glide(content, list.Selected, before.Selected);
            else if (!list.Items.SequenceEqual(before.Items) || list.Layout != before.Layout) Settle(surface, 6, Motion.Normal);
        }
        if (frame.Mode is not null && frame.Mode != last.Mode) Pop(mode);
        // A commit: the text drops into the field.
        if (frame.Done.Length > last.Done.Length) Settle(done, -5, Motion.Normal);
    }
    /// Fades in and grows from the caret side over Fast, as `candidate.rs` opens the real window.
    void Open()
    {
        var v = ElementCompositionPreview.GetElementVisual(surface); var c = v.Compositor;
        v.CenterPoint = new Vector3(8, 0, 0);
        var grow = c.CreateVector3KeyFrameAnimation(); grow.InsertKeyFrame(0, new Vector3(0.92f, 0.92f, 1)); grow.InsertKeyFrame(1, Vector3.One, Motion.Ease(c, Motion.Smooth)); grow.Duration = Motion.Duration(Motion.Normal);
        var fade = c.CreateScalarKeyFrameAnimation(); fade.InsertKeyFrame(0, 0); fade.InsertKeyFrame(1, 1, Motion.Ease(c, Motion.Smooth)); fade.Duration = Motion.Duration(Motion.Fast);
        v.StartAnimation("Scale", grow); v.StartAnimation("Opacity", fade);
    }
    /// The selected pill slides from the previous item to the new one.
    static void Glide(StackPanel content, int selected, int before)
    {
        if (content.Children[^1] is not StackPanel rows || selected >= rows.Children.Count) return;
        var pill = rows.Children[selected];
        ElementCompositionPreview.SetIsTranslationEnabled(pill, true);
        var v = ElementCompositionPreview.GetElementVisual(pill); var c = v.Compositor;
        var move = c.CreateVector3KeyFrameAnimation();
        move.InsertKeyFrame(0, new Vector3(0, (float)((before - selected) * (ItemHeight + 2)), 0)); move.InsertKeyFrame(1, Vector3.Zero, Motion.Ease(c, Motion.Glide));
        move.Duration = Motion.Duration(Motion.Normal);
        v.StartAnimation("Translation", move);
    }
    static void Settle(UIElement element, float rise, double ms)
    {
        ElementCompositionPreview.SetIsTranslationEnabled(element, true);
        var v = ElementCompositionPreview.GetElementVisual(element); var c = v.Compositor;
        var move = c.CreateVector3KeyFrameAnimation(); move.InsertKeyFrame(0, new Vector3(0, rise, 0)); move.InsertKeyFrame(1, Vector3.Zero, Motion.Ease(c, Motion.Smooth)); move.Duration = Motion.Duration(ms);
        var fade = c.CreateScalarKeyFrameAnimation(); fade.InsertKeyFrame(0, 0.2f); fade.InsertKeyFrame(1, 1, Motion.Ease(c, Motion.Smooth)); fade.Duration = Motion.Duration(ms);
        v.StartAnimation("Translation", move); v.StartAnimation("Opacity", fade);
    }
    static void Pop(FrameworkElement element)
    {
        var v = ElementCompositionPreview.GetElementVisual(element); var c = v.Compositor;
        v.CenterPoint = new Vector3((float)element.Width / 2, (float)element.Height / 2, 0);
        var grow = c.CreateVector3KeyFrameAnimation(); grow.InsertKeyFrame(0, new Vector3(0.6f, 0.6f, 1)); grow.InsertKeyFrame(1, Vector3.One, Motion.Ease(c, Motion.Spring)); grow.Duration = Motion.Duration(Motion.Slow);
        var fade = c.CreateScalarKeyFrameAnimation(); fade.InsertKeyFrame(0, 0); fade.InsertKeyFrame(1, 1); fade.Duration = Motion.Duration(Motion.Fast);
        v.StartAnimation("Scale", grow); v.StartAnimation("Opacity", fade);
    }
    /// The real window's item height and label size, at a slightly smaller word size so nine fit.
    UIElement ListView(DemoList list, Color text, Color number)
    {
        var selectedText = Motion.OnAccent; var accent = Motion.Accent;
        Border Item(int i)
        {
            bool selected = i == list.Selected;
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 5, VerticalAlignment = VerticalAlignment.Center };
            row.Children.Add(new TextBlock { Text = (i + 1).ToString(), FontSize = LabelSize, VerticalAlignment = VerticalAlignment.Center, Foreground = selected ? selectedText : Brush(number) });
            row.Children.Add(new TextBlock { Text = list.Items[i], FontSize = WordSize, VerticalAlignment = VerticalAlignment.Center, Foreground = selected ? selectedText : Brush(text) });
            return new Border { Height = ItemHeight, CornerRadius = new CornerRadius(ItemHeight / 2), Padding = new Thickness(5, 0, 6, 0), Background = selected ? accent : Brush(Colors.Transparent), Child = row };
        }
        if (list.Layout == DemoLayout.Grid)
        {
            var grid = new Grid { RowSpacing = 2, ColumnSpacing = 2 };
            for (int c = 0; c < 3; c++) grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            for (int r = 0; r < (list.Items.Length + 2) / 3; r++) grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            for (int i = 0; i < list.Items.Length; i++) { var item = Item(i); Grid.SetColumn(item, i % 3); Grid.SetRow(item, i / 3); grid.Children.Add(item); }
            return grid;
        }
        var stack = new StackPanel { Spacing = 2 };
        for (int i = 0; i < list.Items.Length; i++) stack.Children.Add(Item(i));
        return stack;
    }
    /// The whole sentence with the marked part on a faint accent pill, as `candidate.rs` draws it.
    static UIElement PanelRow(DemoFrame frame, string text, Color label, Color mark)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(6, 2, 6, 2) };
        TextBlock Run(string part) => new() { Text = part, FontSize = WordSize, Foreground = Brush(label), VerticalAlignment = VerticalAlignment.Center };
        if (frame.MarkStart < 0 || frame.MarkLength == 0) { row.Children.Add(Run(text)); return row; }
        row.Children.Add(Run(text[..frame.MarkStart]));
        row.Children.Add(new Border { Background = Brush(mark), CornerRadius = new CornerRadius(6), Padding = new Thickness(3, 0, 3, 0), Margin = new Thickness(-3, 0, -3, 0), Child = Run(text.Substring(frame.MarkStart, frame.MarkLength)) });
        row.Children.Add(Run(text[(frame.MarkStart + frame.MarkLength)..]));
        return row;
    }
    /// Below the caret and inside the scene, as the real window keeps to the work area.
    void Place()
    {
        var unbounded = new Windows.Foundation.Size(double.PositiveInfinity, double.PositiveInfinity);
        done.Measure(unbounded); before.Measure(unbounded); surface.Measure(unbounded);
        double x = field.Padding.Left + done.DesiredSize.Width + before.DesiredSize.Width, top = field.Height + 6;
        Canvas.SetLeft(surface, Math.Max(0, Math.Min(x - 8, stage.ActualWidth - surface.DesiredSize.Width)));
        Canvas.SetTop(surface, top);
        Canvas.SetLeft(mode, Math.Max(0, Math.Min(x + 2, stage.ActualWidth - mode.Width)));
        Canvas.SetTop(mode, top);
    }
    void Press()
    {
        if (!Animated) return;
        var visual = ElementCompositionPreview.GetElementVisual(keycap);
        var c = visual.Compositor;
        visual.CenterPoint = new Vector3((float)keycap.ActualWidth / 2, (float)keycap.ActualHeight / 2, 0);
        var press = c.CreateVector3KeyFrameAnimation();
        press.InsertKeyFrame(0.35f, new Vector3(0.86f, 0.86f, 1), Motion.Ease(c, Motion.Smooth));
        press.InsertKeyFrame(1, Vector3.One, Motion.Ease(c, Motion.Spring));
        press.Duration = Motion.Duration(Motion.Normal);
        visual.StartAnimation("Scale", press);
    }
    void Blink()
    {
        var visual = ElementCompositionPreview.GetElementVisual(caret);
        var c = visual.Compositor;
        if (!Animated) { visual.StopAnimation("Opacity"); visual.Opacity = 1; return; }
        var blink = c.CreateScalarKeyFrameAnimation();
        blink.InsertKeyFrame(0, 1); blink.InsertKeyFrame(0.5f, 1); blink.InsertKeyFrame(0.51f, 0); blink.InsertKeyFrame(1, 0);
        blink.Duration = TimeSpan.FromMilliseconds(1060); blink.IterationBehavior = AnimationIterationBehavior.Forever;
        visual.StartAnimation("Opacity", blink);
    }
}
