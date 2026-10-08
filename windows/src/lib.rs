mod bridge;
pub mod candidate;
pub mod engine;
pub mod input;
pub mod installer;
pub mod native;
pub mod profile;
pub mod registration;
mod tsf;

pub fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
