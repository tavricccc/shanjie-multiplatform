using System.Text.Json;
namespace Shanjie_UI.Models;
public sealed record ModelAsset(string Role,string Repository,string Release,string Filename,string FormatMagic,long Size,string Sha256,List<string> RequiresAssets);
public sealed record ModelCompatibility(string Target,string ProfileId,string SourceCommit,string State,string CheckedAt,string Report,string? Reason=null);
public sealed record ModelAdvisory(string Code,string Title,string Body,List<JsonElement> Measurements);
public sealed record ModelEntry(string Id,string Name,List<string> RequiredCapabilities,List<ModelAsset> Assets,List<ModelCompatibility> Compatibility,string AdvisoryRevision,List<ModelAdvisory> Advisories);
internal sealed record ModelList(int Format,string Revision,string GeneratedAt,bool ExampleOnly,List<ModelEntry> Models);
public static partial class ModelCatalog
{
    public const string V2Endpoint="https://raw.githubusercontent.com/tavricccc/shanjie-multiplatform/model-catalog/MODEL-CATALOG-V2.json";
    private static readonly HashSet<string> Capabilities=["lm.sjlm1","lm.classless","classes.sjcl1","map.sjwm2","kn.sjkn2","trigram.sjt30001","semantic.sjtl1","semantic.sjtl2","semantic.sjtl3","semantic.sjtl4"];
    public static bool Supported(ModelEntry entry)=>entry.RequiredCapabilities.All(Capabilities.Contains)&&entry.Compatibility.Any(c=>c.Target=="windows-x64"&&c.ProfileId=="windows-bundle-v1"&&c.State=="usable");
    public static string Notes(ModelEntry? entry)=>entry is null?"尚無此配套的評測／備註資料。":string.Join("\n\n",entry.Advisories.Select(a=>a.Title+"\n"+a.Body));
    private static ModelSpec Spec(ModelEntry m)
    {
        var main=m.Assets.Single(a=>a.Role=="bigram");var classes=m.Assets.SingleOrDefault(a=>a.Role=="classes");
        return new(m.Id,m.Name,main.Release,classes?.Release??"",main.Sha256,classes?.Sha256??"",main.Size,classes?.Size??0,m);
    }
    private static async Task<CatalogResult> WithVersionTwo(CatalogResult legacy,List<Release> releases,CancellationToken token)
    {
        string path=Path.Combine(Folder,"catalog-v2-cache.json"),status=legacy.Status;ModelList? list=null;
        try
        {
            string raw=await Fetch(V2Endpoint,token);
            string verified=await Task.Run(()=>Interop.Native.Take(Interop.Native.sj_catalog_validate(raw)),token);
            list=JsonSerializer.Deserialize<ModelList>(verified,Json)??throw new InvalidDataException("清單無內容。");
            await File.WriteAllTextAsync(path+".download",verified,token);File.Move(path+".download",path,true);
            status+=" 自動維護清單已更新。";
        }
        catch(Exception error)when(error is IOException or System.Net.Http.HttpRequestException or JsonException or OperationCanceledException)
        {
            token.ThrowIfCancellationRequested();
            if(File.Exists(path))
            {
                var verified=Interop.Native.Take(Interop.Native.sj_catalog_validate(await File.ReadAllTextAsync(path,token)));
                list=JsonSerializer.Deserialize<ModelList>(verified,Json);status+=" 自動維護清單使用離線快取。";
            }
            else status+=" 自動維護清單暫時無法取得，仍可使用既有模型。";
        }
        if(list is null)return legacy with {Status=status};
        var rows=legacy.Rows;var installed=Installed().Select(m=>m.Id).ToHashSet();
        foreach(var m in list.Models)
        {
            bool supported=Supported(m);
            bool source=m.Assets.All(a=>releases.Any(r=>r.TagName==a.Release&&!r.Draft&&r.Assets.Any(x=>x.Name==a.Filename&&x.Size==a.Size&&x.Digest=="sha256:"+a.Sha256&&x.BrowserDownloadUrl==$"https://github.com/Nanako0129/shanjie/releases/download/{a.Release}/{a.Filename}")));
            string state=!supported?"需要更新核心／格式支援":installed.Contains(m.Id)?"已下載":source?"可下載":"來源附件暫時無法核對";
            if(m.Advisories.Count>0)state+=" · "+string.Join("、",m.Advisories.Select(a=>a.Title));
            rows.Add(new(m.Name,state,Spec(m),supported&&source));
        }
        return new(rows,status);
    }
    private static async Task<ModelChoice> DownloadBundle(ModelSpec spec,IProgress<string> progress,CancellationToken token)
    {
        var m=spec.Catalog!;if(!Supported(m))throw new InvalidDataException("此配套需要更新核心。");
        // Native validation checks the immutable identity, all capabilities and paired files again.
        var existing=(await Task.Run(Installed,token)).FirstOrDefault(x=>x.Id==spec.Id);if(existing is not null)return existing;
        var files=new List<(ModelAsset Asset,string Path)>();
        foreach(var a in m.Assets)files.Add((a,await AssetFile(a.Release,a.Filename,a.Size,a.Sha256,progress,token)));
        token.ThrowIfCancellationRequested();
        string work=Guid.NewGuid().ToString("N"),stage=Path.Combine(Folder,".staging",work);Directory.CreateDirectory(stage);
        await Task.Run(()=>{
            foreach(var (a,path) in files)File.Copy(path,Path.Combine(stage,a.Filename));
            File.WriteAllText(Path.Combine(stage,"model.json"),JsonSerializer.Serialize(spec,Json));
            Interop.Native.Take(Interop.Native.sj_model_prepare(work));
        },token);
        token.ThrowIfCancellationRequested();
        return new(spec.Id,spec.Name+"（已下載）",m.AdvisoryRevision,m);
    }
}
