using System.Collections.ObjectModel;
using Shanjie_UI.Models;
namespace Shanjie_UI.ViewModels;
public partial class SettingsViewModel
{
    public ObservableCollection<ModelChoice> AvailableModels { get; } = [new(ModelCatalog.Builtin,"model-v6＋classes-v5（內建）")];
    public string ModelId { get; private set; } = ModelCatalog.Builtin;
    public string ModelRevision { get; private set; } = "";
    partial void OnModelVariantChanged(int value)
    {
        if(!loading&&value>=0&&value<AvailableModels.Count) {ModelId=AvailableModels[value].Id;ModelRevision=AvailableModels[value].Revision;OnPropertyChanged(nameof(ModelId));}
    }
    public void RefreshModels(IEnumerable<ModelChoice> models)
    {
        bool previous=loading;loading=true;
        try
        {
            AvailableModels.Clear();AvailableModels.Add(new(ModelCatalog.Builtin,"model-v6＋classes-v5（內建）"));
            foreach(var model in models.Where(m=>m.Id!=ModelCatalog.Builtin).DistinctBy(m=>m.Id))AvailableModels.Add(model);
            ModelVariant=Math.Max(0,AvailableModels.ToList().FindIndex(m=>m.Id==ModelId));
            var selected=AvailableModels.FirstOrDefault(m=>m.Id==ModelId);
            if(selected is not null)ModelRevision=selected.Revision;
            OnPropertyChanged(nameof(ModelHelp));
        }
        finally{loading=previous;}
    }
    public void SelectModel(string id)
    {
        int index=AvailableModels.ToList().FindIndex(m=>m.Id==id);
        if(index<0)throw new InvalidDataException("模型尚未通過驗證，無法選取。");
        ModelId=id;ModelVariant=index;ModelRevision=AvailableModels[index].Revision+"-"+Guid.NewGuid().ToString("N");OnPropertyChanged(nameof(ModelId));OnPropertyChanged(nameof(ModelHelp));HasChanges=true;
    }
}
