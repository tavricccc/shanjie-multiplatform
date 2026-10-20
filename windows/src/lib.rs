mod bridge;
pub mod model_store;
pub mod broker;
pub mod candidate;
mod dictionary_store;
mod word_store;
mod word_popup;
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

pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
