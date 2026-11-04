using System.Diagnostics;
using System.Net;
using System.Net.Http;
using System.Reflection;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Shanjie_UI.Setup;

/// <summary>Idle：還沒查；Checking：查詢中；UpToDate：已是最新；Available：有新版還沒下載；Downloading：下載中；
/// Ready：已下載並核對，等使用者按「安裝更新」；Failed：這一步失敗（原因在 Error）。</summary>
internal enum UpdateState { Idle, Checking, UpToDate, Available, Downloading, Ready, Failed }

/// <summary>Stable：只看正式版；Prerelease：另外列出比正式版新的測試版。</summary>
internal enum UpdateChannel { Stable, Prerelease }

internal sealed record UpdateRelease(string Version, string Notes, DateTimeOffset PublishedAt, long SizeBytes, string PageUrl, bool Prerelease)
{
    internal Uri Package { get; init; } = null!;
    internal Uri Sums { get; init; } = null!;
}

/// <summary>
/// 從正式 repo 的 GitHub Releases 取比目前新的最新正式版與最新測試版（pre-release，只在比正式版新時列出），使用者選一個；下載單檔安裝程式並核對 Release 附的 SHA-256，
/// 再以 <c>--auto-update</c> 啟動：安裝視窗自己裝完並開啟新版設定。做法參考 Pane 的 UpdateService。
/// </summary>
internal sealed class UpdateService : IDisposable
{
    internal static readonly Uri ProductionSource = new("https://api.github.com/repos/tavricccc/shanjie-multiplatform/releases?per_page=30");
    internal const string Package = "Shanjie-Windows-Setup.exe";
    readonly HttpClient http;
    readonly Uri source;
    readonly string directory;
    readonly string channelFile;
    readonly SemaphoreSlim gate = new(1);
    readonly CancellationTokenSource lifetime = new();
    DateTimeOffset retryAfter;
    string? readyPath;
    bool disposed;
    internal string CurrentVersion { get; }
    internal UpdateState State { get; private set; }
    /// <summary>要檢查的通道；存在 update-channel.txt，與核心共用的 profile.ini 無關。</summary>
    internal UpdateChannel Channel { get; private set; }
    /// <summary>使用者選定要下載的版本；預設是正式版，沒有才是測試版。</summary>
    internal UpdateRelease? Latest { get; private set; }
    /// <summary>可選的新版：正式版在前，測試版在後；最多各一個。</summary>
    internal IReadOnlyList<UpdateRelease> Choices { get; private set; } = [];
    internal double DownloadProgress { get; private set; }
    internal string? Error { get; private set; }
    /// <summary>任何狀態變化；可能在背景執行緒觸發，介面自己切回 UI 執行緒。</summary>
    internal event EventHandler? Changed;

    /// <param name="testSource">只給測試用的本機 HTTP 來源；產品不讀任何更新來源的環境變數。</param>
    internal UpdateService(string version, string data, Uri? testSource = null)
    {
        CurrentVersion = version; directory = Path.Combine(data, "updates"); source = testSource ?? ProductionSource;
        channelFile = Path.Combine(data, "update-channel.txt");
        if (testSource != null && (testSource.Scheme != "http" || testSource.Host != "127.0.0.1"))
            throw new ArgumentException("測試來源必須是本機 HTTP 服務");
        Channel = LoadChannel();
        http = new(new HttpClientHandler { UseCookies = false, UseDefaultCredentials = false, AllowAutoRedirect = false });
        http.DefaultRequestHeaders.UserAgent.ParseAdd("Shanjie-Windows/" + version);
        http.Timeout = TimeSpan.FromMinutes(10);
    }

    /// <summary>執行中的版本；測試版保留 -beta.N 後綴（去掉 + 之後的組建資訊），否則更新檢查會把 beta.1 當成正式版。</summary>
    internal static string RunningVersion()
    {
        var assembly = typeof(UpdateService).Assembly;
        string? informational = assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion?.Split('+')[0];
        if (informational != null && VersionFormat.IsMatch(informational)) return informational;
        var v = assembly.GetName().Version ?? new Version(0, 0, 0);
        return $"{v.Major}.{v.Minor}.{Math.Max(0, v.Build)}";
    }

