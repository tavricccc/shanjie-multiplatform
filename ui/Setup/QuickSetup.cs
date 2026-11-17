using System.Security.Principal;
using Shanjie_UI.Interop;
using Shanjie_UI.Models;
namespace Shanjie_UI.Setup;

/// <summary>
/// Whether the settings app opens with Quick Setup. The marker is a file beside profile.ini, so it belongs to this
/// Windows user and survives upgrades; like update-auto.txt it stays out of the profile, which presets rewrite.
/// It is written when the flow first shows, so skipping or closing it counts as seen. Removing the settings removes it too.
/// </summary>
internal static class QuickSetupGate
{
    internal const string OpenArgument = "--quick-setup", BypassArgument = "--no-quick-setup", BypassVariable = "SHANJIE_NO_QUICK_SETUP";
    internal static string MarkerPath => Path.Combine(Path.GetDirectoryName(ProfileStore.Path)!, "quick-setup.txt");
    /// Only a person's own, unelevated settings launch shows it: a test profile or an elevated process is an automated
    /// or administrative run, whose folder may not even be that person's.
    internal static bool Decide(bool marker, bool elevated, bool bypass, bool testProfile) => !marker && !elevated && !bypass && !testProfile;
    internal static bool Bypassed(string[] arguments) => arguments.Contains(BypassArgument) || Environment.GetEnvironmentVariable(BypassVariable) == "1";
    internal static bool Due(string[] arguments) => Decide(File.Exists(MarkerPath), Elevated(), Bypassed(arguments), Environment.GetEnvironmentVariable("SHANJIE_TEST_PROFILE") == "1");
    internal static void MarkShown()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(MarkerPath)!);
            File.WriteAllText(MarkerPath, $"shown {typeof(QuickSetupGate).Assembly.GetName().Version} {DateTime.UtcNow:O}\n");
        }
        // Without the file it shows again next time, which is better than failing to open the settings.
        catch (Exception error) when (error is IOException or UnauthorizedAccessException) { }
    }
    static bool Elevated() { using var identity = WindowsIdentity.GetCurrent(); return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator); }
}

internal enum QuickTemplate { Keep, Shanjie, Windows }
internal enum SpaceChoice { Insert, OpenList, Commit, Custom }
internal enum MixChoice { Automatic, RawKeys, Off }

/// <summary>
/// What Quick Setup would save. With a template it is a fresh profile: that template file as it ships, then the few
/// <see cref="Carried"/> keys from the saved profile, then the person's choices; nothing else from the old file comes
/// along. "Keep my settings" is the saved profile plus only the keys the person changed. Nothing is written until
/// <see cref="Apply"/>; the file it replaces is copied once to <see cref="BackupPath"/> first, and keeping without a
/// change writes nothing. Dictionary and learning files are separate and never touched.
/// </summary>
internal sealed class QuickSetupDraft
{
    /// Carried into a template: what a template never decides and what would hurt to lose — the chosen language
    /// model, the word packs, the chat-app list, the personal dictionary file and the keyboard layout (a person on
    /// 倚天 could not type on the standard one). A subset of `PERSONAL` in shared/src/profile.rs.
    internal static readonly (string Section, string Key)[] Carried = [
        ("engine", "model_id"), ("engine", "model_revision"), ("engine", "acg_pack"), ("engine", "it_pack"), ("engine", "beauty_pack"), ("engine", "quote_pack"),
        ("engine", "chat_apps"), ("input", "dictionary"), ("input", "keyboard")];
    internal static string BackupPath => ProfileStore.Path + ".before-quick-setup.bak";
    readonly string baseline;
    readonly ProfileDocument saved;
    readonly Dictionary<(string Section, string Key), object> choices = [];
    bool wrote;
    internal QuickSetupDraft()
    {
        HadSettings = File.Exists(ProfileStore.Path);
        baseline = ProfileStore.Load().ToString();
        saved = ProfileDocument.Parse(baseline);
        Template = HadSettings ? QuickTemplate.Keep : QuickTemplate.Windows;
    }
    /// A settings file exists: the person may have tuned it, so the flow starts from keeping it.
    internal bool HadSettings { get; }
    internal QuickTemplate Template { get; private set; }
    /// Another template means other starting values, so earlier detail choices go, except learning, which is asked the same way under each.
    internal void Choose(QuickTemplate template)
    {
        if (template == Template) return;
        Template = template;
        foreach (var key in choices.Keys.Where(k => k.Key != "learning").ToList()) choices.Remove(key);
    }
    internal Models.ProfileDocument Current
    {
        get
        {
            ProfileDocument document;
            if (Template == QuickTemplate.Keep) document = ProfileDocument.Parse(baseline);
            else { document = ProfileStore.Defaults(Template == QuickTemplate.Windows); document.PreserveFrom(saved, Carried); }
            foreach (var (place, value) in choices) document.Set(place.Section, place.Key, value);
            return document;
        }
    }
    internal bool Changed => !Lines(Current.ToString()).SequenceEqual(Lines(baseline));
    static IEnumerable<string> Lines(string text) => text.Split('\n').Select(line => line.Trim()).Where(line => line.Length > 0).Order(StringComparer.Ordinal);
    /// Saves the result; going back and changing it rewrites it from the same starting point. Returns whether the file
    /// now holds the flow's result rather than what was there before.
    internal bool Apply()
    {
        bool changed = Template != QuickTemplate.Keep || Changed;
        if (!changed && !wrote) return false;
        var document = Current;
        // The settings page makes the same correction when it loads a keyboard that is no longer installed.
        uint installed = Native.sj_installed_keyboards();
        int keyboard = document.Number("input", "keyboard", 0);
        if ((keyboard == 1 && (installed & 1) == 0) || (keyboard == 2 && (installed & 2) == 0)) document.Set("input", "keyboard", 0);
        // One copy of the file as it was before this run; a second save in the same run must not overwrite it with the first.
        if (!wrote && File.Exists(ProfileStore.Path)) File.Copy(ProfileStore.Path, BackupPath, overwrite: true);
        ProfileStore.Save(document.ToString());
        wrote = true;
        return changed;
    }
    void Set(string section, string key, object value) => choices[(section, key)] = value;

