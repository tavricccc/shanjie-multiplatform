using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Shanjie_UI.Models;
namespace Shanjie_UI.Controls;

public sealed partial class ContributorsView : UserControl
{
    internal readonly List<(HyperlinkButton Link, Flyout Preview)> Profiles = [];
    private readonly DispatcherTimer hover = new() { Interval = TimeSpan.FromMilliseconds(200) };
    private readonly DispatcherTimer dismiss = new() { Interval = TimeSpan.FromMilliseconds(180) };
    private (HyperlinkButton Link, Flyout Preview)? pending;

    public ContributorsView()
    {
        dismiss.Tick += (_, _) => ClosePreviews();
        hover.Tick += (_, _) => {
            hover.Stop();
            if (pending is { } target) target.Preview.ShowAt(target.Link, new FlyoutShowOptions
                { Placement = FlyoutPlacementMode.Right, ShowMode = FlyoutShowMode.Transient });
            pending = null;
        };
        var body = (Style)Application.Current.Resources["SjBody"];
        var note = (Style)Application.Current.Resources["SjNote"];
        var list = new StackPanel { Spacing = 2 };
        foreach (var person in Contributors.Catalog.Contributors)
        {
            var name = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            name.Children.Add(new GitHubMark { Width = 16, Height = 16, VerticalAlignment = VerticalAlignment.Center });
            name.Children.Add(new TextBlock { Text = person.Name, FontSize = 14, VerticalAlignment = VerticalAlignment.Center });
            name.Children.Add(new TextBlock { Text = $"@{person.Login}", Style = body, VerticalAlignment = VerticalAlignment.Center });
            var link = new HyperlinkButton { Content = name, NavigateUri = person.Profile, Padding = new Thickness(4, 8, 4, 8), HorizontalAlignment = HorizontalAlignment.Left };
            AutomationProperties.SetName(link, $"{person.Name}，GitHub @{person.Login}");
            AutomationProperties.SetHelpText(link, person.Summary);

            var details = new StackPanel { Width = 300, Spacing = 12 };
            var identity = new Grid { ColumnSpacing = 12 };
            identity.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            identity.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            var portrait = new BitmapImage();
            var portraitBrush = new ImageBrush { ImageSource = portrait, Stretch = Stretch.UniformToFill };
            portrait.ImageFailed += (_, _) => portraitBrush.ImageSource = new BitmapImage(person.Portrait);
            identity.Children.Add(new Microsoft.UI.Xaml.Shapes.Ellipse { Width = 56, Height = 56, Fill = portraitBrush });
            var caption = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
            caption.Children.Add(new TextBlock { Text = person.Name, FontSize = 16, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
            caption.Children.Add(new TextBlock { Text = $"@{person.Login} · {person.Contributions:N0} 次提交", Style = body });
            Grid.SetColumn(caption, 1); identity.Children.Add(caption);
            details.Children.Add(identity);
            details.Children.Add(new TextBlock { Text = person.Summary, Style = body });
            var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 16 };
            actions.Children.Add(new HyperlinkButton { Content = "GitHub 個人頁面 ↗", NavigateUri = person.Profile, Padding = new Thickness(0) });
            actions.Children.Add(new HyperlinkButton { Content = "查看提交 ↗", NavigateUri = person.Commits, Padding = new Thickness(0) });
            details.Children.Add(actions);
            var preview = new Flyout { Content = details, SystemBackdrop = new DesktopAcrylicBackdrop(), ShouldConstrainToRootBounds = false, AreOpenCloseAnimationsEnabled = false,
                FlyoutPresenterStyle = (Style)Application.Current.Resources["SjFlyoutPresenter"], OverlayInputPassThroughElement = this };
            preview.Opening += (_, _) => portrait.UriSource ??= new Uri(person.AvatarSource);
            preview.Opened += (_, _) => dismiss.Stop();
            // The automatic move-away mode closes before Opened in the desktop XAML island.
            // Leave a short gap for moving from the link into the card instead.
            link.PointerEntered += (_, _) => {
                dismiss.Stop();
                if (preview.IsOpen) return;
                pending = (link, preview); hover.Stop(); hover.Start();
            };
            link.PointerExited += (_, _) => {
                if (pending?.Preview == preview) { hover.Stop(); pending = null; }
                dismiss.Start();
            };
            details.PointerEntered += (_, _) => dismiss.Stop();
            details.PointerExited += (_, _) => dismiss.Start();
            link.ContextFlyout = preview;
            Profiles.Add((link, preview));

            var row = new Grid { ColumnSpacing = 8 };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.Children.Add(link);
            var count = new TextBlock { Text = $"{person.Contributions:N0} 次提交", Style = body, VerticalAlignment = VerticalAlignment.Center };
            Grid.SetColumn(count, 1); row.Children.Add(count);
            list.Children.Add(row);
        }
        list.Children.Add(new TextBlock { Text = $"依 GitHub 提交數排序 · 統計至 {Contributors.Catalog.UpdatedAt.Replace('-', '/')}", Style = note });
        Content = list;
        Unloaded += (_, _) => ClosePreviews();
    }

    internal void ClosePreviews() { hover.Stop(); pending = null; dismiss.Stop(); foreach (var profile in Profiles) profile.Preview.Hide(); }
}