    /// <summary>讀 update-channel.txt；沒有檔案時，執行中的是測試版就預設測試版通道，否則正式版。</summary>
    UpdateChannel LoadChannel()
    {
        try
        {
            if (File.Exists(channelFile)) return File.ReadAllText(channelFile).Trim() == "prerelease" ? UpdateChannel.Prerelease : UpdateChannel.Stable;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
        return CurrentVersion.Contains('-') ? UpdateChannel.Prerelease : UpdateChannel.Stable;
    }

    /// <summary>切換通道並寫入檔案；寫入失敗時這次執行仍使用新通道。檢查或下載中不切換。</summary>
    internal bool SetChannel(UpdateChannel channel)
    {
        if (State is UpdateState.Checking or UpdateState.Downloading) return false;
        Channel = channel;
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(channelFile)!);
            File.WriteAllText(channelFile, channel == UpdateChannel.Prerelease ? "prerelease" : "stable");
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
        return true;
    }

    /// <summary>X.Y.Z，或測試版的 X.Y.Z-beta.N。</summary>
    static readonly Regex VersionFormat = new(@"^\d+\.\d+\.\d+(-beta\.\d+)?$", RegexOptions.CultureInvariant);

    internal static string DataFolder() =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Shanjie");

    /// <summary>
    /// 依 semver 比較；a 比 b 新回傳正數。先逐段比 X.Y.Z；數字相同時，沒有後綴的正式版比有後綴的測試版新，
    /// 兩個測試版再逐段比後綴（beta.2 比 beta.1 新，beta.10 比 beta.2 新）。
    /// </summary>
    internal static int Compare(string a, string b)
    {
        string[] x = a.Split('-', 2), y = b.Split('-', 2);
        int[] cx = x[0].Split('.').Select(int.Parse).ToArray(), cy = y[0].Split('.').Select(int.Parse).ToArray();
        for (int i = 0; i < Math.Max(cx.Length, cy.Length); i++)
        {
            int d = (i < cx.Length ? cx[i] : 0).CompareTo(i < cy.Length ? cy[i] : 0);
            if (d != 0) return d;
        }
        if (x.Length != y.Length) return x.Length == 1 ? 1 : -1;
        if (x.Length == 1) return 0;
        string[] px = x[1].Split('.'), py = y[1].Split('.');
        for (int i = 0; i < Math.Min(px.Length, py.Length); i++)
        {
            bool nx = int.TryParse(px[i], out int vx), ny = int.TryParse(py[i], out int vy);
            int d = nx && ny ? vx.CompareTo(vy) : nx ? -1 : ny ? 1 : string.CompareOrdinal(px[i], py[i]);
            if (d != 0) return d;
        }
        return px.Length.CompareTo(py.Length);
    }

    void Set(UpdateState state, string? error = null) { State = state; Error = error; Changed?.Invoke(this, EventArgs.Empty); }

