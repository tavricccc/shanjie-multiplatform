using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Windowing;
using Shanjie_UI.Interop;
using System.Runtime.InteropServices;
using CommunityToolkit.Mvvm.ComponentModel;
namespace Shanjie_UI;
public sealed class CandidateItem : ObservableObject
{
    public required string Label { get; init; }
    public required string Text { get; init; }
    public double FontSize { get; init; }
    public int Index { get; init; }
}
public sealed partial class CandidateWindow : Window
{
    private readonly CandidateChannel? channel;
    private readonly nint hwnd, previousProcedure;
    private readonly Native.WindowProcedure procedure;
    private Task? worker;
    private bool closing;
    private CandidateFrame? frame;
    private string[] words = [];
    private int fontSize;
    private bool associated;
    private double gridCellWidth, gridCellHeight;
    private readonly bool verification;
    public CandidateWindow(nint mapping, nint signal = 0)
    {
        InitializeComponent();
        verification = mapping == 0 || Environment.GetEnvironmentVariable("SHANJIE_VERIFY_UI") == "1";
        if (mapping != 0) channel = new(mapping, signal);
        hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        var presenter = OverlappedPresenter.CreateForToolWindow(); presenter.SetBorderAndTitleBar(false, false);
        presenter.IsResizable = presenter.IsMaximizable = presenter.IsMinimizable = false; presenter.IsAlwaysOnTop = true;
        AppWindow.SetPresenter(presenter); AppWindow.IsShownInSwitchers = false;
        Native.SetWindowLongPtr(hwnd, -16, Native.GetWindowLongPtr(hwnd, -16) & ~(nint)0x00CF0000); // Caption, resize frame and frame buttons.
        // The XAML surface owns the only border. The candidate is always instantaneous.
        int disabled = 1;
        Marshal.ThrowExceptionForHR(Native.DwmSetWindowAttribute(hwnd, 2, ref disabled, sizeof(int))); // DWMNCRP_DISABLED
        Marshal.ThrowExceptionForHR(Native.DwmSetWindowAttribute(hwnd, 3, ref disabled, sizeof(int))); // TRANSITIONS_FORCEDISABLED
        TableList.Loaded += (_, _) => { if (frame is { Visible: true }) Render(frame); };
        Native.SetWindowLongPtr(hwnd, -20, Native.GetWindowLongPtr(hwnd, -20) | 0x08000080);
        procedure = WindowProcedure;
        previousProcedure = Native.SetWindowLongPtr(hwnd, -4, Marshal.GetFunctionPointerForDelegate(procedure));
        Native.SetWindowPos(hwnd, 0, 0, 0, 0, 0, 0x37); // Recalculate the entire window as client content.
        if (channel != null) worker = Task.Run(() =>
        {
            try
            {
                while (channel.Wait())
                {
                    var next = channel.Read();
                    if (next != null) DispatcherQueue.TryEnqueue(() => Receive(next));
                }
                DispatcherQueue.TryEnqueue(() => { if (!closing) Close(); });
            }
            finally { channel.Dispose(); }
        });
        Closed += (_, _) => { closing = true; channel?.Stop(); };
    }
    private nint WindowProcedure(nint window, uint message, nuint wp, nint lp)
    {
        if (message == 0x21) return 3; // WM_MOUSEACTIVATE: never take keyboard focus.
        if (message == 0x83 && wp != 0) return 0; // WM_NCCALCSIZE: no second native frame.
        return Native.CallWindowProc(previousProcedure, window, message, wp, lp);
    }
    private void Receive(CandidateFrame next)
    {
        if (closing || (channel != null && !channel.IsCurrent(next.Sequence))) return;
        if (next.Shutdown) { Close(); return; }
        frame = next;
        Native.PostMessage(next.Callback, 0x8000 + 43, (nuint)hwnd, 0);
        if (!next.Visible || next.Words.Length == 0) { Native.ShowWindow(hwnd, 0); return; }
        Render(next);
    }
    private void Render(CandidateFrame next)
    {
        PaletteRoot.RequestedTheme = (ElementTheme)next.Theme;
        bool vertical = next.Layout == 0 && next.Columns == 0;
        VerticalList.Visibility = vertical ? Visibility.Visible : Visibility.Collapsed;
        TableList.Visibility = vertical ? Visibility.Collapsed : Visibility.Visible;
        if (!words.SequenceEqual(next.Words) || fontSize != next.FontSize || associated != next.Associated)
        {
            words = next.Words; fontSize = next.FontSize; associated = next.Associated;
            var items = words.Select((text, i) => new CandidateItem { Label = (associated ? "⇧" : "") + (i % 9 + 1).ToString(), Text = text, Index = i, FontSize = fontSize }).ToArray();
            VerticalList.ItemsSource = items; TableList.ItemsSource = items;
        }
        VerticalList.SelectedIndex = TableList.SelectedIndex = next.Selected;
        AutomationProperties.SetName(Surface, $"善解候選，第 {next.First + Math.Max(0, next.Selected) + 1} 個，共 {next.Total} 個" + (associated ? "，Shift＋數字選聯想詞" : ""));
        var caret = new Native.Rect { Left = next.Left, Right = next.Left + 1, Top = next.Top, Bottom = next.Bottom };
        var monitor = new Native.MonitorInfo { Size = Marshal.SizeOf<Native.MonitorInfo>() };
        Native.GetMonitorInfo(Native.MonitorFromRect(ref caret, 2), ref monitor);
        // Move to the target monitor before reading the host's DPI.
        Native.SetWindowPos(hwnd, -1, next.Left, next.Bottom + 4, 0, 0, 0x11);
        double scale = Native.GetDpiForWindow(hwnd) / 96.0;
        int columns = vertical ? 1 : Math.Min(9, words.Length);
        var textSizes = words.Select(text =>
        {
            var label = new TextBlock { Text = text, FontFamily = new Microsoft.UI.Xaml.Media.FontFamily("Microsoft JhengHei UI"), FontSize = fontSize };
            label.Measure(new Windows.Foundation.Size(double.PositiveInfinity, double.PositiveInfinity));
            return label.DesiredSize;
        }).ToArray();
        double textWidth = textSizes.Max(size => size.Width);
        double cell = Math.Max(vertical ? 96 : 52, textWidth + (associated ? 44 : 32));
        double rowHeight = Math.Max(32, textSizes.Max(size => size.Height) + 8);
        double workWidth = (monitor.Work.Right - monitor.Work.Left) / scale;
        cell = Math.Min(cell, (workWidth - 20) / columns);
        cell = Math.Ceiling(cell * scale) / scale;
        if (!vertical && (gridCellWidth != cell || gridCellHeight != rowHeight))
        {
            gridCellWidth = cell; gridCellHeight = rowHeight;
            var style = new Style(typeof(GridViewItem)) { BasedOn = (Style)PaletteRoot.Resources["CompactGridItem"] };
            style.Setters.Add(new Setter(FrameworkElement.WidthProperty, cell));
            style.Setters.Add(new Setter(FrameworkElement.HeightProperty, rowHeight));
            TableList.ItemContainerStyle = style;
        }
        TableList.UpdateLayout();
        if (!vertical && TableList.ItemsPanelRoot is ItemsWrapGrid panel)
        {
            panel.ItemWidth = cell; panel.ItemHeight = rowHeight;
            // Keep the TSF nine-column contract independent of ScrollViewer's viewport rounding.
            panel.Width = cell * columns + 1;
        }
        int width = (int)Math.Ceiling((cell * columns + 20) * scale);
        int height = (int)Math.Ceiling(((words.Length + columns - 1) / columns * rowHeight + 14) * scale);
        height = Math.Min(height, monitor.Work.Bottom - monitor.Work.Top);
        int x = Math.Clamp(next.Left, monitor.Work.Left, monitor.Work.Right - width);
        int y = next.Bottom + 4; if (y + height > monitor.Work.Bottom) y = next.Top - height - 4;
        y = Math.Max(y, monitor.Work.Top);
        AppWindow.MoveAndResize(new Windows.Graphics.RectInt32(verification ? -12000 : x, verification ? -12000 : y, width, height));
        if (channel == null || channel.IsCurrent(next.Sequence)) AppWindow.Show(false);
    }
    internal FrameworkElement CaptureElement => Surface;
    internal void ShowFrame(CandidateFrame next) { frame = next; Render(next); }
    private void Pick(object sender, ItemClickEventArgs args)
    {
        if (frame is { Visible: true } && args.ClickedItem is CandidateItem item)
            Native.PostMessage(frame.Callback, 0x8000 + 42, (nuint)item.Index, frame.Sequence);
    }
}
