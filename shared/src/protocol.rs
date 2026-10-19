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
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Failure {
    Busy,
    Disconnected,
    Timeout,
    Ambiguous,
    Failed,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_classes")]
    pub classes: u32,
    pub layout: u32,
    pub ranking: u32,
    pub right: bool,
    pub learning: bool,
    pub big5: bool,
    pub dictionary: String,
    pub prediction: bool,
    // Word pack switch; absent from older senders, which always had no packs on the wire.
    #[serde(default = "default_acg")]
    pub acg: bool,
}
fn default_classes() -> u32 { 5 }
fn default_acg() -> bool { true }
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
}