    internal async Task CheckAsync(CancellationToken cancellationToken)
    {
        if (disposed || !await gate.WaitAsync(0, cancellationToken).ConfigureAwait(false)) return;
        using var cancel = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken, lifetime.Token);
        try
        {
            if (DateTimeOffset.UtcNow < retryAfter) { Set(UpdateState.Failed, "GitHub 暫時限制了檢查次數，過一會兒再試"); return; }
            Set(UpdateState.Checking);
            using var response = await GetAsync(source, cancel.Token).ConfigureAwait(false);
            using var json = JsonDocument.Parse(await response.Content.ReadAsStringAsync(cancel.Token).ConfigureAwait(false));
            Latest = null; readyPath = null; Choices = [];
            var found = json.RootElement.EnumerateArray().Select(Parse).OfType<UpdateRelease>()
                .Where(r => Compare(r.Version, CurrentVersion) > 0).ToList();
            var stable = found.Where(r => !r.Prerelease).MaxBy(r => r.Version, Comparer<string>.Create(Compare));
            var preview = Channel == UpdateChannel.Prerelease
                ? found.Where(r => r.Prerelease && (stable is null || Compare(r.Version, stable.Version) > 0)).MaxBy(r => r.Version, Comparer<string>.Create(Compare))
                : null;
            Choices = new[] { stable, preview }.OfType<UpdateRelease>().ToArray();
            if (Choices.Count == 0) { Set(UpdateState.UpToDate); return; }
            Latest = Choices[0];
            Set(UpdateState.Available);
        }
        catch (OperationCanceledException) { Set(cancel.IsCancellationRequested ? UpdateState.Idle : UpdateState.Failed, cancel.IsCancellationRequested ? null : "連 GitHub 逾時，稍後再試"); }
        catch (Exception e) when (Expected(e)) { Set(UpdateState.Failed, Message(e)); }
        finally { gate.Release(); }
    }

    /// <summary>改選另一個版本（還沒開始下載或下載失敗時）。</summary>
    internal void Choose(UpdateRelease release)
    {
        if (!Choices.Contains(release) || State is UpdateState.Checking or UpdateState.Downloading) return;
        Latest = release; readyPath = null; Set(UpdateState.Available);
    }

    /// <summary>一筆 Release；草稿、版本號不合格式或缺安裝檔與 .sha256 的略過。</summary>
    UpdateRelease? Parse(JsonElement root)
    {
        if (root.GetProperty("draft").GetBoolean()) return null;
        string version = root.GetProperty("tag_name").GetString()!.TrimStart('v');
        if (!VersionFormat.IsMatch(version)) return null;
        var files = root.GetProperty("assets").EnumerateArray().ToArray();
        var package = files.FirstOrDefault(f => f.GetProperty("name").GetString() == Package);
        var sums = files.FirstOrDefault(f => f.GetProperty("name").GetString() == Package + ".sha256");
        if (package.ValueKind == JsonValueKind.Undefined || sums.ValueKind == JsonValueKind.Undefined) return null;
        return new(version, PlainNotes(root.GetProperty("body").GetString() ?? ""), root.GetProperty("published_at").GetDateTimeOffset(),
            package.GetProperty("size").GetInt64(), ValidateUrl(root.GetProperty("html_url").GetString()!).AbsoluteUri, root.GetProperty("prerelease").GetBoolean())
        {
            Package = ValidateUrl(package.GetProperty("browser_download_url").GetString()!),
            Sums = ValidateUrl(sums.GetProperty("browser_download_url").GetString()!),
        };
    }

    internal async Task DownloadAsync(CancellationToken cancellationToken)
    {
        if (disposed || !await gate.WaitAsync(0, cancellationToken).ConfigureAwait(false)) return;
        using var cancel = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken, lifetime.Token);
        try { if (Latest != null) await DownloadCore(cancel.Token).ConfigureAwait(false); }
        catch (OperationCanceledException) { Set(cancel.IsCancellationRequested ? UpdateState.Available : UpdateState.Failed, cancel.IsCancellationRequested ? null : "下載逾時，稍後再試"); }
        catch (Exception e) when (Expected(e)) { Set(UpdateState.Failed, Message(e)); }
        finally { gate.Release(); }
    }

    async Task DownloadCore(CancellationToken token)
    {
        Directory.CreateDirectory(directory);
        string target = Path.Combine(directory, $"Shanjie-Windows-Setup-{Latest!.Version}.exe"), partial = target + ".partial";
        readyPath = null; DownloadProgress = 0; Set(UpdateState.Downloading);
        try
        {
            using var checksum = await GetAsync(Latest.Sums, token).ConfigureAwait(false);
            var match = Regex.Match((await checksum.Content.ReadAsStringAsync(token).ConfigureAwait(false)).Trim(), @"^([a-fA-F0-9]{64})\s+\*?(.+)$");
            if (!match.Success || match.Groups[2].Value.Trim() != Package) throw new InvalidDataException();
            string hash = match.Groups[1].Value;
            if (File.Exists(target) && await Valid(target, hash, token).ConfigureAwait(false)) { Ready(target); return; }
            File.Delete(target);
            using var response = await GetAsync(Latest.Package, token).ConfigureAwait(false);
            await using (var input = await response.Content.ReadAsStreamAsync(token).ConfigureAwait(false))
            await using (var output = new FileStream(partial, FileMode.Create, FileAccess.Write, FileShare.None, 65536, true))
            {
                byte[] buffer = new byte[65536]; long received = 0; double notified = 0; var watch = Stopwatch.StartNew();
                int read;
                while ((read = await input.ReadAsync(buffer, token).ConfigureAwait(false)) != 0)
                {
                    await output.WriteAsync(buffer.AsMemory(0, read), token).ConfigureAwait(false); received += read;
                    double progress = Latest.SizeBytes > 0 ? Math.Min(1, (double)received / Latest.SizeBytes) : 0;
                    if (progress - notified >= .01 || watch.ElapsedMilliseconds >= 200)
                    { DownloadProgress = progress; notified = progress; watch.Restart(); Changed?.Invoke(this, EventArgs.Empty); }
                }
            }
            if (!await Valid(partial, hash, token).ConfigureAwait(false)) throw new InvalidDataException();
            File.Move(partial, target, true); Ready(target);
        }
        finally { File.Delete(partial); }
    }

    async Task<bool> Valid(string path, string hash, CancellationToken token)
    {
        if (new FileInfo(path).Length != Latest!.SizeBytes) return false;
        await using var stream = File.OpenRead(path);
        return Convert.ToHexString(await SHA256.HashDataAsync(stream, token).ConfigureAwait(false)).Equals(hash, StringComparison.OrdinalIgnoreCase);
    }

    void Ready(string path)
    {
        // 只留這次下載的安裝程式。
        foreach (string old in Directory.EnumerateFiles(directory, "Shanjie-Windows-Setup-*.exe")) if (old != path) File.Delete(old);
        readyPath = path; DownloadProgress = 1; Set(UpdateState.Ready);
    }

    Uri ValidateUrl(string text)
    {
        var url = new Uri(text);
        bool allowed = source != ProductionSource ? url.Scheme == "http" && url.Host == "127.0.0.1" && url.Port == source.Port :
            url.Scheme == "https" && (url.Host == "github.com" || url.Host == "api.github.com" || url.Host.EndsWith(".githubusercontent.com", StringComparison.OrdinalIgnoreCase));
        if (!allowed || url.UserInfo.Length != 0) throw new InvalidDataException();
        return url;
    }

    async Task<HttpResponseMessage> GetAsync(Uri url, CancellationToken token)
    {
        for (int redirects = 0; redirects < 6; redirects++)
        {
            var response = await http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, token).ConfigureAwait(false);
            if ((int)response.StatusCode is 403 or 429)
            {
                retryAfter = DateTimeOffset.UtcNow.AddHours(1);
                if (response.Headers.RetryAfter?.Delta is { } delay && DateTimeOffset.UtcNow + delay > retryAfter) retryAfter = DateTimeOffset.UtcNow + delay;
                if (response.Headers.TryGetValues("X-RateLimit-Reset", out var values) && long.TryParse(values.FirstOrDefault(), out long seconds))
                { var reset = DateTimeOffset.FromUnixTimeSeconds(seconds); if (reset > retryAfter) retryAfter = reset; }
                response.Dispose(); throw new HttpRequestException("限流", null, HttpStatusCode.TooManyRequests);
            }
            if ((int)response.StatusCode is 301 or 302 or 303 or 307 or 308)
            {
                var location = response.Headers.Location; response.Dispose();
                url = ValidateUrl(new Uri(url, location ?? throw new InvalidDataException()).AbsoluteUri); continue;
            }
            if (!response.IsSuccessStatusCode) { var status = response.StatusCode; response.Dispose(); throw new HttpRequestException("請求失敗", null, status); }
            return response;
        }
        throw new HttpRequestException("重新導向過多");
    }

    internal static string PlainNotes(string notes)
    {
        notes = Regex.Replace(notes, @"!?\[([^\]]*)\]\([^)]*\)", "$1");
        notes = Regex.Replace(notes, @"(?m)^\s{0,3}(#{1,6}\s*|>\s*|[-*+]\s+)", "");
        notes = Regex.Replace(notes, @"<[^>]+>|[`*_~]", "").Trim();
        return notes[..Math.Min(2000, notes.Length)];
    }

    static bool Expected(Exception e) => e is InvalidDataException or HttpRequestException or IOException or UnauthorizedAccessException or JsonException or InvalidOperationException or FormatException or ArgumentException;

    static string Message(Exception e) => e switch
    {
        InvalidDataException => "下載的檔案不完整，已刪除，稍後再試",
        HttpRequestException { StatusCode: HttpStatusCode.TooManyRequests } => "GitHub 暫時限制了檢查次數，過一會兒再試",
        HttpRequestException { StatusCode: HttpStatusCode.NotFound } => "找不到 Release",
        HttpRequestException => "連不上 GitHub，檢查網路後再試",
        UnauthorizedAccessException or IOException => "更新檔存不下來，看看磁碟空間夠不夠",
        _ => "看不懂 GitHub 回傳的更新資訊，稍後再試",
    };

    /// <summary>只在 Ready 時：以自動更新方式啟動安裝程式；成功回傳 true，呼叫端接著關閉設定視窗。</summary>
    internal bool Install()
    {
        if (disposed || State != UpdateState.Ready || readyPath == null) return false;
        try
        {
            var start = new ProcessStartInfo(readyPath) { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(readyPath)! };
            start.ArgumentList.Add("--auto-update");
            using var setup = Process.Start(start) ?? throw new InvalidOperationException();
            return true;
        }
        catch (Exception e) when (e is IOException or System.ComponentModel.Win32Exception or InvalidOperationException)
        { Set(UpdateState.Failed, "安裝程式沒能啟動，稍後再試"); return false; }
    }

    public void Dispose() { if (disposed) return; disposed = true; lifetime.Cancel(); http.Dispose(); }
}
