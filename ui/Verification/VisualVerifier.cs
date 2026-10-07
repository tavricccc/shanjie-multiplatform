using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;
using Shanjie_UI.Interop;
using Windows.Graphics;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;
namespace Shanjie_UI.Verification;
internal static class VisualVerifier
{
    private static async Task Capture(FrameworkElement element, string path)
    {
        element.UpdateLayout();
        var bitmap = new RenderTargetBitmap(); await bitmap.RenderAsync(element);
        var pixels = await bitmap.GetPixelsAsync();
        using var stream = new InMemoryRandomAccessStream();
        var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, stream);
        using var reader = DataReader.FromBuffer(pixels); byte[] bytes = new byte[pixels.Length]; reader.ReadBytes(bytes);
        encoder.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied, (uint)bitmap.PixelWidth, (uint)bitmap.PixelHeight, 96, 96, bytes);
        await encoder.FlushAsync(); stream.Seek(0);
        using var output = File.Create(path); await stream.AsStreamForRead().CopyToAsync(output);
    }
    public static async Task Run(MainWindow settings, string output)
    {
        // Keep verification away from the user's windows and never activate the test surfaces.
        settings.AppWindow.Move(new PointInt32(-12000, -12000)); settings.AppWindow.Show(false);
        await Task.Delay(500);
        settings.Vm.Preset(true); settings.Vm.Save(); settings.Vm.Load();
        if (settings.Vm.Interaction != 0 || settings.Vm.Anchor != 0 || !settings.Vm.ShiftToggle) throw new InvalidDataException("Windows profile round-trip failed");
        settings.Vm.Preset(false); settings.Vm.Save(); settings.Vm.Load();
        if (settings.Vm.Interaction != 1 || settings.Vm.Layout != 1 || settings.Vm.ShiftToggle) throw new InvalidDataException("Shanjie profile round-trip failed");
        settings.LoadProfile(true);
        foreach (int theme in new[] { 1, 2 })
        {
            settings.Vm.Theme = theme;
            for (int page = 0; page < 5; page++)
            {
                settings.ShowSection(page); await Task.Delay(120);
                await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-{page}.png"));
            }
            settings.ShowSection(0); settings.SaveDraft(); await Task.Delay(120);
            await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-saved.png"));
            settings.ShowSection(3); settings.Vm.DictionaryText = "invalid"; settings.SaveDraft(); await Task.Delay(120);
            await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-error.png"));
            settings.Vm.DictionaryText = ""; settings.Vm.Interaction = 1; settings.ShowSection(2); await Task.Delay(120);
            await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-shanjie.png"));
            settings.LoadProfile(true); await Task.Delay(120);
            await Capture(settings.CaptureElement, Path.Combine(output, $"settings-{theme}-error-then-preset.png"));
        }
        var candidates = new CandidateWindow(0);
        string[] words = ["你", "妳", "擬", "禰", "妮", "尼", "倪", "泥", "逆"];
        foreach (int theme in new[] { 1, 2 })
        {
            foreach (int layout in new[] { 0, 1, 2 })
            {
                var items = layout == 2 ? words.Concat(words).ToArray() : words;
                candidates.ShowFrame(new(2, true, layout, 1, 0, 38, -12000, -12000, -12000, 96, theme, 18, layout == 2 ? 9 : 0, false, 0, items, false, "ㄋㄧˇ"));
                // Render() positions at a real monitor; move off-screen immediately, without activation.
                candidates.AppWindow.Move(new PointInt32(-12000, -12000));
                await Task.Delay(150);
                await Capture(candidates.CaptureElement, Path.Combine(output, $"candidates-{theme}-{layout}.png"));
            }
        }
        candidates.Close();
        settings.Close();
    }
}
