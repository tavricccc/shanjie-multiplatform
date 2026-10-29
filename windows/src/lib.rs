mod bridge;
#[cfg(test)]mod mixed_regression;
#[cfg(feature = "input-diagnostics")]
pub mod input_diagnostics;
pub mod model_store;
pub mod broker;
pub mod candidate;
mod dictionary_store;
mod word_store;
mod word_popup;
mod word_gesture;
pub mod word_helper;
mod selection_text;
mod selection_notice;
pub mod engine;
pub mod input;
pub mod installation;
pub mod installer;
pub mod native;
mod pipe_io;
pub mod process_wait;
pub mod profile;
pub mod registration;
pub mod setup_bundle;
mod registry_snapshot;
mod tsf;
mod runtime_log;

pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
