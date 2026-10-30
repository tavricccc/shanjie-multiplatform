use serde::{Deserialize, Serialize};

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub handled: bool,
    pub associated: bool,
    pub prediction: bool,
    pub reading_hint: bool,
    #[serde(default)]
    pub phonetic_pending: bool,
    pub commit: String,
    pub preedit: String,
    pub cursor: u32,
    pub columns: u32,
    pub first: u32,
    pub total: u32,
    pub selected: Option<usize>,
    pub candidates: Vec<String>,
    /// The core is reranking this output in the background; `Op::Rerank` applies the result once it is ready.
    #[serde(default)]
    pub rerank: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Failure {
    Busy,
    Disconnected,
    Timeout,
    Ambiguous,
    Failed,
}
/// Everything an engine is built with. Fields added later default to upstream's defaults, so an older
/// sender still gets a working engine.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub model: String,
    pub layout: u32,
    pub ranking: u32,
    pub right: bool,
    pub learning: bool,
    pub big5: bool,
    pub dictionary: String,
    pub prediction: bool,
    #[serde(default = "on")]
    pub acg: bool,
    #[serde(default = "on")]
    pub it: bool,
    /// 語意校正, used when the model has a small model beside it.
    #[serde(default = "on")]
    pub rerank: bool,
    /// shanjie_engine_set_rerank_level: 0 conservative, 1 standard, 2 aggressive.
    #[serde(default = "standard")]
    pub rerank_level: u32,
    /// shanjie_engine_set_fuzzy bits: the four sound pairs, then adjacent keys.
    #[serde(default)]
    pub fuzzy: u32,
    #[serde(default)]
    pub english_key_order: bool,
}
fn on() -> bool { true }
fn standard() -> u32 { 1 }
pub const PACK_ACG: u32 = 1;
pub const PACK_IT: u32 = 4;
impl Default for Config {
    /// The built-in model with upstream's defaults, Standard layout, chat profile.
    fn default() -> Self {
        Self {
            model: String::new(),
            layout: 0,
            ranking: 0,
            right: true,
            learning: false,
            big5: false,
            dictionary: "dictionary.tsv".into(),
            prediction: true,
            acg: true,
            it: true,
            rerank: true,
            rerank_level: standard(),
            fuzzy: 0,
            english_key_order: false,
        }
    }
}
impl Config {
    pub fn packs(&self) -> u32 {
        (if self.acg { PACK_ACG } else { 0 }) | (if self.it { PACK_IT } else { 0 })
    }
    /// The engine settings of a profile; the platform decides the layout from its keyboard choice.
    pub fn from_profile(p: &crate::profile::Document, layout: u32) -> Self {
        Self {
            model: p.model_id(),
            layout,
            ranking: p.ranking(None),
            right: p.get("candidates", "anchor", "right") == "right",
            learning: p.flag("input", "learning", true),
            big5: p.flag("candidates", "big5", false),
            dictionary: p.get("input", "dictionary", "dictionary.tsv").to_owned(),
            prediction: p.flag("engine", "prediction", true),
            acg: p.flag("engine", "acg_pack", true),
            it: p.flag("engine", "it_pack", true),
            rerank: p.flag("engine", "rerank", true),
            rerank_level: crate::profile::RERANK_LEVELS.iter().position(|l| *l == p.get("engine", "rerank_level", "standard")).unwrap_or(1) as u32,
            fuzzy: p.fuzzy(),
            english_key_order: p.flag("input", "english_key_order", false),
        }
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    Key {
        kind: u32,
        ch: u32,
        modifiers: u32,
        latin: Option<char>,
    },
    Reset(bool),
    Pick(u32),
    Associate(String),
    Reading,
    Prediction(bool),
    Demote(bool),
    Punctuation(String),
    Cycle,
    Space,
    Context(String),
    Options(u32),
    Mixed(bool),
    Tolerance(bool),
    English(u32),
    /// Applies the rerank of the last output, waiting for it if it is still running. The shell sends it only once the
    /// core says the job is done, so replaying it after a restart gives what was shown.
    Rerank,
}
