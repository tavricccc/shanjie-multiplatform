using Shanjie_UI.Models;
using System.Net;
using System.Net.Http;
using System.Security.Cryptography;
using System.Text.Json;
namespace Shanjie_UI.Verification;
internal static class ModelVerifier
{
    internal static async Task RunVersionTwo(MainWindow settings,string output,string fixtures)
    {
        if(Environment.GetEnvironmentVariable("SHANJIE_TEST_PROFILE")!="1")throw new InvalidDataException("Isolated profile required");
        settings.AppWindow.Move(new Windows.Graphics.PointInt32(-12000,-12000));settings.AppWindow.Show(false);
        string log=Path.Combine(output,"models-v2.log");void Check(bool ok,string what){if(!ok)throw new InvalidDataException(what);File.AppendAllText(log,"PASS "+what+Environment.NewLine);}
        var catalog=JsonSerializer.Deserialize<ModelList>(await File.ReadAllTextAsync(Path.Combine(fixtures,"fixture-v2.json")),ModelCatalog.Json)!;
        var future=JsonSerializer.Deserialize<ModelList>(await File.ReadAllTextAsync(Path.Combine(fixtures,"future-v2.json")),ModelCatalog.Json)!.Models.Single();
        string futureRelease=future.Assets.Single(a=>a.Role=="semantic").Release;
        var selected=catalog.Models.First(m=>m.Assets.Any(a=>a.Role=="semantic")&&m.Advisories.Count>0);
        var files=catalog.Models.SelectMany(m=>m.Assets).DistinctBy(a=>a.Sha256).ToList();
        var requests=new List<string>();bool mirrorOnly=false,staleIndex=false,newRelease=false,indexUnavailable=false,revisedRelease=false;
        using var client=new HttpClient(new Replies(request=>{
            string url=request.RequestUri!.AbsoluteUri;requests.Add(url);
            if(mirrorOnly&&url.Contains("/Nanako0129/shanjie/"))return new(HttpStatusCode.ServiceUnavailable);
            if(url==ModelCatalog.V2Endpoint)return indexUnavailable?new(HttpStatusCode.ServiceUnavailable):new(HttpStatusCode.OK){Content=new StringContent(File.ReadAllText(Path.Combine(fixtures,staleIndex?"stale-v2.json":"fixture-v2.json")))};
            if(url.Contains("/releases?"))
            {
                var releases=JsonSerializer.Deserialize<List<Release>>(File.ReadAllText(Path.Combine(fixtures,"upstream-releases.json")),ModelCatalog.Json)!;
                if(revisedRelease){var release=releases.Single(r=>r.TagName==selected.Assets.Single(a=>a.Role=="semantic").Release);int index=release.Assets.FindIndex(a=>a.Name=="tinylm.sjtl");release.Assets[index]=release.Assets[index] with{Digest="sha256:"+new string('f',64)};}
                if(newRelease){var semantic=selected.Assets.Single(a=>a.Role=="semantic");releases.Add(new(futureRelease,false,false,[new("tinylm.sjtl",semantic.Size,"sha256:"+semantic.Sha256,$"https://github.com/Nanako0129/shanjie/releases/download/{futureRelease}/tinylm.sjtl")]));}
                return new(HttpStatusCode.OK){Content=new StringContent(JsonSerializer.Serialize(releases,ModelCatalog.Json))};
            }
            if(url.EndsWith("/main/MODEL-CATALOG.json"))return new(HttpStatusCode.OK){Content=new StringContent(JsonSerializer.Serialize(new Compatibility(1,ModelCatalog.Known()),ModelCatalog.Json))};
            var a=files.FirstOrDefault(a=>ModelCatalog.UpstreamRepositories.Any(repository=>url==$"https://github.com/{repository}/releases/download/{a.Release}/{a.Filename}"));
            return a is null?new(HttpStatusCode.NotFound):new(HttpStatusCode.OK){Content=new StreamContent(File.OpenRead(Path.Combine(fixtures,"assets",a.Sha256)))};
        }));
        ModelCatalog.VerifyWith(client);
        try{
            settings.Vm.DictionaryText="ㄇㄠ 貓";settings.Vm.HasChanges=true;string draft=settings.Vm.DictionaryText;
            byte[]? before=File.Exists(ProfileStore.Path)?await File.ReadAllBytesAsync(ProfileStore.Path):null;
            var online=await ModelCatalog.Online(CancellationToken.None);
            Check(online.Rows.Any(r=>r.Spec?.Id==selected.Id&&r.Available&&r.Summary.Contains("退步")),"regression advisory remains downloadable in v2 UI");
            Check(requests.Any(url=>url.Contains("api.github.com/repos/Nanako0129/shanjie/"))&&!requests.Any(url=>url.Contains("/tavricccc/shanjie-upstream-backup/")),"reachable official repository does not query the mirror");
            requests.Clear();mirrorOnly=true;
            var fallback=await ModelCatalog.Online(CancellationToken.None);
            Check(fallback.Rows.Any(r=>r.Spec?.Id==selected.Id&&r.Available)&&fallback.Status.Contains("鏡像"),"official failure uses the mirror release list");
            Check(requests.FindIndex(url=>url.Contains("api.github.com/repos/Nanako0129/shanjie/"))<requests.FindIndex(url=>url.Contains("api.github.com/repos/tavricccc/shanjie-upstream-backup/")),"release list attempts official before mirror");
            requests.Clear();
            var main=selected.Assets.Single(a=>a.Role=="bigram");var classes=selected.Assets.Single(a=>a.Role=="classes");
            var spec=new ModelSpec(selected.Id,selected.Name,main.Release,classes.Release,main.Sha256,classes.Sha256,main.Size,classes.Size,selected);
            var choice=await ModelCatalog.Download(spec,new ProgressLog(_=>{}),CancellationToken.None);
            foreach(var asset in selected.Assets)
                Check(requests.FindIndex(url=>url==$"https://github.com/Nanako0129/shanjie/releases/download/{asset.Release}/{asset.Filename}") is var first&&first>=0&&first<requests.FindIndex(url=>url==$"https://github.com/tavricccc/shanjie-upstream-backup/releases/download/{asset.Release}/{asset.Filename}"),"asset download attempts official before mirror: "+asset.Role);
            Check(ModelCatalog.Installed().Any(m=>m.Id==choice.Id&&m.Catalog?.Advisories.Count>0),"native bundle download retains offline advisory metadata");
            var switched=spec with {Id="alternate-source-fixture",Catalog=selected with {Id="alternate-source-fixture",Assets=selected.Assets.Select(a=>a with {Repository=a.Repository=="Nanako0129/shanjie"?"tavricccc/shanjie-upstream-backup":"Nanako0129/shanjie"}).ToList()}};
            var stored=ModelCatalog.FindInstalled(switched,ModelCatalog.Installed());
            Check(stored?.Id==choice.Id&&!ModelCatalog.HasUpdate(stored.InstalledSpec!,switched),"same model name and files remain downloaded after source and bundle-ID changes");
            requests.Clear();
            var reused=await ModelCatalog.Download(switched,new ProgressLog(_=>{}),CancellationToken.None);
            Check(reused.Id==choice.Id&&requests.Count==0,"switching model source reuses the installed model without downloading");
            var changed=spec with {Catalog=selected with {Assets=selected.Assets.Select(a=>a.Role=="semantic"?a with {Sha256=new string('f',64)}:a).ToList()}};
            Check(ModelCatalog.HasUpdate(stored!.InstalledSpec!,changed),"same-name semantic weight updates are still detected");
            var newer=catalog.Models.First(m=>m.Id!=selected.Id&&ModelCatalog.Supported(m)) with {Name=selected.Name};
            var newerMain=newer.Assets.Single(a=>a.Role=="bigram");var newerClasses=newer.Assets.SingleOrDefault(a=>a.Role=="classes");
            var newerSpec=new ModelSpec(newer.Id,newer.Name,newerMain.Release,newerClasses?.Release??"",newerMain.Sha256,newerClasses?.Sha256??"",newerMain.Size,newerClasses?.Size??0,newer);
            var updateList=await ModelCatalog.Online(CancellationToken.None);
            Check(updateList.Rows.Any(r=>r.Spec?.Id==selected.Id&&r.State=="已下載"&&r.ActionLabel=="選用"),"existing model remains shown as downloaded");
            Check(updateList.Rows.Any(r=>r.Spec?.Id==newer.Id&&r.State=="已下載・可更新"&&r.ActionLabel=="更新"),"same-name new revision is shown as an update");
            var refreshed=await ModelCatalog.Download(newerSpec,new ProgressLog(_=>{}),CancellationToken.None);
            Check(refreshed.Id==newer.Id&&refreshed.Id!=choice.Id,"same-name updated bundle can still be downloaded and published");
            Check(ModelCatalog.FindInstalled(newerSpec,ModelCatalog.Installed())?.Id==refreshed.Id,"a newer installed revision wins over an older same-name model");
            var legacy=new ModelSpec("legacy-update-fixture","舊格式更新驗證","model-v7-fixture","classes-v6-fixture",main.Sha256,classes.Sha256,main.Size,classes.Size);
            var legacyFirst=await ModelCatalog.Download(legacy,new ProgressLog(_=>{}),CancellationToken.None);
            var legacyUpdate=legacy with {ModelRelease="model-v7-fixture-refresh"};
            var legacyUpdated=await ModelCatalog.Download(legacyUpdate,new ProgressLog(_=>{}),CancellationToken.None);
            Check(legacyUpdated.Id!=legacyFirst.Id&&ModelCatalog.FindInstalled(legacyUpdate,ModelCatalog.Installed())?.Id==legacyUpdated.Id,"same-name legacy release updates get a new usable revision");
            mirrorOnly=false;
            var complete=catalog.Models.Single(m=>m.Name=="與內建同版的完整配套");
            var completeSpec=spec with{Id=complete.Id,Name=complete.Name,Catalog=complete};
            var completeChoice=await ModelCatalog.Download(completeSpec,new ProgressLog(_=>{}),CancellationToken.None);
            settings.Vm.RefreshModels(ModelCatalog.Installed());settings.Vm.SelectModel(completeChoice.Id);
            settings.Vm.Status="";settings.ShowModelNotice();
            Check(settings.Vm.Status.Length==0,"a downloaded full SR-8 equivalent does not recommend switching to the built-in set");
            var futureChoice=new ModelChoice(future.Id,future.Name){InstalledSpec=completeSpec with{Id=future.Id,Name=future.Name,Catalog=future}};
            settings.Vm.RefreshModels(ModelCatalog.Installed().Append(futureChoice));settings.Vm.SelectModel(futureChoice.Id);
            settings.Vm.Status="";settings.ShowModelNotice();
            Check(settings.Vm.Status.Length==0,"a newer selected model does not recommend downgrading to the built-in set");
            settings.Vm.RefreshModels(ModelCatalog.Installed());settings.Vm.SelectModel(completeChoice.Id);
            staleIndex=true;newRelease=true;revisedRelease=true;requests.Clear();
            var stale=await ModelCatalog.Online(CancellationToken.None);
            Check(stale.Rows.Any(r=>r.Spec?.Id==complete.Id&&r.Available&&r.State.Contains("與內建相同")),"a stale compatibility index retains the verified full installed model");
            Check(stale.Rows.Any(r=>r.Name==futureRelease&&r.State.Contains("上游新模型")&&!r.Available),"new upstream model releases remain visible before compatibility metadata catches up");
            Check(stale.Rows.Any(r=>r.Name==main.Release)==false&&stale.Rows.Any(r=>r.Name==selected.Assets.Single(a=>a.Role=="semantic").Release&&r.State.Contains("已更新")),"same-tag upstream weight updates are detected while the existing model remains downloaded");
            Check(!requests.Any(url=>url.Contains("/tavricccc/shanjie-upstream-backup/")),"a stale compatibility index does not trigger the mirror while upstream is reachable");
            string indexCache=Path.Combine(ModelCatalog.Folder,"catalog-v2-cache.json");
            File.Move(indexCache,indexCache+".saved");indexUnavailable=true;requests.Clear();
            try{
                var missing=await ModelCatalog.Online(CancellationToken.None);
                Check(missing.Rows.Any(r=>r.Spec?.Id==complete.Id&&r.Available),"verified installed models survive an unavailable compatibility service with no index cache");
                Check(!requests.Any(url=>url.Contains("/tavricccc/shanjie-upstream-backup/")),"an unavailable compatibility service does not redirect a reachable upstream to the mirror");
            }finally{File.Move(indexCache+".saved",indexCache,true);indexUnavailable=false;staleIndex=false;newRelease=false;revisedRelease=false;}
            await ModelCatalog.Online(CancellationToken.None);
            var windowSize=settings.AppWindow.Size;
            settings.Vm.RefreshModels(ModelCatalog.Installed());settings.Vm.SelectModel(choice.Id);settings.ShowModelOptions();await Task.Delay(150);
            var advisoryList=(Microsoft.UI.Xaml.Controls.ItemsControl)settings.FindName("ModelAdvisoryList");
            Check(advisoryList.Items.Count==selected.Advisories.Count,"settings render each model advisory as a separate card");
            var elements=new List<Microsoft.UI.Xaml.DependencyObject>{advisoryList};
            for(int i=0;i<elements.Count;i++)
                for(int child=0;child<Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChildrenCount(elements[i]);child++)
                    elements.Add(Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChild(elements[i],child));
            foreach(var advisory in selected.Advisories)
                Check(elements.OfType<Microsoft.UI.Xaml.Controls.TextBlock>().Any(t=>t.Text==advisory.Title&&t.ActualHeight>0)
                    &&elements.OfType<Microsoft.UI.Xaml.Controls.TextBlock>().Any(t=>t.Text==advisory.Body&&t.ActualHeight>0),"model advisory title and full body are rendered: "+advisory.Title);
            int theme=settings.Vm.Theme;
            foreach(int selectedTheme in new[]{1,2})
            {
                settings.Vm.Theme=selectedTheme;
                advisoryList.StartBringIntoView(new Microsoft.UI.Xaml.BringIntoViewOptions{AnimationDesired=false,VerticalAlignmentRatio=0});await Task.Delay(180);
                var viewport=(Microsoft.UI.Xaml.Controls.ScrollViewer)settings.FindName("ContentScroll");
                var bounds=advisoryList.TransformToVisual(viewport).TransformBounds(new Windows.Foundation.Rect(0,0,advisoryList.ActualWidth,advisoryList.ActualHeight));
                Check(bounds.Top>=-1&&bounds.Bottom<=viewport.ViewportHeight+1,"settings advisory cards fit the visible page: theme "+selectedTheme);
                await VisualVerifier.Capture(settings.CaptureElement,Path.Combine(output,$"model-advisory-cards-{selectedTheme}.png"));
            }
            settings.Vm.Theme=theme;
            foreach(var size in new[]{new Windows.Graphics.SizeInt32(840,680),new Windows.Graphics.SizeInt32(640,560)})
            {
                settings.AppWindow.Resize(size);await Task.Delay(150);
                var showing=settings.OnlineModels();await Task.Delay(250);
                var dialog=Microsoft.UI.Xaml.Media.VisualTreeHelper.GetOpenPopupsForXamlRoot(settings.CaptureElement.XamlRoot)
                    .Select(p=>p.Child).OfType<Microsoft.UI.Xaml.Controls.ContentDialog>().Single();
                try{
                    var content=(Microsoft.UI.Xaml.Controls.ScrollViewer)dialog.Content;
                    var panel=(Microsoft.UI.Xaml.Controls.StackPanel)content.Content;
                    var choices=panel.Children.OfType<Microsoft.UI.Xaml.Controls.ListView>().Single();
                    choices.SelectedIndex=choices.Items.Cast<Microsoft.UI.Xaml.Controls.StackPanel>().ToList()
                        .FindIndex(item=>item.Children.OfType<Microsoft.UI.Xaml.Controls.TextBlock>().First().Text==selected.Name);
                    await Task.Delay(150);dialog.UpdateLayout();
                    content.ChangeView(null,content.ScrollableHeight,null,true);await Task.Delay(150);dialog.UpdateLayout();
                    var notes=(Microsoft.UI.Xaml.FrameworkElement)panel.Children.OfType<Microsoft.UI.Xaml.Controls.ContentControl>().Single().Content;
                    var hint=panel.Children.OfType<Microsoft.UI.Xaml.Controls.TextBlock>().Last();
                    foreach(var text in new[]{notes,hint})
                    {
                        var bounds=text.TransformToVisual(content).TransformBounds(new Windows.Foundation.Rect(0,0,text.ActualWidth,text.ActualHeight));
                        Check(text.ActualHeight>0&&bounds.Bottom<=content.ViewportHeight+1&&bounds.Right<=content.ViewportWidth+1,
                            $"model notes and final hint can be read to the bottom at {size.Width}x{size.Height}: {bounds}");
                    }
                    Check(content.ScrollableHeight>0&&Math.Abs(content.VerticalOffset-content.ScrollableHeight)<1,"long model notes scroll to their full end");
                    await VisualVerifier.Capture(dialog,Path.Combine(output,$"model-notes-{size.Width}.png"));
                }finally{dialog.Hide();await showing;settings.AppWindow.Resize(windowSize);}
            }
            settings.Vm.RefreshModels(ModelCatalog.Installed());string original=settings.Vm.ModelId;int prompts=0;
            await settings.SelectModelWithConfirmation(choice,()=>{prompts++;return Task.FromResult(false);});
            Check(prompts==1&&settings.Vm.ModelId==original&&settings.Vm.DictionaryText==draft&&settings.Vm.HasChanges,"cancel keeps original model and unrelated unsaved draft");
            await settings.SelectModelWithConfirmation(choice,()=>{prompts++;return Task.FromResult(true);});
            Check(prompts==2&&settings.Vm.ModelId==choice.Id,"explicit acceptance selects the model draft");
            Check(before is null?!File.Exists(ProfileStore.Path):(await File.ReadAllBytesAsync(ProfileStore.Path)).SequenceEqual(before),"download and selection do not save settings implicitly");
            using var offline=new HttpClient(new Replies(_=>new(HttpStatusCode.ServiceUnavailable)));
            ModelCatalog.VerifyWith(offline);var cached=await ModelCatalog.Online(CancellationToken.None);
            Check(cached.Rows.Any(r=>r.Spec?.Id==choice.Id&&r.Spec.Catalog?.Advisories.Count>0)&&cached.Status.Contains("快取"),"offline v2 list retains advisories");
            string cache=Path.Combine(ModelCatalog.Folder,"catalog-v2-cache.json");
            string validCache=await File.ReadAllTextAsync(cache);
            try {
                await File.WriteAllTextAsync(cache,"{ invalid cache");
                var installed=ModelCatalog.Installed();settings.Vm.RefreshModels(installed);
                Check(installed.Any(m=>m.Id==choice.Id)&&settings.Vm.ModelId==choice.Id,"valid downloaded bundle survives damaged note cache and list refresh");
                var degraded=await ModelCatalog.Online(CancellationToken.None);
                Check(!string.IsNullOrWhiteSpace(degraded.Status)&&settings.Vm.ModelId==choice.Id,"offline catalog failure does not reset selected bundle");
                settings.Vm.Save();settings.Vm.Load();
                Check(settings.Vm.ModelId==choice.Id,"saved selection survives reload with damaged note cache");
            } finally { await File.WriteAllTextAsync(cache,validCache); }
            Check(ModelCatalog.Installed().Any(m=>m.Id==choice.Id&&m.Catalog?.Advisories.Count>0),"valid cache restores advisory metadata without redownloading");
        }finally{ModelCatalog.VerifyWith(null);}
    }
    private sealed class ProgressLog(Action<string> write):IProgress<string>{public void Report(string text)=>write(text);}
    private sealed class Replies(Func<HttpRequestMessage,HttpResponseMessage> reply):HttpMessageHandler
    {protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request,CancellationToken token)=>Task.FromResult(reply(request));}
    private sealed class Slow:Stream
    {
        private bool sent;public override bool CanRead=>true;public override bool CanSeek=>false;public override bool CanWrite=>false;
        public override long Length=>16384;public override long Position{get=>0;set=>throw new NotSupportedException();}
        public override async ValueTask<int> ReadAsync(Memory<byte> buffer,CancellationToken token=default){if(!sent){sent=true;buffer.Span[..4096].Fill(1);return 4096;}await Task.Delay(Timeout.Infinite,token);return 0;}
        public override int Read(byte[] b,int o,int c)=>throw new NotSupportedException();public override void Flush()=>throw new NotSupportedException();
        public override long Seek(long o,SeekOrigin s)=>throw new NotSupportedException();public override void SetLength(long v)=>throw new NotSupportedException();public override void Write(byte[] b,int o,int c)=>throw new NotSupportedException();
    }
    internal static async Task Run(MainWindow settings,string output,string assets)
    {
        if(Environment.GetEnvironmentVariable("SHANJIE_TEST_PROFILE")!="1")throw new InvalidDataException("Isolated profile required");
        settings.AppWindow.Move(new Windows.Graphics.PointInt32(-12000,-12000));settings.AppWindow.Show(false);
        Directory.CreateDirectory(ModelCatalog.Folder);
        string log=Path.Combine(output,"models.log");void Note(string s)=>File.AppendAllText(log,s+Environment.NewLine);
        void Check(bool condition,string what){if(!condition)throw new InvalidDataException(what);Note("PASS "+what);}
        settings.Vm.DictionaryText="ㄇㄠ 貓";settings.Vm.HasChanges=true;string draft=settings.Vm.DictionaryText;
        byte[]? before=File.Exists(ProfileStore.Path)?await File.ReadAllBytesAsync(ProfileStore.Path):null;
        var catalog=await ModelCatalog.Online(CancellationToken.None);
        string classV4=ModelCatalog.Known().Single(x=>x.Id=="model-v6-classes-v4").ClassesSha256;
        Check(catalog.Rows.Any(x=>x.Spec?.ClassesSha256==classV4&&x.Available),"live GitHub assets and digests identify the v4 pair");
        Check(catalog.Rows.Any(x=>x.Spec?.Id==ModelCatalog.Builtin&&!x.Available),"built-in model is already available without a redundant download");
        Note(catalog.Status);
        using(var offline=new HttpClient(new Replies(_=>new(HttpStatusCode.ServiceUnavailable))))
        {
            ModelCatalog.VerifyWith(offline);
            var cached=await ModelCatalog.Online(CancellationToken.None);
            Check(cached.Rows.Any(x=>x.Spec?.ClassesSha256==classV4&&x.Available)&&cached.Status.Contains("快取"),"offline catalog uses the last successful metadata");
            ModelCatalog.VerifyWith(null);
        }
        var v4=ModelCatalog.Known().Single(x=>x.Id=="model-v6-classes-v4");
        string cache=Path.Combine(ModelCatalog.Folder,"Assets");Directory.CreateDirectory(cache);
        File.Copy(Path.Combine(assets,"bigram-v6.sjlm"),Path.Combine(cache,v4.ModelSha256),true);
        // Resume the actual public class download, sharing the already verified main-model asset.
        using(var source=File.OpenRead(Path.Combine(assets,"classes-v4.sjc")))
        using(var partial=File.Create(Path.Combine(cache,v4.ClassesSha256+".download"))) {var head=new byte[4096];source.ReadExactly(head);partial.Write(head);}
        var selected=await ModelCatalog.Download(v4,new ProgressLog(Note),CancellationToken.None);
        Check(selected.Id==v4.Id&&ModelCatalog.Installed().Any(x=>x.Id==v4.Id),"real resumed download and native map equivalence publish the package");
        settings.Vm.RefreshModels(ModelCatalog.Installed());settings.Vm.SelectModel(v4.Id);
        Check(settings.Vm.ModelId==v4.Id&&settings.Vm.DictionaryText==draft&&settings.Vm.HasChanges,"model selection preserves the unrelated unsaved dictionary draft");
        Check(before is null?!File.Exists(ProfileStore.Path):(await File.ReadAllBytesAsync(ProfileStore.Path)).SequenceEqual(before),"download and selection do not save the profile implicitly");
        settings.Vm.Preset(true);Check(settings.Vm.ModelId==v4.Id,"operation preset keeps a user-selected model id");
        using(var slow=new HttpClient(new Replies(_=>new(HttpStatusCode.OK){Content=new StreamContent(new Slow())})))
        using(var stop=new CancellationTokenSource(250))
        {
            ModelCatalog.VerifyWith(slow);bool cancelled=false;
            try{await ModelCatalog.Download(v4 with{Id="cancel-fixture",ModelSize=16384,ModelSha256=new('a',64)},new ProgressLog(Note),stop.Token);}
            catch(OperationCanceledException){cancelled=true;}finally{ModelCatalog.VerifyWith(null);}
            string partial=Path.Combine(cache,new string('a',64)+".download");
            Check(cancelled&&File.Exists(partial)&&new FileInfo(partial).Length<16384&&!Directory.Exists(Path.Combine(ModelCatalog.Folder,"cancel-fixture")),"cancellation retains an incomplete download without publishing a model");
        }
        using(var corrupt=new HttpClient(new Replies(_=>new(HttpStatusCode.OK){Content=new ByteArrayContent(new byte[64])})))
        {
            ModelCatalog.VerifyWith(corrupt);bool refused=false;
            try{await ModelCatalog.Download(v4 with{Id="corrupt-fixture",ModelSize=64,ModelSha256=new('b',64)},new ProgressLog(Note),CancellationToken.None);}
            catch(InvalidDataException){refused=true;}finally{ModelCatalog.VerifyWith(null);}
            Check(refused&&!Directory.Exists(Path.Combine(ModelCatalog.Folder,"corrupt-fixture")),"incorrect SHA-256 never publishes or selects a package");
        }
        var v5=ModelCatalog.Known().Single(x=>x.Id=="model-v5-classes-v3");
        await ModelCatalog.Download(v5,new ProgressLog(Note),CancellationToken.None);
        Check(ModelCatalog.Installed().Any(x=>x.Id==v5.Id),"older official model-v5/classes-v3 parses, maps and validates");
        // A changed package must be rejected even after an earlier successful cache lookup.
        string table=Path.Combine(ModelCatalog.Folder,v4.Id,"classes.sjc");byte[] original=await File.ReadAllBytesAsync(table);
        byte[] changed=(byte[])original.Clone();changed[^1]^=1;await File.WriteAllBytesAsync(table,changed);
        Check(ModelCatalog.Installed().All(x=>x.Id!=v4.Id),"modified assets invalidate the local catalog cache");
        Check(ModelCatalog.Issues().Any(x=>x.Contains(v4.Id)),"invalid local packages have a visible explanation");
        await File.WriteAllBytesAsync(table,original);
        settings.Vm.RefreshModels(ModelCatalog.Installed());settings.Vm.SelectModel(ModelCatalog.Builtin);
        settings.ShowModelOptions();await Task.Delay(350);await VisualVerifier.Capture(settings.CaptureElement,Path.Combine(output,"model-manager.png"));
    }
}
