using System.Text.Json;
namespace Shanjie_UI.Models;

internal sealed record Contributor(string Login, string Name, int Contributions, string Type, string Summary, string Avatar, string AvatarSource = "", string Role = "")
{
    public Uri Profile => new($"https://github.com/{Login}");
    public Uri Commits => new($"https://github.com/tavricccc/shanjie-multiplatform/commits?author={Login}");
    public Uri Portrait => new($"ms-appx:///Assets/Contributors/{Avatar}");
}
internal sealed record ContributorCatalog(string UpdatedAt, string Repository, string Revision, List<Contributor> Contributors);

internal static class Contributors
{
    private static readonly HashSet<string> Automation = new(StringComparer.OrdinalIgnoreCase)
        { "github", "github-actions", "claude", "claude-ai", "claude-code", "codex" };
    internal static readonly ContributorCatalog Catalog;

    static Contributors()
    {
        using var stream = typeof(Contributors).Assembly.GetManifestResourceStream("About.contributors.json")!;
        using var reader = new StreamReader(stream);
        Catalog = Parse(reader.ReadToEnd());
    }

    internal static ContributorCatalog Parse(string json)
    {
        var catalog = JsonSerializer.Deserialize<ContributorCatalog>(json, new JsonSerializerOptions(JsonSerializerDefaults.Web))!;
        return catalog with { Contributors = catalog.Contributors
            .Where(person => person.Type == "User" && !person.Login.EndsWith("[bot]", StringComparison.OrdinalIgnoreCase)
                && !Automation.Contains(person.Login))
            .OrderByDescending(person => person.Role.Length > 0).ThenByDescending(person => person.Contributions).ThenBy(person => person.Login, StringComparer.OrdinalIgnoreCase).ToList() };
    }
}
