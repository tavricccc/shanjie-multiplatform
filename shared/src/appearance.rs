//! Personal appearance policy, independent of a particular Windows renderer.
use crate::profile::Document;

pub const DEFAULT_ACCENT: u32 = 0x5874c9;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Accent {
    #[default]
    Default,
    Windows,
    Custom(u32),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Animation {
    #[default]
    System,
    On,
    Off,
}
impl Animation {
    pub fn enabled(self, system: bool) -> bool {
        match self { Self::System => system, Self::On => true, Self::Off => false }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Appearance {
    pub accent: Accent,
    pub pure_black: bool,
    pub animation: Animation,
}
impl Appearance {
    pub fn from_document(doc: &Document) -> Self {
        Self {
            accent: match doc.get("appearance", "accent", "default") {
                "windows" => Accent::Windows,
                "custom" => Accent::Custom(parse_hex(doc.get("appearance", "accent_color", "#5874C9")).unwrap_or(DEFAULT_ACCENT)),
                _ => Accent::Default,
            },
            pure_black: doc.flag("appearance", "pure_black", false),
            animation: match doc.get("appearance", "animation", "system") {
                "on" => Animation::On,
                "off" => Animation::Off,
                _ => Animation::System,
            },
        }
    }
    pub fn accent(self, windows: u32) -> u32 {
        match self.accent { Accent::Default => DEFAULT_ACCENT, Accent::Windows => windows, Accent::Custom(rgb) => rgb }
    }
}
pub fn parse_hex(value: &str) -> Option<u32> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    (hex.len() == 6 && hex.bytes().all(|c| c.is_ascii_hexdigit())).then(|| u32::from_str_radix(hex, 16).ok()).flatten()
}
fn luminance(rgb: u32) -> f64 {
    [16, 8, 0].into_iter().zip([0.2126, 0.7152, 0.0722]).map(|(shift, weight)| {
        let c = ((rgb >> shift) & 255) as f64 / 255.0;
        weight * if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    }).sum()
}
pub fn contrast(a: u32, b: u32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
pub fn foreground(background: u32) -> u32 {
    if contrast(background, 0xffffff) >= contrast(background, 0) { 0xffffff } else { 0 }
}
/// An accent used as small text must also be readable against the panel.
pub fn text_accent(accent: u32, background: u32) -> u32 {
    let target = foreground(background);
    for step in 0..=100 {
        let mut result = 0;
        for shift in [16, 8, 0] {
            let c = (accent >> shift) & 255;
            let t = (target >> shift) & 255;
            result |= ((c * (100 - step) + t * step) / 100) << shift;
        }
        if contrast(result, background) >= 4.5 { return result; }
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_preserves_defaults_and_personal_choices() {
        assert_eq!(Appearance::from_document(&Document::default()), Appearance::default());
        let doc = Document::parse("[appearance]\naccent=custom\naccent_color=#00FF80\npure_black=1\nanimation=on\n").unwrap();
        let a = Appearance::from_document(&doc);
        assert_eq!(a.accent(0x123456), 0x00ff80);
        assert!(a.pure_black && a.animation.enabled(false));
        assert!(!Animation::Off.enabled(true));
        assert!(!Animation::System.enabled(false));
        assert!(Animation::System.enabled(true));
        assert_eq!(Appearance::from_document(&Document::parse("[appearance]\naccent=windows\n").unwrap()).accent(0x123456), 0x123456);
        for invalid in ["#FFF", "#12345678", "xyzxyz", "#ZZ0000"] { assert_eq!(parse_hex(invalid), None); }
    }
    #[test]
    fn bright_dark_and_saturated_accents_remain_readable() {
        for accent in [0, 0xffffff, 0xffff00, 0x00ff00, 0xff0000, 0x0000ff, DEFAULT_ACCENT] {
            assert!(contrast(accent, foreground(accent)) >= 4.5);
            for background in [0, 0x1c1c20, 0xf8f8fa] { assert!(contrast(text_accent(accent, background), background) >= 4.5); }
        }
    }
}
