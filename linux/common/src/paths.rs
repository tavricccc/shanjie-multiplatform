//! Where the Linux build keeps the user's files, and reading profile.ini with the Windows rules
//! (windows/src/profile.rs `load`). One folder holds what %LOCALAPPDATA%\Shanjie holds on Windows:
//! profile.ini, the personal dictionary, learning and downloaded models.
use shanjie_shell::profile::{Document, Result};
use std::path::{Path, PathBuf};

pub const TEMPLATE: &str = include_str!("../../../profiles/windows.ini");

/// $XDG_DATA_HOME/shanjie, else ~/.local/share/shanjie.
pub fn folder() -> PathBuf {
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(data).join("shanjie");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
    home.join(".local/share/shanjie")
}

pub fn path() -> PathBuf {
    folder().join("profile.ini")
}

/// The installed runtime data, or SHANJIE_DATA for a development build.
pub fn data() -> PathBuf {
    std::env::var_os("SHANJIE_DATA").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/usr/share/shanjie/data"))
}

pub fn load(path: &Path) -> Result<Document> {
    if !path.exists() {
        return Document::parse(TEMPLATE);
    }
    let original = std::fs::read_to_string(path)?;
    let mut doc = Document::parse(&original)?;
    // Reading never persists a migration; only the settings app writes the file.
    doc.migrate();
    Ok(doc)
}

/// windows/src/model_store.rs `resolve`: the built-in model, or a downloaded one in Models/<id>.
pub fn model(data: &Path, id: &str) -> PathBuf {
    let builtin = data.join("models/model-v6/bigram.winmap");
    if id.is_empty() || id == shanjie_shell::profile::BUILTIN {
        return builtin;
    }
    let valid = id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.') && !id.starts_with('.');
    let folder = folder().join("Models").join(id);
    if valid && folder.join("bigram.winmap").is_file() && folder.join("classes.sjc").is_file() {
        return folder.join("bigram.winmap");
    }
    // Keep the selected id in settings; an unavailable package must not disable typing.
    builtin
}
