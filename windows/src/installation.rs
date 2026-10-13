//! Machine installation choices are separate from everyday input preferences.
use crate::{
    profile::Result,
    registry_snapshot::{self, Location, Saved},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::HMODULE,
        System::{
            LibraryLoader::{
                GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            },
            Registry::*,
        },
    },
};
pub const STANDARD: &str = "{AC59A613-F813-4DAD-BA97-D221A87A6ACE}";
pub const ETEN: &str = "{338145B4-047A-4564-A0DE-223DC22C1AE1}";
pub fn user_sid() -> Result<String> {
    Ok(crate::broker::user_sid()?)
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Installed {
    pub version: String,
    #[serde(default)]
    pub keyboards: Option<u32>,
}
pub fn load(target: &Path) -> Result<Option<Installed>> {
    let path = target.join("installed.json");
    if !path.try_exists()? {
        return Ok(None);
    }
    let value: Installed = serde_json::from_slice(&fs::read(path)?)?;
    let mut components = Path::new(&value.version).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
        || !matches!(value.keyboards, None | Some(1..=3))
    {
        return Err("已安裝配置紀錄有誤，請先修復安裝。".into());
    }
    Ok(Some(value))
}
pub fn tips(mask: u32) -> String {
    [(1, STANDARD), (2, ETEN)]
        .iter()
        .filter(|(bit, _)| mask & bit != 0)
        .map(|(_, id)| format!("0x0404:{}{id}", crate::installer::SERVICE))
        .collect::<Vec<_>>()
        .join(";")
}
pub fn registered(view: REG_SAM_FLAGS) -> Result<u32> {
    let saved = registry_snapshot::capture(Location::tree(
        false,
        view,
        format!(
            "SOFTWARE\\Microsoft\\CTF\\TIP\\{}\\LanguageProfile\\0x00000404",
            crate::installer::SERVICE
        ),
    ))?;
    Ok(mask(&saved))
}
pub fn mask(saved: &Saved) -> u32 {
    saved.tree.as_ref().map_or(0, |tree| {
        [(1, STANDARD), (2, ETEN)]
            .iter()
            .filter(|(_, id)| tree.children.keys().any(|key| key.eq_ignore_ascii_case(id)))
            .map(|(bit, _)| bit)
            .sum()
    })
}
pub fn listed(saved: &Saved) -> u32 {
    fn visit(tree: &registry_snapshot::Tree) -> u32 {
        let here = tree.values.keys().fold(0, |mask, key| {
            let key = key.to_uppercase();
            mask | if key.contains(crate::installer::SERVICE) {
                if key.contains(STANDARD) {
                    1
                } else if key.contains(ETEN) {
                    2
                } else {
                    0
                }
            } else {
                0
            }
        });
        tree.children
            .values()
            .fold(here, |mask, child| mask | visit(child))
    }
    saved.tree.as_ref().map_or(0, visit)
}
pub fn available() -> u32 {
    fn file() -> Option<PathBuf> {
        let mut module = HMODULE::default();
        let mut path = vec![0u16; 32768];
        unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                PCWSTR(available as *const u16),
                &mut module,
            )
            .ok()?;
            let count = GetModuleFileNameW(Some(module), &mut path);
            if count == 0 {
                return None;
            }
            Some(PathBuf::from(String::from_utf16_lossy(
                &path[..count as usize],
            )))
        }
    }
    if let Some(file) = file() {
        for folder in file.ancestors().skip(1).take(5) {
            if let Ok(Some(state)) = load(folder) {
                if let Some(mask) = state.keyboards {
                    return mask;
                }
            }
        }
    }
    match (registered(KEY_WOW64_64KEY), registered(KEY_WOW64_32KEY)) {
        (Ok(a), Ok(b)) if a | b != 0 => a & b,
        _ => 3, // Unregistered portable verification has no installation restriction.
    }
}
pub fn keyboard(preference: u32, mask: u32) -> u32 {
    if preference == 1 && mask & 1 == 0 || preference == 2 && mask & 2 == 0 {
        0
    } else {
        preference
    }
}
pub fn layout(preference: u32, active: u32, mask: u32) -> u32 {
    if mask == 0 {
        return active;
    }
    match keyboard(preference, mask) {
        2 => 1,
        0 => {
            if active == 1 && mask & 2 != 0 {
                1
            } else if mask & 1 == 0 {
                1
            } else {
                0
            }
        }
        _ => 0,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_preferences_fall_back_without_changing_shared_features() {
        assert_eq!(keyboard(2, 1), 0);
        assert_eq!(layout(2, 1, 1), 0);
        assert_eq!(keyboard(1, 2), 0);
        assert_eq!(layout(1, 0, 2), 1);
        assert_eq!(layout(0, 1, 3), 1);
        assert_eq!(layout(0, 0, 3), 0);
        for preference in [3, 4] {
            assert_eq!(keyboard(preference, 1), preference);
            assert_eq!(keyboard(preference, 2), preference);
            assert_eq!(layout(preference, 1, 2), 0);
        }
        assert!(tips(1).contains(STANDARD) && !tips(1).contains(ETEN));
        assert!(tips(2).contains(ETEN) && !tips(2).contains(STANDARD));
    }
}
