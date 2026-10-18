using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.Json.Serialization;
namespace Shanjie_UI.Models;

public sealed record ModelChoice(string Id, string Name, string Revision="");
public sealed record ModelSpec(
    string Id, string Name, string ModelRelease, string ClassesRelease,
    string ModelSha256, string ClassesSha256, long ModelSize, long ClassesSize);
public sealed record CatalogRow(string Name, string State, ModelSpec? Spec, bool Available);
public sealed record CatalogResult(List<CatalogRow> Rows, string Status);
internal sealed record Asset(string Name, long Size, string? Digest, string BrowserDownloadUrl);
internal sealed record Release(string TagName, bool Draft, bool Prerelease, List<Asset> Assets);
internal sealed record Compatibility(int Format, List<ModelSpec> Models);

public static class ModelCatalog
{
    public const string Builtin = "model-v6-classes-v5";
    public static readonly JsonSerializerOptions Json = new() { PropertyNamingPolicy=JsonNamingPolicy.SnakeCaseLower, PropertyNameCaseInsensitive=true };
    public static string Folder => Path.Combine(Path.GetDirectoryName(ProfileStore.Path)!, "Models");
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
        client.DefaultRequestHeaders.UserAgent.ParseAdd("Shanjie-Windows/0.10.2");
        client.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json");
        return client;
    }
    public static List<ModelSpec> Known()
    {
        using var stream=typeof(ModelCatalog).Assembly.GetManifestResourceStream("Models.catalog.json")!;
        return JsonSerializer.Deserialize<Compatibility>(stream,Json)!.Models;
    }
    public static List<ModelChoice> Installed()
    {
        var list=JsonSerializer.Deserialize<List<ModelSpec>>(Interop.Native.Take(Interop.Native.sj_models_scan()),Json)??[];
        return list.Select(x=>new ModelChoice(x.Id,x.Name+"（已下載）",File.GetLastWriteTimeUtc(Path.Combine(Folder,x.Id,"model.json")).Ticks.ToString())).ToList();
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
            for(int page=1;page<=10;page++)
            {
                var batch=JsonSerializer.Deserialize<List<Release>>(await Fetch($"https://api.github.com/repos/Nanako0129/shanjie/releases?per_page=100&page={page}",token),Json)??[];
                releases.AddRange(batch);if(batch.Count<100)break;
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
        return Rows(releases,known,status);
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
            &&a.Digest=="sha256:"+hash&&a.BrowserDownloadUrl==$"https://github.com/Nanako0129/shanjie/releases/download/{tag}/{name}"));
    internal static CatalogResult Rows(List<Release> releases,List<ModelSpec> known,string status)
    {
        releases=releases.Where(r=>r is not null&&r.TagName is not null&&r.Assets is not null).ToList();
        var installed=Installed().Select(x=>x.Id).ToHashSet();var rows=new List<CatalogRow>();
        foreach(var spec in known.Where(ValidSpec))
        {
            bool available=HasAsset(releases,spec.ModelRelease,"bigram.sjlm",spec.ModelSize,spec.ModelSha256)
                &&HasAsset(releases,spec.ClassesRelease,"classes.sjc",spec.ClassesSize,spec.ClassesSha256);
            var state=spec.Id==Builtin?"內建":installed.Contains(spec.Id)?"已下載":available?$"可下載 · {(spec.ModelSize+spec.ClassesSize)/1048576d:F1} MiB":"線上附件無法核對";
            rows.Add(new(spec.Name,state,spec,available&&spec.Id!=Builtin));
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
            using var deadline=CancellationTokenSource.CreateLinkedTokenSource(token);deadline.CancelAfter(TimeSpan.FromMinutes(15));
            using var request=new HttpRequestMessage(HttpMethod.Get,$"https://github.com/Nanako0129/shanjie/releases/download/{tag}/{name}");
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
        if(!ValidSpec(spec))throw new InvalidDataException("此模型配套資訊無效。");
        if(spec.Id==Builtin)return new(Builtin,"model-v6＋classes-v5（內建）");
        var existing=(await Task.Run(Installed,token)).FirstOrDefault(x=>x.Id==spec.Id);if(existing is not null)return existing;
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
