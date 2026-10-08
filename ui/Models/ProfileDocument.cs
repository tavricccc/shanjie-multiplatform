using System.Globalization;
using System.Text;
namespace Shanjie_UI.Models;
public sealed class ProfileDocument
{
    private readonly Dictionary<string, Dictionary<string, string>> sections = new(StringComparer.OrdinalIgnoreCase);
    public static ProfileDocument Parse(string content)
    {
        var document = new ProfileDocument(); string section = "";
        foreach (string row in content.Split('\n'))
        {
            string line = row.Trim();
            if (line.Length == 0 || line[0] is ';' or '#') continue;
            if (line[0] == '[' && line[^1] == ']') { section = line[1..^1]; continue; }
            int equals = line.IndexOf('=');
            if (section.Length == 0 || equals <= 0) throw new InvalidDataException("設定檔格式有誤，請確認每項設定都放在 [分類] 之下。");
            document.Set(section, line[..equals].Trim(), line[(equals + 1)..].Trim());
        }
        return document;
    }
    public string Read(string section, string key, string defaultValue) => sections.TryGetValue(section, out var fields) && fields.TryGetValue(key, out var value) ? value : defaultValue;
    public int Number(string section, string key, int defaultValue) => int.Parse(Read(section, key, defaultValue.ToString(CultureInfo.InvariantCulture)), CultureInfo.InvariantCulture);
    public bool Flag(string section, string key, bool defaultValue) => Read(section, key, defaultValue ? "1" : "0") == "1";
    public void Set(string section, string key, object value)
    {
        if (!sections.TryGetValue(section, out var fields)) sections[section] = fields = new(StringComparer.OrdinalIgnoreCase);
        fields[key] = value is bool flag ? flag ? "1" : "0" : Convert.ToString(value, CultureInfo.InvariantCulture)!;
    }
    public void Remove(string section, string key) { if (sections.TryGetValue(section, out var fields)) fields.Remove(key); }
    public void FillMissing(ProfileDocument defaults)
    {
        foreach (var section in defaults.sections)
            foreach (var field in section.Value)
                if (!sections.TryGetValue(section.Key, out var fields) || !fields.ContainsKey(field.Key)) Set(section.Key, field.Key, field.Value);
    }
    public void PreserveFrom(ProfileDocument source, params (string Section, string Key)[] keys)
    {
        foreach (var (section, key) in keys)
            if (source.sections.TryGetValue(section, out var fields) && fields.TryGetValue(key, out var value)) Set(section, key, value);
    }
    public override string ToString()
    {
        var content = new StringBuilder();
        foreach (var section in sections)
        {
            content.AppendLine($"[{section.Key}]");
            foreach (var field in section.Value) content.AppendLine($"{field.Key}={field.Value}");
            content.AppendLine();
        }
        return content.ToString();
    }
}
