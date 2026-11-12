using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Serialization;
namespace Shanjie_UI.Models;

public sealed record ModelChoice(string Id, string Name, string Revision="",ModelEntry? Catalog=null)
{
    internal ModelSpec? InstalledSpec { get; init; }
}
public sealed record ModelSpec(
    string Id, string Name, string ModelRelease, string ClassesRelease,
    string ModelSha256, string ClassesSha256, long ModelSize, long ClassesSize,ModelEntry? Catalog=null);
public sealed record CatalogRow(string Name, string State, ModelSpec? Spec, bool Available, string Summary="",string ActionLabel="下載");
/// ListGenerated is the maintained list's own generated_at; ListCached marks it as the offline copy.
public sealed record CatalogResult(List<CatalogRow> Rows, string Status, DateTimeOffset? ListGenerated=null, bool ListCached=false);
internal sealed record Asset(string Name, long Size, string? Digest, string BrowserDownloadUrl);
internal sealed record Release(string TagName, bool Draft, bool Prerelease, List<Asset> Assets);
internal sealed record Compatibility(int Format, List<ModelSpec> Models);
internal sealed record BuiltinAsset(string Role, string Release, string Filename, long Size, string Sha256);
internal sealed record BuiltinSet(string Id, List<BuiltinAsset> Assets);
internal sealed record ModelSources(int Format, BuiltinSet Builtin);

