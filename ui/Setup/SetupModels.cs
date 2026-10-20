using System.Text.Json;
using CommunityToolkit.Mvvm.ComponentModel;
namespace Shanjie_UI.Setup;

/// <summary>`shanjie-manager install-status`: read unelevated before anything changes.</summary>
internal sealed record SetupStatus(string PackageVersion, string? InstalledVersion, uint? Keyboards, uint Registered64, uint Registered32, uint Listed, string Target)
{
    internal static SetupStatus Parse(string json)
    {
        using var document = JsonDocument.Parse(json);
        var root = document.RootElement;
        static uint? Number(JsonElement root, string name) => root.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.Number ? value.GetUInt32() : null;
        static string? Text(JsonElement root, string name) => root.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.String ? value.GetString() : null;
        return new(Text(root, "package_version") ?? throw new InvalidDataException("安裝狀態缺少版本。"), Text(root, "installed_version"), Number(root, "keyboards"),
            Number(root, "registered64") ?? 0, Number(root, "registered32") ?? 0, Number(root, "listed") ?? 0, Text(root, "target") ?? "");
    }
    internal bool Installed => InstalledVersion is not null;
    /// <summary>The keyboard to preselect: the recorded choice, else what is listed, else standard.</summary>
    internal uint SuggestedKeyboard => Keyboards is 1 or 2 ? Keyboards.Value : Listed == 2 ? 2u : 1u;
    internal string KeyboardSummary =>
        Registered64 != Registered32 ? "目前鍵盤註冊不一致，這次會依你的選擇修復。"
        : Listed == 3 ? "目前清單有標準注音與倚天。"
        : Listed == 1 ? "目前清單有標準注音。"
        : Listed == 2 ? "目前清單有倚天。"
        : "請選擇要安裝的鍵盤配置。";
}

/// <summary>One JSON line from the installer's progress pipe.</summary>
internal sealed record SetupEvent(string Stage, string Status, string Message, int? Done, int? Total)
{
    internal static SetupEvent? Parse(string line)
    {
        try
        {
            using var document = JsonDocument.Parse(line);
            var root = document.RootElement;
            string Text(string name) => root.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.String ? value.GetString() ?? "" : "";
            int? Number(string name) => root.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.Number ? value.GetInt32() : null;
            string stage = Text("stage");
            return stage.Length == 0 ? null : new(stage, Text("status"), Text("message"), Number("done"), Number("total"));
        }
        catch (JsonException) { return null; }
    }
}

internal enum StepState { Pending, Running, Succeeded, Failed }

internal sealed partial class SetupStep(string stage, string label) : ObservableObject
{
    internal string Stage { get; } = stage;
    public string Label { get; } = label;
    private StepState state;
    internal StepState State
    {
        get => state;
        set { if (SetProperty(ref state, value)) { OnPropertyChanged(nameof(Glyph)); OnPropertyChanged(nameof(Opacity)); } }
    }
    // Segoe Fluent Icons: circle, sync, check, error.
    public string Glyph => state switch { StepState.Running => "", StepState.Succeeded => "", StepState.Failed => "", _ => "" };
    public double Opacity => state == StepState.Pending ? 0.5 : 1;
}

internal static class SetupStages
{
    internal static readonly (string Stage, string Label)[] Install =
    [
        ("Verify", "校驗安裝包"), ("Copy", "複製檔案"), ("Checkpoint", "保存目前狀態"),
        ("Register64", "註冊 64 位元輸入法"), ("Register32", "註冊 32 位元輸入法"), ("Profiles", "更新輸入法清單"),
        ("LaunchPaths", "更新啟動路徑"), ("AppEntry", "登記到已安裝的應用程式"), ("Login", "設定登入時啟動核心"),
        ("Validate", "校驗安裝結果"), ("Cleanup", "清除舊版本"), (Runtime, "啟動常駐核心"),
    ];
    internal static readonly (string Stage, string Label)[] Uninstall =
    [
        ("Stop", "停止常駐核心與登入啟動"), ("Unregister", "移除輸入法註冊"), ("Remove", "移除程式檔案"), ("Data", "處理個人資料"),
    ];
    /// <summary>The unelevated follow-up that starts the core once the elevated part reports success.</summary>
    internal const string Runtime = "Runtime";
    internal static string Label(string stage) => stage switch
    {
        "Rollback" => "回復安裝前狀態",
        "LegacyPermissions" => "修復舊版權限",
        _ => Install.Concat(Uninstall).FirstOrDefault(s => s.Stage == stage).Label ?? stage,
    };
}
