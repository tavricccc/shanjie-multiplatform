//! The user's input languages as Windows keeps them under `Control Panel\International\User
//! Profile`. Shanjie adds and removes only its own TIPs through InstallLayoutOrTip, which appends
//! them (and the language when missing); it never rewrites the rest of the list.
use super::*;
use crate::registry_snapshot::{self, Tree};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Language {
    pub tag: String,
    pub tips: Vec<String>,
}
pub fn bit(tip: &str) -> u32 {
    let tip = tip.to_uppercase();
    for (bit, id) in [
        (1, crate::installation::STANDARD),
        (2, crate::installation::ETEN),
    ] {
        if tip == format!("0404:{}{id}", SERVICE) {
            return bit;
        }
    }
    0
}
pub fn mask(languages: &[Language]) -> u32 {
    languages
        .iter()
        .flat_map(|l| &l.tips)
        .fold(0, |m, t| m | bit(t))
}
/// A language whose only input methods are Shanjie's: removing them makes Windows drop the
/// language itself.
pub fn sole_language(languages: &[Language]) -> Option<&str> {
    languages
        .iter()
        .find(|l| !l.tips.is_empty() && l.tips.iter().all(|t| bit(t) != 0))
        .map(|l| l.tag.as_str())
}
pub fn read() -> Result<Vec<Language>> {
    let saved = registry_snapshot::capture(transaction::locations()[6].clone())?;
    Ok(saved.tree.as_ref().map_or_else(Vec::new, parse))
}
/// `Languages` (REG_MULTI_SZ) orders the tags; each tag's subkey names its input methods as
/// values such as `0404:{CLSID}{profile}` or `0409:00000409`.
fn parse(tree: &Tree) -> Vec<Language> {
    let order = tree.values.iter().find(|(name, _)| name.eq_ignore_ascii_case("Languages"));
    let tags: Vec<String> = order.map_or_else(Vec::new, |(_, value)| {
        let units: Vec<u16> = value
            .bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        units
            .split(|&unit| unit == 0)
            .filter(|tag| !tag.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    });
    tags.into_iter()
        .map(|tag| {
            let tips = tree
                .children
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&tag))
                .map_or_else(Vec::new, |(_, key)| {
                    key.values.keys().filter(|name| name.contains(':')).cloned().collect()
                });
            Language { tag, tips }
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry_snapshot::Value;
    fn tip(bit: u32) -> String {
        let id = if bit == 1 { crate::installation::STANDARD } else { crate::installation::ETEN };
        format!("0404:{SERVICE}{id}")
    }
    fn key(names: &[&str]) -> Tree {
        Tree {
            values: names
                .iter()
                .map(|n| (n.to_string(), Value { kind: 4, bytes: vec![1, 0, 0, 0] }))
                .collect(),
            children: Default::default(),
            security: Vec::new(),
        }
    }
    #[test]
    fn the_registry_list_is_read_in_order_with_only_input_methods() {
        let mut root = key(&[]);
        let tags: Vec<u8> = "en-US\0zh-Hant-TW\0\0"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        root.values.insert("Languages".into(), Value { kind: 7, bytes: tags });
        let standard = tip(1);
        root.children.insert("zh-Hant-TW".into(), key(&["CachedLanguageName", &standard]));
        root.children.insert("en-US".into(), key(&["CachedLanguageName", "0409:00000409"]));
        root.children.insert("fr-FR".into(), key(&["040C:0000040C"]));
        let languages = parse(&root);
        assert_eq!(languages.iter().map(|l| l.tag.as_str()).collect::<Vec<_>>(), ["en-US", "zh-Hant-TW"]);
        assert_eq!(languages[1].tips, [standard]);
        assert_eq!(mask(&languages), 1);
    }
    #[test]
    fn only_a_language_left_without_other_input_blocks_removal() {
        let mut languages = vec![
            Language { tag: "en-US".into(), tips: vec!["0409:00000409".into()] },
            Language { tag: "zh-Hant-TW".into(), tips: vec![tip(1), "0404:OTHER".into(), tip(2)] },
        ];
        assert_eq!(mask(&languages), 3);
        assert_eq!(sole_language(&languages), None);
        languages[1].tips.retain(|t| bit(t) != 0);
        assert_eq!(sole_language(&languages), Some("zh-Hant-TW"));
        languages[1].tips.clear();
        assert_eq!(sole_language(&languages), None);
    }
}