    internal bool WindowsInteraction => Current.Read("profile", "interaction", "windows") == "windows";
    internal bool AnchorRight => Current.Read("candidates", "anchor", "right") == "right";
    internal bool EnterCommits => Current.Flag("candidates", "enter_commits", WindowsInteraction);
    internal bool SpaceAfterCommit => Current.Flag("input", "space_after_commit", true);
    int Keyboard => Current.Number("input", "keyboard", 0);
    internal bool IsZhuyin => Keyboard < 3;
    /// The upstream judgement works on the standard layout only (`installation::layout` in keys.rs); 跟隨配置 follows what is installed.
    internal bool SupportsAutomatic => Keyboard == 1 || (Keyboard == 0 && (Native.sj_installed_keyboards() & 1) != 0);

    internal SpaceChoice Space
    {
        get
        {
            var d = Current;
            bool operates = d.Flag("candidates", "space_operates", false), commits = d.Flag("input", "space_commits", false);
            if (!operates) return commits ? SpaceChoice.Commit : SpaceChoice.Insert;
            return d.Read("candidates", "open", "down") != "down" && d.Flag("candidates", "space_pages", true) ? SpaceChoice.OpenList : SpaceChoice.Custom;
        }
        set
        {
            if (value == SpaceChoice.Custom) return;
            Set("candidates", "space_operates", value == SpaceChoice.OpenList);
            Set("input", "space_commits", value == SpaceChoice.Commit);
            if (value != SpaceChoice.OpenList) return;
            Set("candidates", "space_pages", true);
            if (Current.Read("candidates", "open", "down") == "down") Set("candidates", "open", "both");
        }
    }
    static readonly string[] RightActions = ["none", "page", "expand"];
    internal int RightAction { get => Math.Max(0, Array.IndexOf(RightActions, Current.Read("candidates", "right_action", "none"))); set => Set("candidates", "right_action", RightActions[value]); }
    internal MixChoice Mix
    {
        get
        {
            // One choice; raw-key mixing wins over upstream English, as Document::migrate stores it.
            var d = Current;
            return d.Flag("input", "mixed_input", false) ? MixChoice.RawKeys : d.Flag("input", "upstream_english", false) ? MixChoice.Automatic : MixChoice.Off;
        }
        set
        {
            if (value is not (MixChoice.Automatic or MixChoice.RawKeys)) return;
            Set("input", "mixed_input", value == MixChoice.RawKeys);
            Set("input", "upstream_english", value == MixChoice.Automatic);
        }
    }
    /// `Document::reading_panel`: raw-key mixing forces the panel at read time and leaves the stored choice alone.
    internal bool PanelForced => IsZhuyin && Current.Flag("input", "mixed_input", false);
    internal bool PanelChoice { get => Current.Flag("candidates", "reading_panel", false); set => Set("candidates", "reading_panel", value); }
    internal bool PanelShown => PanelForced || PanelChoice;
    internal bool ShiftToggle { get => Current.Flag("shortcuts", "shift_toggle", true); set => Set("shortcuts", "shift_toggle", value); }
    internal bool Learning { get => Current.Flag("input", "learning", true); set => Set("input", "learning", value); }
}
