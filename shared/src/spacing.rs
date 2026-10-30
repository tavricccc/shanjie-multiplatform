//! 中英文之間自動加空格 (upstream docs/contracts/cjk-spacing.md section 1): a space between a Han character and an ASCII
//! letter or digit, applied only to the text the shell commits.
#[derive(Clone, Copy, PartialEq)]
enum Kind { Han, Ascii, Other }

/// Upstream LeftContext.isHan.
fn is_han(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2A6DF | 0x2A700..=0x2EBEF
        | 0x30000..=0x3134F | 0x2F800..=0x2FA1F | 0x3007)
}
/// A variation selector belongs to the character before it, as in upstream's per-grapheme reading.
fn selector(c: char) -> bool {
    matches!(c as u32, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF)
}
fn kind(c: char) -> Kind {
    if c.is_ascii_alphanumeric() { Kind::Ascii } else if is_han(c) { Kind::Han } else { Kind::Other }
}
/// The character `apply` takes as `previous` from the text before the cursor.
pub fn last(text: &str) -> Option<char> {
    text.chars().rev().find(|c| !selector(*c))
}
/// `previous` is the character before the insertion point, `None` when unknown (no leading space then).
pub fn apply(previous: Option<char>, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut before = previous.map_or(Kind::Other, kind);
    for c in text.chars() {
        if selector(c) {
            out.push(c);
            continue;
        }
        let k = kind(c);
        if matches!((before, k), (Kind::Han, Kind::Ascii) | (Kind::Ascii, Kind::Han)) {
            out.push(' ');
        }
        out.push(c);
        before = k;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contract_examples_and_previous_characters() {
        for (previous, text, want) in [
            (None, "我用claude寫", "我用 claude 寫"),
            (None, "2026下午3點", "2026 下午 3 點"),
            (None, "我用 claude 寫", "我用 claude 寫"),
            (None, "（2026）", "（2026）"),
            (None, "，claude", "，claude"),
            (Some('a'), "你好", " 你好"),
            (Some('7'), "點", " 點"),
            (Some('字'), "ok", " ok"),
            (Some('字'), "好", "好"),
            (Some(' '), "ok", "ok"),
            (Some('，'), "ok", "ok"),
            (None, "ok", "ok"),
            (Some('a'), "\u{20000}b", " \u{20000} b"),
            (None, "，。！", "，。！"),
            (Some('a'), "", ""),
            (None, "葛\u{E0100}a", "葛\u{E0100} a"),
            (None, "ａ字", "ａ字"),
        ] {
            assert_eq!(apply(previous, text), want, "{previous:?} {text}");
        }
        assert_eq!(last("ab葛\u{E0100}"), Some('葛'), "a Han character with a selector is still Han as the previous one");
        assert_eq!(last(""), None);
    }
}
