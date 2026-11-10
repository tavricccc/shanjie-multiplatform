namespace Shanjie_UI.Verification;
/// <summary>A fresh settings process with a saved downloaded model; scripts/verify-ui.ps1 prepares the isolated profile and runs it twice.</summary>
internal static class ModelStartupVerifier
{
    internal static string Id => Environment.GetEnvironmentVariable("SHANJIE_MODEL_STARTUP_ID") ?? "fixture-model";
    internal static async Task Run(MainWindow settings, string phase)
    {
        void Check(bool ok, string what)
        {
            if (!ok) throw new InvalidDataException($"{phase}: {what}: draft={settings.Vm.ModelId}, shown={settings.ShownModelForVerification}, changes={settings.Vm.HasChanges}");
        }
        Check(settings.Vm.ModelId == Id, "the saved model was not loaded");
        settings.AppWindow.Move(new Windows.Graphics.PointInt32(-12000, -12000)); settings.AppWindow.Show(false);
        // The first scan hashes the model, so this also covers the time before the list is ready.
        var until = DateTime.UtcNow.AddSeconds(90);
        while (settings.Vm.AvailableModels.All(m => m.Id != Id))
        {
            Check(settings.Vm.ModelId == Id && !settings.Vm.HasChanges, "start-up changed the draft before the list was ready");
            if (DateTime.UtcNow > until) throw new InvalidDataException($"{phase}: the model was not listed: {settings.Vm.Status}");
            await Task.Delay(20);
        }
        await Task.Delay(200);
        Check(settings.ShownModelForVerification == Id && settings.Vm.ModelId == Id, "the saved model is not shown");
        Check(!settings.Vm.HasChanges, "start-up left unsaved changes");
        if (phase != "first-open") {
            Check(settings.Vm.AccentMode==2 && settings.Vm.AccentColor=="#42D6A4" && settings.Vm.PureBlack && settings.Vm.Theme==2 && settings.Vm.AnimationMode==1,"appearance did not survive a fresh settings process");
        }
        settings.Vm.RefreshModels(settings.Vm.AvailableModels.Where(m => m.Id != Models.ModelCatalog.Builtin).ToList());
        await Task.Delay(100);
        Check(settings.ShownModelForVerification == Id && !settings.Vm.HasChanges, "a rescan changed the selection");
        if (phase == "first-open") {
            settings.Vm.AccentMode=2; settings.Vm.AccentColor="#42D6A4";settings.Vm.PureBlack=true;settings.Vm.Theme=2;settings.Vm.AnimationMode=1;
        }
        // Saving an unrelated setting keeps the model.
        settings.Vm.Demote = !settings.Vm.Demote;
        Check(settings.Vm.HasChanges, "the unrelated change was not noticed");
        settings.Vm.Save();
        Check(File.ReadAllLines(Models.ProfileStore.Path).Contains("model_id=" + Id), "saving lost the model");
    }
}