public static partial class ModelCatalog
{
    public const string Builtin = "builtin";
    public static string BuiltinName => string.Join("＋", BuiltinAssets.Value
        .Where(a => a.Role is "bigram" or "classes" or "semantic")
        .Select(a => a.Release[..a.Release.LastIndexOf('-')].ToUpperInvariant())) + "（內建）";
    public static readonly JsonSerializerOptions Json = new() { PropertyNamingPolicy=JsonNamingPolicy.SnakeCaseLower, PropertyNameCaseInsensitive=true };
    public static string Folder => Path.Combine(Path.GetDirectoryName(ProfileStore.Path)!, "Models");
    internal static readonly string[] UpstreamRepositories = ["Nanako0129/shanjie", "tavricccc/shanjie-upstream-backup"];
    private static bool IsUpstreamAssetUrl(string url,string tag,string name)=>UpstreamRepositories.Any(repository=>url==$"https://github.com/{repository}/releases/download/{tag}/{name}");
    private static readonly HttpClient LiveClient = CreateClient();
    private static HttpClient? verificationClient;
    private static HttpClient Client => verificationClient??LiveClient;
    internal static void VerifyWith(HttpClient? client)
    {
        if(Environment.GetEnvironmentVariable("SHANJIE_TEST_PROFILE")!="1")throw new InvalidOperationException("Isolated verification only");
        verificationClient=client;
    }
    private static HttpClient CreateClient()
    {
        var client=new HttpClient {Timeout=Timeout.InfiniteTimeSpan};
        client.DefaultRequestHeaders.UserAgent.ParseAdd("Shanjie-Windows/0.13.0");
        client.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json");
        return client;
    }
    public static List<ModelSpec> Known()
    {
        using var stream=typeof(ModelCatalog).Assembly.GetManifestResourceStream("Models.catalog.json")!;
        return JsonSerializer.Deserialize<Compatibility>(stream,Json)!.Models;
    }
    /// The package's own model set (MODEL-SOURCES.json); it is never downloaded.
    internal static readonly Lazy<List<BuiltinAsset>> BuiltinAssets = new(() =>
    {
        using var stream=typeof(ModelCatalog).Assembly.GetManifestResourceStream("Models.sources.json")!;
        return JsonSerializer.Deserialize<ModelSources>(stream,Json)!.Builtin.Assets;
    });
    public static ModelSpec BuiltinSpec
    {
        get
        {
            var main=BuiltinAssets.Value.Single(a=>a.Role=="bigram");var classes=BuiltinAssets.Value.Single(a=>a.Role=="classes");
            return new(Builtin,BuiltinName,main.Release,classes.Release,main.Sha256,classes.Sha256,main.Size,classes.Size);
        }
    }
    public static List<ModelChoice> Installed()
    {
        var list=JsonSerializer.Deserialize<List<ModelSpec>>(Interop.Native.Take(Interop.Native.sj_models_scan()),Json)??[];
        var notes=CachedNotes();
        return list.Select(x=>new ModelChoice(x.Id,x.Name+"（已下載）",File.GetLastWriteTimeUtc(Path.Combine(Folder,x.Id,"model.json")).Ticks.ToString(),
            notes.GetValueOrDefault(x.Id)??notes.Values.FirstOrDefault(m=>SameSources(m,x))??x.Catalog){InstalledSpec=x}).ToList();
    }
    internal static bool HasUpdate(ModelSpec local,ModelSpec offered)
    {
        // Source repositories change bundle IDs. Compare the actual versions/files independently.
        if(local.Catalog is {} saved&&offered.Catalog is {} current)
        {
            var files=saved.Assets.Select(a=>(a.Role,a.Release,a.Filename,a.Sha256,a.Size)).ToHashSet();
            return !files.SetEquals(current.Assets.Select(a=>(a.Role,a.Release,a.Filename,a.Sha256,a.Size)));
        }
        if(offered.Catalog is {} bundle&&bundle.Assets.Any(a=>a.Role is not ("bigram" or "classes")))return true;
        return local.ModelRelease!=offered.ModelRelease||local.ClassesRelease!=offered.ClassesRelease
            ||local.ModelSha256!=offered.ModelSha256||local.ClassesSha256!=offered.ClassesSha256;
    }
    internal static ModelChoice? FindInstalled(ModelSpec offered,IReadOnlyList<ModelChoice> installed)
    {
        var named=installed.Where(m=>string.Equals(m.InstalledSpec!.Name,offered.Name,StringComparison.Ordinal)).ToList();
        return named.FirstOrDefault(m=>!HasUpdate(m.InstalledSpec!,offered))??named.FirstOrDefault()
            ??installed.FirstOrDefault(m=>m.Id==offered.Id||m.Catalog?.Id==offered.Id);
    }
    public static string[] Issues()=>JsonSerializer.Deserialize<string[]>(Interop.Native.Take(Interop.Native.sj_models_errors()),Json)??[];
    private static async Task<string> Fetch(string url,CancellationToken token)
    {
        using var deadline=CancellationTokenSource.CreateLinkedTokenSource(token);deadline.CancelAfter(TimeSpan.FromSeconds(25));
        using var response=await Client.GetAsync(url,HttpCompletionOption.ResponseHeadersRead,deadline.Token);
        response.EnsureSuccessStatusCode();
        if(response.Content.Headers.ContentLength>8*1024*1024)throw new InvalidDataException("線上清單過大。");
        await using var stream=await response.Content.ReadAsStreamAsync(deadline.Token);
        using var buffer=new MemoryStream();var bytes=new byte[65536];int count;
        while((count=await stream.ReadAsync(bytes,deadline.Token))>0){if(buffer.Length+count>8*1024*1024)throw new InvalidDataException("線上清單過大。");buffer.Write(bytes,0,count);}
        return System.Text.Encoding.UTF8.GetString(buffer.ToArray());
    }
    public static async Task<CatalogResult> Online(CancellationToken token)
    {
        Directory.CreateDirectory(Folder);string cache=Path.Combine(Folder,"catalog-cache.json");
        List<Release> releases=[];string status="已更新上游公開清單。";
        try
        {
            foreach(string repository in UpstreamRepositories)
            {
                releases.Clear();
                try
                {
                    for(int page=1;page<=10;page++)
                    {
                        var batch=JsonSerializer.Deserialize<List<Release>>(await Fetch($"https://api.github.com/repos/{repository}/releases?per_page=100&page={page}",token),Json)??[];
                        releases.AddRange(batch);if(batch.Count<100)break;
                    }
                    if(repository!=UpstreamRepositories[0])status="上游暫時無法讀取，已更新鏡像公開清單。";
                    break;
                }
                catch(Exception e) when(repository==UpstreamRepositories[0]&&e is HttpRequestException or IOException or JsonException or OperationCanceledException)
                {
                    token.ThrowIfCancellationRequested();
                }
            }
            var temporary=cache+".download";
            await File.WriteAllTextAsync(temporary,JsonSerializer.Serialize(releases,Json),token);File.Move(temporary,cache,true);
        }
        catch(Exception e) when(e is HttpRequestException or IOException or JsonException or OperationCanceledException)
        {
            token.ThrowIfCancellationRequested();
            if(File.Exists(cache))
            {
                releases=JsonSerializer.Deserialize<List<Release>>(await File.ReadAllTextAsync(cache,token),Json)??[];
                status=$"無法更新線上清單；顯示 {File.GetLastWriteTime(cache):yyyy/MM/dd HH:mm} 的快取。";
            }
            else return new([],"目前無法連線取得清單；內建及已下載模型仍可使用。請稍後重試。");
        }
        var known=Known();
        // Optional project-maintained compatibility metadata can add data versions without a binary update.
        try
        {
            var remote=JsonSerializer.Deserialize<Compatibility>(await Fetch("https://raw.githubusercontent.com/tavricccc/shanjie-multiplatform/main/MODEL-CATALOG.json",token),Json);
            if(remote is {Format:1,Models:not null}) foreach(var spec in remote.Models.Where(ValidSpec)) if(known.All(x=>x.Id!=spec.Id))known.Add(spec);
        }
        catch(Exception e) when(e is HttpRequestException or IOException or JsonException or OperationCanceledException){token.ThrowIfCancellationRequested();}
        var legacy=await Task.Run(()=>Rows(releases,known,status),token);
        return await WithVersionTwo(legacy,releases,token);
    }
    private static bool ValidSpec(ModelSpec s)=>s is not null&&s.Id is {Length:>0 and <=80}&&s.Name is {Length:>0 and <=240}
        &&s.ModelRelease is not null&&s.ClassesRelease is not null&&s.ModelSha256 is not null&&s.ClassesSha256 is not null
        &&s.Id.All(c=>char.IsAsciiLetterLower(c)||char.IsAsciiDigit(c)||c=='-')
        &&s.ModelRelease.StartsWith("model-v",StringComparison.Ordinal)&&s.ClassesRelease.StartsWith("classes-v",StringComparison.Ordinal)
        &&s.ModelRelease.All(c=>char.IsAsciiLetterOrDigit(c)||c=='-')&&s.ClassesRelease.All(c=>char.IsAsciiLetterOrDigit(c)||c=='-')
        &&s.ModelSize is >0 and <=1073741824&&s.ClassesSize is >0 and <=1073741824
        &&s.ModelSha256.Length==64&&s.ClassesSha256.Length==64&&s.ModelSha256.All(Uri.IsHexDigit)&&s.ClassesSha256.All(Uri.IsHexDigit);
    private static bool HasAsset(List<Release> releases,string tag,string name,long size,string hash)=>
        releases.Any(r=>r is not null&&r.TagName==tag&&!r.Draft&&!r.Prerelease&&r.Assets is not null&&r.Assets.Any(a=>a is not null&&a.Name==name&&a.Size==size
            &&a.Digest=="sha256:"+hash&&IsUpstreamAssetUrl(a.BrowserDownloadUrl,tag,name)));
    internal static CatalogResult Rows(List<Release> releases,List<ModelSpec> known,string status)
    {
        releases=releases.Where(r=>r is not null&&r.TagName is not null&&r.Assets is not null).ToList();
        var installed=Installed();var rows=new List<CatalogRow>{new(BuiltinName,"內建",BuiltinSpec,false)};
        foreach(var spec in known.Where(ValidSpec))
        {
            bool available=HasAsset(releases,spec.ModelRelease,"bigram.sjlm",spec.ModelSize,spec.ModelSha256)
                &&HasAsset(releases,spec.ClassesRelease,"classes.sjc",spec.ClassesSize,spec.ClassesSha256);
            var downloaded=FindInstalled(spec,installed);bool update=downloaded is not null&&HasUpdate(downloaded.InstalledSpec!,spec);
            var state=downloaded is not null?(update?"已下載・可更新":"已下載"):available?$"可下載 · {(spec.ModelSize+spec.ClassesSize)/1048576d:F1} MiB":"線上附件無法核對";
            rows.Add(new(spec.Name,state,spec,available||downloaded is not null&&!update,ActionLabel:downloaded is null?"下載":update?"更新":"選用"));
        }
        foreach(var release in releases.Where(r=>!r.Draft&&r.TagName.StartsWith("model-v",StringComparison.Ordinal)&&known.All(x=>x.ModelRelease!=r.TagName)))
            rows.Add(new(release.TagName,release.Prerelease?"預覽資料，尚未支援":"尚未確認相容配套",null,false));
        return new(rows,status);
    }
    private static async Task<string> AssetFile(string tag,string name,long size,string sha,IProgress<string> progress,CancellationToken token)
    {
        string assets=Path.Combine(Folder,"Assets");Directory.CreateDirectory(assets);
        string complete=Path.Combine(assets,sha),partial=complete+".download";
        if(File.Exists(complete)&&await Matches(complete,size,sha,token))return complete;
        long offset=File.Exists(partial)?new FileInfo(partial).Length:0;
        if(offset>size){File.Delete(partial);offset=0;}
        if(offset<size)
        {
            foreach(string repository in UpstreamRepositories)
            {
                offset=File.Exists(partial)?new FileInfo(partial).Length:0;
                try
                {
                    using var deadline=CancellationTokenSource.CreateLinkedTokenSource(token);deadline.CancelAfter(TimeSpan.FromMinutes(15));
                    using var request=new HttpRequestMessage(HttpMethod.Get,$"https://github.com/{repository}/releases/download/{tag}/{name}");
                    if(offset>0)request.Headers.Range=new RangeHeaderValue(offset,null);
                    using var response=await Client.SendAsync(request,HttpCompletionOption.ResponseHeadersRead,deadline.Token);response.EnsureSuccessStatusCode();
                    bool append=response.StatusCode==HttpStatusCode.PartialContent&&offset>0;
                    if(response.StatusCode==HttpStatusCode.PartialContent&&response.Content.Headers.ContentRange?.From!=offset)throw new InvalidDataException("下載分段位置不符，尚未套用。");
                    if(!append)offset=0;
                    await using var output=new FileStream(partial,append?FileMode.Append:FileMode.Create,FileAccess.Write,FileShare.None,65536,true);
                    await using var input=await response.Content.ReadAsStreamAsync(deadline.Token);var buffer=new byte[65536];int count;
                    while((count=await input.ReadAsync(buffer,deadline.Token))>0)
                    {
                        if(offset+count>size)throw new InvalidDataException("下載檔案超過預期大小，尚未套用。");
                        await output.WriteAsync(buffer.AsMemory(0,count),deadline.Token);offset+=count;
                        progress.Report($"下載 {tag}：{offset/1048576d:F1} / {size/1048576d:F1} MiB（{100d*offset/size:F0}%）");
                    }
                    break;
                }
                catch(Exception e) when(repository==UpstreamRepositories[0]&&(e is HttpRequestException or OperationCanceledException || e is IOException))
                {
                    token.ThrowIfCancellationRequested();
                    progress.Report("上游附件暫時無法讀取，改由鏡像繼續下載…");
                }
            }
        }
        progress.Report($"核對 {tag} 的完整性…");
        if(!await Matches(partial,size,sha,token)){File.Delete(partial);throw new InvalidDataException("下載檔案的大小或 SHA-256 不符，請重試。");}
        File.Move(partial,complete,true);return complete;
    }
    private static async Task<bool> Matches(string path,long size,string sha,CancellationToken token)
    {
        if(new FileInfo(path).Length!=size)return false;
        await using var stream=File.OpenRead(path);return Convert.ToHexString(await SHA256.HashDataAsync(stream,token)).Equals(sha,StringComparison.OrdinalIgnoreCase);
    }
    public static async Task<ModelChoice> Download(ModelSpec spec,IProgress<string> progress,CancellationToken token)
    {
        if(spec.Catalog is not null)return await DownloadBundle(spec,progress,token);
        if(!ValidSpec(spec))throw new InvalidDataException("此模型配套資訊無效。");
        if(spec.Id==Builtin)return new(Builtin,BuiltinName);
        var existing=FindInstalled(spec,await Task.Run(Installed,token));if(existing is not null&&!HasUpdate(existing.InstalledSpec!,spec))return existing;
        if(existing?.Id==spec.Id)
        {
            var revision=System.Text.Encoding.UTF8.GetBytes($"{spec.ModelRelease}\n{spec.ClassesRelease}\n{spec.ModelSha256}\n{spec.ClassesSha256}");
            spec=spec with{Id="model-update-"+Convert.ToHexString(SHA256.HashData(revision)).ToLowerInvariant()[..32]};
        }
        string model=await AssetFile(spec.ModelRelease,"bigram.sjlm",spec.ModelSize,spec.ModelSha256,progress,token);
        string classes=await AssetFile(spec.ClassesRelease,"classes.sjc",spec.ClassesSize,spec.ClassesSha256,progress,token);
        token.ThrowIfCancellationRequested();string work=Guid.NewGuid().ToString("N");string stage=Path.Combine(Folder,".staging",work);Directory.CreateDirectory(stage);
        progress.Report("下載完成，正在建立及驗證 Windows 模型快取…");
        await Task.Run(()=>{
            File.Copy(model,Path.Combine(stage,"bigram.sjlm"));File.Copy(classes,Path.Combine(stage,"classes.sjc"));
            File.WriteAllText(Path.Combine(stage,"model.json"),JsonSerializer.Serialize(spec,Json));
            Interop.Native.Take(Interop.Native.sj_model_prepare(work));
        },token);
        token.ThrowIfCancellationRequested();return new(spec.Id,spec.Name+"（已下載）");
    }
}
