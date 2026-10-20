namespace Shanjie_UI.Setup;

/// <summary>
/// The single-file installer opens this window as soon as ui\ and tools\ are written and keeps
/// extracting the models and data next to them, reporting in setup-extract.txt (replaced as a
/// whole): "running &lt;done&gt; &lt;total&gt;", "done" or "failed &lt;message&gt;". No file means the
/// package was already complete, as when run from the extracted zip.
/// </summary>
internal static class BundleExtraction
{
    private static string StatusFile => Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", "setup-extract.txt"));

    /// <summary>Waits until the package is complete; reports progress from 0 to 100 meanwhile.</summary>
    internal static async Task WaitReady(Action<double> progress)
    {
        while (true)
        {
            string text;
            try { text = File.ReadAllText(StatusFile).Trim(); }
            catch (FileNotFoundException) { return; }
            catch (IOException) { await Task.Delay(100); continue; }
            if (text == "done") return;
            if (text.StartsWith("failed ", StringComparison.Ordinal))
                throw new InvalidDataException("解開安裝檔案失敗：" + text["failed ".Length..]);
            var parts = text.Split(' ');
            if (parts is ["running", var done, var total] && double.TryParse(done, out var d) && double.TryParse(total, out var t) && t > 0)
                progress(Math.Min(100, 100 * d / t));
            await Task.Delay(200);
        }
    }
}
