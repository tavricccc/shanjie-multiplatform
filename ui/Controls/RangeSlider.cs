using System.Numerics;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Windows.System;
namespace Shanjie_UI.Controls;

/// <summary>Pane's slider: 4 px track filled with the accent up to the value, 16 px white knob that springs to 1.15 while held.</summary>
public sealed partial class RangeSlider : UserControl
{
    public static readonly DependencyProperty ValueProperty = DependencyProperty.Register(nameof(Value), typeof(double), typeof(RangeSlider), new PropertyMetadata(0.0, (d, _) => ((RangeSlider)d).Place()));
    public double Value { get => (double)GetValue(ValueProperty); set => SetValue(ValueProperty, value); }
    public double Minimum { get; set; }
    public double Maximum { get; set; } = 100;
    public double Step { get; set; } = 1;
    const double TrackWidth = 180, Thumb = 16;
    readonly Border fill = new() { Height = 4, CornerRadius = new CornerRadius(2), HorizontalAlignment = HorizontalAlignment.Left, VerticalAlignment = VerticalAlignment.Center };
    readonly Border thumb = new()
    {
        Width = Thumb, Height = Thumb, CornerRadius = new CornerRadius(Thumb / 2), HorizontalAlignment = HorizontalAlignment.Left, VerticalAlignment = VerticalAlignment.Center,
        Background = new SolidColorBrush(Microsoft.UI.Colors.White),
        BorderBrush = new SolidColorBrush(Windows.UI.Color.FromArgb(0x30, 0, 0, 0)), BorderThickness = new Thickness(0.5),
    };
    bool dragging;

    public RangeSlider()
    {
        IsTabStop = true;
        fill.Background = Motion.Accent;
        var track = new Border { Height = 4, CornerRadius = new CornerRadius(2), Background = Motion.Resource("PaneFillControlBrush"), BorderBrush = Motion.Resource("PaneStrokeBrush"), BorderThickness = new Thickness(1), VerticalAlignment = VerticalAlignment.Center };
        var root = new Grid { Width = TrackWidth, Height = 18, Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent) };
        root.Children.Add(track); root.Children.Add(fill); root.Children.Add(thumb);
        Content = root;
        FocusRing.Attach(this, root, 9);
        ElementCompositionPreview.GetElementVisual(thumb).CenterPoint = new Vector3((float)Thumb / 2, (float)Thumb / 2, 0);
        Place();
        root.PointerPressed += (_, e) => { if (!IsEnabled) return; dragging = true; root.CapturePointer(e.Pointer); Grow(true); Drag(e.GetCurrentPoint(root).Position.X); Focus(FocusState.Pointer); e.Handled = true; };
        root.PointerMoved += (_, e) => { if (dragging) Drag(e.GetCurrentPoint(root).Position.X); };
        root.PointerReleased += (_, e) => { if (!dragging) return; dragging = false; root.ReleasePointerCapture(e.Pointer); Grow(false); e.Handled = true; };
        root.PointerCaptureLost += (_, _) => { if (dragging) { dragging = false; Grow(false); } };
        KeyDown += (_, e) =>
        {
            int d = e.Key is VirtualKey.Left or VirtualKey.Down ? -1 : e.Key is VirtualKey.Right or VirtualKey.Up ? 1 : 0;
            if (d != 0) { Set(Value + d * Step); e.Handled = true; }
        };
        IsEnabledChanged += (_, _) => Opacity = IsEnabled ? 1 : 0.4;
    }

    void Drag(double x) => Set(Minimum + Math.Clamp((x - Thumb / 2) / (TrackWidth - Thumb), 0, 1) * (Maximum - Minimum));

    void Set(double v) => Value = Math.Clamp(Math.Round((v - Minimum) / Step) * Step + Minimum, Minimum, Maximum);

    void Place()
    {
        double p = Maximum > Minimum ? Math.Clamp((Value - Minimum) / (Maximum - Minimum), 0, 1) : 0;
        thumb.Margin = new Thickness(p * (TrackWidth - Thumb), 0, 0, 0);
        fill.Width = Math.Max(4, Thumb / 2 + p * (TrackWidth - Thumb));
        AutomationProperties.SetHelpText(this, $"{Value:0}");
    }

    void Grow(bool down)
    {
        var visual = ElementCompositionPreview.GetElementVisual(thumb); var c = visual.Compositor;
        var scale = c.CreateVector3KeyFrameAnimation();
        scale.InsertKeyFrame(1, down ? new Vector3(1.15f, 1.15f, 1) : Vector3.One, Motion.Ease(c, Motion.Spring));
        scale.Duration = Motion.Duration(Motion.Fast);
        visual.StartAnimation("Scale", scale);
    }
}
