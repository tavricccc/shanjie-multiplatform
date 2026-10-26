using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Shanjie_UI.Models;
using System.Diagnostics;
namespace Shanjie_UI;
public sealed partial class MainWindow
{
    private bool selectingModel;
    private async void ModelSelectionChanged(object sender,SelectionChangedEventArgs args)
    {
        if(selectingModel||ModelSelector.SelectedItem is not ModelChoice choice||choice.Id==Vm.ModelId)return;
        selectingModel=true;
        try
        {
            bool accepted=choice.Id==ModelCatalog.Builtin||choice.Catalog is {Advisories.Count:0};
            if(!accepted)
            {
                var text=new TextBlock{Text=ModelCatalog.Notes(choice.Catalog),TextWrapping=TextWrapping.Wrap,MaxWidth=520};
                var dialog=new ContentDialog{XamlRoot=Root.XamlRoot,Title="選用模型前請確認",Content=new ScrollViewer{Content=text,MaxHeight=420},
                    PrimaryButtonText="仍然選用",CloseButtonText="取消",DefaultButton=ContentDialogButton.Close};
                accepted=await dialog.ShowAsync()==ContentDialogResult.Primary;
            }
            if(accepted)Vm.SelectModel(choice.Id);
            else ModelSelector.SelectedIndex=Vm.AvailableModels.ToList().FindIndex(m=>m.Id==Vm.ModelId);
        }
        finally{selectingModel=false;}
    }
    private FileSystemWatcher? modelWatcher;
    private CancellationTokenSource? modelScan;
    private CancellationTokenSource? modelOperation;
    private bool modelsClosed;
    private string modelIssues="";
    private async Task RefreshModels()
    {
        var inventory=await Task.Run(()=>new {Models=ModelCatalog.Installed(),Issues=ModelCatalog.Issues()});
        if(!modelsClosed){Vm.RefreshModels(inventory.Models);modelIssues=string.Join("\n",inventory.Issues);if(modelIssues.Length>0){Vm.Status="未列入模型：\n"+modelIssues;ShowLoadFeedback();}}
    }
    private async void InitializeModels(object sender,RoutedEventArgs args)
    {
        Root.Loaded-=InitializeModels;
        try
        {
            await RefreshModels();Directory.CreateDirectory(ModelCatalog.Folder);
            modelWatcher=new(ModelCatalog.Folder,"*"){IncludeSubdirectories=true,EnableRaisingEvents=true};
            modelWatcher.Changed+=ModelsChanged;modelWatcher.Created+=ModelsChanged;modelWatcher.Deleted+=ModelsChanged;
            modelWatcher.Renamed+=ModelsChanged;
        }
        catch(Exception error) when(error is IOException or UnauthorizedAccessException or InvalidDataException)
        {Vm.Status="模型清單無法更新，內建模型仍可使用："+error.Message;ShowLoadFeedback();}
    }
    private void ModelsChanged(object sender,FileSystemEventArgs args)
    {
        string name=Path.GetFileName(args.FullPath);
        if(name is not ("model.json" or "bigram.winmap" or "classes.sjc") && !string.Equals(Path.GetDirectoryName(args.FullPath),ModelCatalog.Folder,StringComparison.OrdinalIgnoreCase))return;
        DispatcherQueue.TryEnqueue(async()=>{
            if(modelsClosed)return;modelScan?.Cancel();modelScan=new();var token=modelScan.Token;
            try{await Task.Delay(700,token);await RefreshModels();}catch(OperationCanceledException){}
            catch(Exception error)when(error is IOException or UnauthorizedAccessException or InvalidDataException){Vm.Status="模型資料有變動，請重新掃描："+error.Message;}
        });
    }
    private async void RescanModels(object sender,RoutedEventArgs args)
    {
        try{await RefreshModels();if(modelIssues.Length==0)Vm.Status="已重新掃描；只有通過驗證的模型會列入選單。";}
        catch(Exception error)when(error is IOException or UnauthorizedAccessException or InvalidDataException){Vm.Status=error.Message;}
        ShowLoadFeedback();
    }
    private void OpenModelsFolder(object sender,RoutedEventArgs args)
    {
        try{Directory.CreateDirectory(ModelCatalog.Folder);Process.Start(new ProcessStartInfo(ModelCatalog.Folder){UseShellExecute=true});}
        catch(Exception error)when(error is IOException or UnauthorizedAccessException or System.ComponentModel.Win32Exception){Vm.Status="無法開啟模型資料夾："+error.Message;ShowLoadFeedback();}
    }
    private async void OnlineModels(object sender,RoutedEventArgs args)
    {
        if(modelOperation is not null)return;
        using var cancel=new CancellationTokenSource();modelOperation=cancel;
        var info=new TextBlock {Text="正在取得線上清單…",TextWrapping=TextWrapping.Wrap};
        var progress=new ProgressBar {IsIndeterminate=true};
        var choices=new ListView {MaxHeight=260,SelectionMode=ListViewSelectionMode.Single};
        var notes=new TextBlock{TextWrapping=TextWrapping.Wrap,MaxWidth=520};
        var panel=new StackPanel {Spacing=12,MinWidth=420};panel.Children.Add(choices);panel.Children.Add(progress);panel.Children.Add(info);
        panel.Children.Add(new ScrollViewer{Content=notes,MaxHeight=240});
        panel.Children.Add(new TextBlock {Text="只列相容配套供下載。完成後選入設定草稿，按主視窗「儲存」才套用；個人詞庫與學習資料保留。",TextWrapping=TextWrapping.Wrap});
        var dialog=new ContentDialog {XamlRoot=Root.XamlRoot,Title="取得其他模型",Content=panel,PrimaryButtonText="下載並選取",SecondaryButtonText="取消下載",CloseButtonText="關閉",IsPrimaryButtonEnabled=false,IsSecondaryButtonEnabled=false};
        List<CatalogRow> rows=[];bool busy=false,closeRequested=false;
        choices.SelectionChanged+=(_,_)=>{
            var row=choices.SelectedIndex>=0?rows[choices.SelectedIndex]:null;
            notes.Text=row is null?"":ModelCatalog.Notes(row.Spec?.Catalog);
            dialog.PrimaryButtonText=row?.Spec?.Catalog is {Advisories.Count:0}?"下載並選取":"仍然選用並下載";
            dialog.IsPrimaryButtonEnabled=!busy&&row?.Available==true;
        };
        dialog.Closing+=(_,e)=>{if(busy){e.Cancel=true;closeRequested=true;cancel.Cancel();}else cancel.Cancel();};
        dialog.SecondaryButtonClick+=(_,e)=>{e.Cancel=true;cancel.Cancel();info.Text="正在取消；下載片段會保留供下次續傳。";};
        dialog.PrimaryButtonClick+=async(_,e)=>{
            e.Cancel=true;if(choices.SelectedIndex<0||busy)return;
            var row=rows[choices.SelectedIndex];if(row.Spec is null||!row.Available)return;
            busy=true;choices.IsEnabled=false;dialog.IsPrimaryButtonEnabled=false;dialog.IsSecondaryButtonEnabled=true;progress.IsIndeterminate=true;
            try
            {
                var selected=await ModelCatalog.Download(row.Spec,new Progress<string>(s=>info.Text=s),cancel.Token);
                await RefreshModels();Vm.SelectModel(selected.Id);Vm.Status="模型已下載、驗證並選取，按儲存後套用。其他設定草稿已保留。";ShowLoadFeedback();
                busy=false;dialog.Hide();
            }
            catch(OperationCanceledException){info.Text="已取消，原模型保持不變；重新開啟清單可續傳。";}
            catch(Exception error)when(error is IOException or UnauthorizedAccessException or InvalidDataException or System.Net.Http.HttpRequestException)
            {info.Text="尚未套用模型："+error.Message;}
            finally{busy=false;choices.IsEnabled=true;dialog.IsSecondaryButtonEnabled=false;progress.IsIndeterminate=false;if(closeRequested)dialog.Hide();else dialog.IsPrimaryButtonEnabled=!cancel.IsCancellationRequested;}
        };
        var shown=dialog.ShowAsync();
        try
        {
            var result=await ModelCatalog.Online(cancel.Token);rows=result.Rows;
            foreach(var row in rows)choices.Items.Add(row.Name+" — "+row.State);
            info.Text=result.Status;progress.IsIndeterminate=false;
        }
        catch(OperationCanceledException){}
        catch(Exception error)when(error is IOException or UnauthorizedAccessException or InvalidDataException or System.Net.Http.HttpRequestException or System.Text.Json.JsonException){info.Text="無法取得線上清單："+error.Message;progress.IsIndeterminate=false;}
        try{await shown;}finally{modelOperation=null;}
    }
}
