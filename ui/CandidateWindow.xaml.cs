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
    private readonly Microsoft.UI.Dispatching.DispatcherQueueTimer timer;
    private CandidateFrame? frame;
    private string[] words = [];
    private int fontSize;
    private readonly bool verification;
    public CandidateWindow(nint mapping)
    {
        InitializeComponent();
        verification = mapping == 0;
        if (mapping != 0) channel = new(mapping);
        hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        var presenter = OverlappedPresenter.CreateForContextMenu(); presenter.SetBorderAndTitleBar(false, false);
        presenter.IsResizable = presenter.IsMaximizable = presenter.IsMinimizable = false; presenter.IsAlwaysOnTop = true;
        AppWindow.SetPresenter(presenter); AppWindow.IsShownInSwitchers = false;
        TableList.Loaded += (_, _) => { if (frame is { Visible: true }) Render(frame); };
        Native.SetWindowLongPtr(hwnd, -20, Native.GetWindowLongPtr(hwnd, -20) | 0x08000080);
        procedure = WindowProcedure;
        previousProcedure = Native.SetWindowLongPtr(hwnd, -4, Marshal.GetFunctionPointerForDelegate(procedure));
        timer = DispatcherQueue.CreateTimer(); timer.Interval = TimeSpan.FromMilliseconds(16); timer.Tick += Tick;
        if (channel != null) timer.Start();
        Closed += (_, _) => { timer.Stop(); channel?.Dispose(); };
    }
    private nint WindowProcedure(nint window, uint message, nuint wp, nint lp) => message == 0x21 ? 3 : Native.CallWindowProc(previousProcedure, window, message, wp, lp);
    private void Tick(Microsoft.UI.Dispatching.DispatcherQueueTimer sender, object args)
    {
        if (channel == null) return;
        if (channel.ParentExited) { Close(); return; }
        var next = channel.Read(); if (next is null) return;
        if (next.Shutdown) { Close(); return; }
        frame = next;
        if (!next.Visible || next.Words.Length == 0) { Native.ShowWindow(hwnd, 0); return; }
        Render(next);
    }
    private void Render(CandidateFrame next)
    {
        Surface.RequestedTheme = (ElementTheme)next.Theme;
        bool vertical = next.Layout == 0 && next.Columns == 0;
        VerticalList.Visibility = vertical ? Visibility.Visible : Visibility.Collapsed;
        TableList.Visibility = vertical ? Visibility.Collapsed : Visibility.Visible;
        if (!words.SequenceEqual(next.Words) || fontSize != next.FontSize)
        {
            words = next.Words; fontSize = next.FontSize;
            var items = words.Select((text, i) => new CandidateItem { Label = (i % 9 + 1).ToString(), Text = text, Index = i, FontSize = fontSize }).ToArray();
            VerticalList.ItemsSource = items; TableList.ItemsSource = items;
        }
        VerticalList.SelectedIndex = TableList.SelectedIndex = next.Selected;
        PageStatus.Text = $"{next.First + Math.Max(0, next.Selected) + 1} / {next.Total}　空白換頁";
        AutomationProperties.SetName(Surface, "善解候選，" + PageStatus.Text);
        var caret = new Native.Rect { Left = next.Left, Right = next.Left + 1, Top = next.Top, Bottom = next.Bottom };
        var monitor = new Native.MonitorInfo { Size = Marshal.SizeOf<Native.MonitorInfo>() };
        Native.GetMonitorInfo(Native.MonitorFromRect(ref caret, 2), ref monitor);
        // Move to the target monitor before reading the host's DPI.
        Native.SetWindowPos(hwnd, -1, next.Left, next.Bottom + 4, 0, 0, 0x11);
        double scale = Native.GetDpiForWindow(hwnd) / 96.0;
        int columns = vertical ? 1 : Math.Min(9, words.Length);
        double cell = Math.Max(vertical ? 180 : 76, words.Max(text => text.Length) * fontSize + 56);
        double workWidth = (monitor.Work.Right - monitor.Work.Left) / scale;
        cell = Math.Min(cell, (workWidth - 20) / columns);
        if (!vertical && TableList.ItemsPanelRoot is ItemsWrapGrid panel) { panel.ItemWidth = cell; panel.ItemHeight = fontSize + 28; }
        int width = (int)Math.Ceiling((cell * columns + 18) * scale);
        int height = (int)Math.Ceiling(((words.Length + columns - 1) / columns * (fontSize + 28) + 46) * scale);
        height = Math.Min(height, monitor.Work.Bottom - monitor.Work.Top);
        int x = Math.Clamp(next.Left, monitor.Work.Left, monitor.Work.Right - width);
        int y = next.Bottom + 4; if (y + height > monitor.Work.Bottom) y = next.Top - height - 4;
        y = Math.Max(y, monitor.Work.Top);
        Native.SetWindowPos(hwnd, -1, verification ? -12000 : x, verification ? -12000 : y, width, height, 0x50);
    }
    internal FrameworkElement CaptureElement => Surface;
    internal void ShowFrame(CandidateFrame next) { frame = next; Render(next); }
    private void Pick(object sender, ItemClickEventArgs args)
    {
        if (frame is { Visible: true } && args.ClickedItem is CandidateItem item)
            Native.PostMessage(frame.Callback, 0x8000 + 42, (nuint)item.Index, frame.Sequence);
    }
}
