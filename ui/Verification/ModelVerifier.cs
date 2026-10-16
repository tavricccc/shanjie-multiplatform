using Shanjie_UI.Models;
using System.Net;
using System.Net.Http;
using System.Security.Cryptography;
namespace Shanjie_UI.Verification;
internal static class ModelVerifier
{
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
        Check(catalog.Rows.Any(x=>x.Spec?.Id=="model-v6-classes-v4"&&x.Available),"live GitHub assets and digests identify the v4 pair");
        Check(catalog.Rows.Any(x=>x.Spec?.Id==ModelCatalog.Builtin&&!x.Available),"built-in model is already available without a redundant download");
        Note(catalog.Status);
        using(var offline=new HttpClient(new Replies(_=>new(HttpStatusCode.ServiceUnavailable))))
        {
            ModelCatalog.VerifyWith(offline);
            var cached=await ModelCatalog.Online(CancellationToken.None);
            Check(cached.Rows.Any(x=>x.Spec?.Id=="model-v6-classes-v4"&&x.Available)&&cached.Status.Contains("快取"),"offline catalog uses the last successful metadata");
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
