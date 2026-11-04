using System.Numerics;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Windows.System;
namespace Shanjie_UI.Controls;

/// <summary>
/// Switch: 38 × 22 track, 16 px white knob that springs across; held down, the knob stretches to 21 px.
/// The accent shows only on the track when on.
/// </summary>
public sealed partial class Switch : UserControl
{
    public static readonly DependencyProperty IsOnProperty = DependencyProperty.Register(nameof(IsOn), typeof(bool), typeof(Switch), new PropertyMetadata(false, (d, _) => ((Switch)d).Animate()));
    public bool IsOn { get => (bool)GetValue(IsOnProperty); set => SetValue(IsOnProperty, value); }
    readonly Border on = new() { Width = 38, Height = 22, CornerRadius = new CornerRadius(11) };
    readonly Border knob = new()
    {
        Width = 16, Height = 16, CornerRadius = new CornerRadius(8), Margin = new Thickness(3, 0, 0, 0),
        HorizontalAlignment = HorizontalAlignment.Left, VerticalAlignment = VerticalAlignment.Center,
        Background = new SolidColorBrush(Microsoft.UI.Colors.White),
        BorderBrush = new SolidColorBrush(Windows.UI.Color.FromArgb(0x30, 0, 0, 0)), BorderThickness = new Thickness(0.5),
    };
    readonly Visual knobVisual, onVisual;
    bool pressed, placed;

    public Switch()
    {
        IsTabStop = true;
        var track = new Border { Width = 38, Height = 22, CornerRadius = new CornerRadius(11), BorderThickness = new Thickness(1), Background = Motion.Resource("SjFillControlBrush"), BorderBrush = Motion.Resource("SjStrokeBrush") };
        on.Background = Motion.Accent;
        var root = new Grid { Width = 38, Height = 22, Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent) };
        root.Children.Add(track); root.Children.Add(on); root.Children.Add(knob);
        Content = root;
        FocusRing.Attach(this, root, 11);
        ElementCompositionPreview.SetIsTranslationEnabled(knob, true);
        knobVisual = ElementCompositionPreview.GetElementVisual(knob);
        onVisual = ElementCompositionPreview.GetElementVisual(on);
        Loaded += (_, _) => { if (!placed) Place(); };
        PointerPressed += (_, e) => { if (!IsEnabled) return; pressed = true; CapturePointer(e.Pointer); Stretch(true); e.Handled = true; };
        PointerReleased += (_, e) => { if (!pressed) return; pressed = false; ReleasePointerCapture(e.Pointer); Stretch(false); IsOn = !IsOn; e.Handled = true; };
        PointerCaptureLost += (_, _) => { if (pressed) { pressed = false; Stretch(false); } };
        KeyDown += (_, e) => { if (e.Key is VirtualKey.Space or VirtualKey.Enter) { IsOn = !IsOn; e.Handled = true; } };
        IsEnabledChanged += (_, _) => Opacity = IsEnabled ? 1 : 0.4;
    }

    void Place()
    {
        placed = true;
        knobVisual.Properties.InsertVector3("Translation", new Vector3(IsOn ? 16 : 0, 0, 0));
        onVisual.Opacity = IsOn ? 1 : 0;
    }

    void Animate()
    {
        (FrameworkElementAutomationPeer.FromElement(this) as SwitchPeer)?.Changed(!IsOn, IsOn);
        if (!placed || !IsLoaded) { Place(); return; }
        var c = knobVisual.Compositor;
        var move = c.CreateVector3KeyFrameAnimation();
        move.InsertKeyFrame(1, new Vector3(IsOn ? (pressed ? 11 : 16) : 0, 0, 0), Motion.Ease(c, Motion.Spring));
        move.Duration = Motion.Duration(Motion.Normal);
        knobVisual.StartAnimation("Translation", move);
        var fade = c.CreateScalarKeyFrameAnimation();
        fade.InsertKeyFrame(1, IsOn ? 1 : 0, Motion.Ease(c, Motion.Smooth));
        fade.Duration = Motion.Duration(Motion.Fast);
        onVisual.StartAnimation("Opacity", fade);
    }

    // Held down the knob grows toward the side it will travel: right when off, left when on.
    void Stretch(bool down)
    {
        var c = knobVisual.Compositor;
        knobVisual.CenterPoint = new Vector3(IsOn ? 16 : 0, 8, 0);
        var scale = c.CreateVector3KeyFrameAnimation();
        scale.InsertKeyFrame(1, new Vector3(down ? 21f / 16 : 1, 1, 1), Motion.Ease(c, Motion.Smooth));
        scale.Duration = Motion.Duration(Motion.Fast);
        knobVisual.StartAnimation("Scale", scale);
    }

    protected override AutomationPeer OnCreateAutomationPeer() => new SwitchPeer(this);

    sealed partial class SwitchPeer(Switch owner) : FrameworkElementAutomationPeer(owner), Microsoft.UI.Xaml.Automation.Provider.IToggleProvider
    {
        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.Button;
        protected override object GetPatternCore(PatternInterface pattern) => pattern == PatternInterface.Toggle ? this : base.GetPatternCore(pattern);
        public ToggleState ToggleState => owner.IsOn ? ToggleState.On : ToggleState.Off;
        public void Toggle() { if (owner.IsEnabled) owner.IsOn = !owner.IsOn; }
        internal void Changed(bool from, bool to) => RaisePropertyChangedEvent(TogglePatternIdentifiers.ToggleStateProperty, from ? ToggleState.On : ToggleState.Off, to ? ToggleState.On : ToggleState.Off);
    }
}
