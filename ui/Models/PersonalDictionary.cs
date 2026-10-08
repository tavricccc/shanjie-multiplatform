using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
namespace Shanjie_UI.Models;
internal static class PersonalDictionary
{
    public static string Save(string text, string folder)
    {
        var rows = new List<string>();
        foreach (string line in text.Split('\n').Select(row => row.Trim()).Where(row => row.Length > 0))
        {
            var fields = line.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
            if (fields.Length is not (2 or 3) || !Regex.IsMatch(fields[0], "^[ㄅ-ㄩˊˇˋ˙]+(-[ㄅ-ㄩˊˇˋ˙]+)*$") || fields[0].Split('-').Length != fields[1].EnumerateRunes().Count())
                throw new InvalidDataException("個人詞庫格式有誤：每行填「注音 字詞」，每個字的注音以 - 分隔。");
            string score = fields.Length == 3 ? fields[2] : "-0.01";
            if (!double.TryParse(score, NumberStyles.Float, CultureInfo.InvariantCulture, out double value) || !double.IsFinite(value) || value > 0)
                throw new InvalidDataException("詞庫分數必須是有限的非正數。");
            rows.Add($"{fields[0]}\t{fields[1]}\t{score}");
        }
        var content = string.Join("\n", rows) + (rows.Count > 0 ? "\n" : "");
        string name = "dictionary-" + Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(content))).ToLowerInvariant() + ".tsv";
        Directory.CreateDirectory(folder); string path = Path.Combine(folder, name);
        if (!File.Exists(path)) { File.WriteAllText(path + ".tmp", content, new UTF8Encoding(false)); File.Move(path + ".tmp", path); }
        return name;
    }
}
