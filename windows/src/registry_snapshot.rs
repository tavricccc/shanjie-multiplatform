//! Exact registry checkpoints for the installer; unrelated startup values stay intact.
use crate::{profile::Result, wide};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::*,
        Security::{OBJECT_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR},
        System::Registry::*,
    },
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Value {
    pub kind: u32,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tree {
    pub values: BTreeMap<String, Value>,
    pub children: BTreeMap<String, Tree>,
    pub security: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub user: bool,
    pub view: u32,
    pub path: String,
    pub value: Option<String>,
}
impl Location {
    fn root(&self) -> HKEY {
        if self.user {
            HKEY_CURRENT_USER
        } else {
            HKEY_LOCAL_MACHINE
        }
    }
    fn view(&self) -> REG_SAM_FLAGS {
        REG_SAM_FLAGS(self.view)
    }
    pub fn tree(user: bool, view: REG_SAM_FLAGS, path: impl Into<String>) -> Self {
        Self {
            user,
            view: view.0,
            path: path.into(),
            value: None,
        }
    }
    pub fn value(user: bool, view: REG_SAM_FLAGS, path: impl Into<String>, name: &str) -> Self {
        Self {
            user,
            view: view.0,
            path: path.into(),
            value: Some(name.into()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    pub location: Location,
    pub tree: Option<Tree>,
}
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
unsafe fn open(root: HKEY, path: &str, access: REG_SAM_FLAGS) -> Result<Option<Key>> {
    let mut handle = HKEY::default();
    let status = RegOpenKeyExW(
        root,
        PCWSTR(wide(path).as_ptr()),
        Some(0),
        access,
        &mut handle,
    );
    if status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND {
        return Ok(None);
    }
    status.ok()?;
    Ok(Some(Key(handle)))
}
unsafe fn create(root: HKEY, path: &str, access: REG_SAM_FLAGS) -> Result<Key> {
    let mut handle = HKEY::default();
    RegCreateKeyExW(
        root,
        PCWSTR(wide(path).as_ptr()),
        Some(0),
        None,
        REG_OPTION_NON_VOLATILE,
        access,
        None,
        &mut handle,
        None,
    )
    .ok()?;
    Ok(Key(handle))
}
unsafe fn read_tree(handle: HKEY) -> Result<Tree> {
    let mut values = BTreeMap::new();
    let mut children = BTreeMap::new();
    let mut index = 0;
    loop {
        let mut name = vec![0u16; 16384];
        let mut length = name.len() as u32;
        let mut kind = 0;
        let mut size = 0;
        let status = RegEnumValueW(
            handle,
            index,
            Some(PWSTR(name.as_mut_ptr())),
            &mut length,
            None,
            Some(&mut kind),
            None,
            Some(&mut size),
        );
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        status.ok()?;
        let mut bytes = vec![0u8; size as usize];
        length = name.len() as u32;
        RegEnumValueW(
            handle,
            index,
            Some(PWSTR(name.as_mut_ptr())),
            &mut length,
            None,
            Some(&mut kind),
            Some(bytes.as_mut_ptr()),
            Some(&mut size),
        )
        .ok()?;
        bytes.truncate(size as usize);
        values.insert(
            String::from_utf16(&name[..length as usize])?,
            Value { kind, bytes },
        );
        index += 1;
    }
    index = 0;
    loop {
        let mut name = vec![0u16; 512];
        let mut length = name.len() as u32;
        let status = RegEnumKeyExW(
            handle,
            index,
            Some(PWSTR(name.as_mut_ptr())),
            &mut length,
            None,
            None,
            None,
            None,
        );
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        status.ok()?;
        let name = String::from_utf16(&name[..length as usize])?;
        let key = open(handle, &name, KEY_READ)?.ok_or("登錄子項讀取失敗。")?;
        children.insert(name, read_tree(key.0)?);
        index += 1;
    }
    let mut length = 0;
    let info = OBJECT_SECURITY_INFORMATION(7);
    let status = RegGetKeySecurity(handle, info, None, &mut length);
    if status != ERROR_INSUFFICIENT_BUFFER {
        status.ok()?;
    }
    let mut security = vec![0u8; length as usize];
    RegGetKeySecurity(
        handle,
        info,
        Some(PSECURITY_DESCRIPTOR(security.as_mut_ptr().cast())),
        &mut length,
    )
    .ok()?;
    security.truncate(length as usize);
    Ok(Tree {
        values,
        children,
        security,
    })
}
pub fn capture(location: Location) -> Result<Saved> {
    let mut tree = unsafe {
        open(location.root(), &location.path, KEY_READ | location.view())?
            .map(|key| read_tree(key.0))
            .transpose()?
    };
    if let (Some(name), Some(tree)) = (&location.value, &mut tree) {
        tree.values.retain(|key, _| key == name);
        tree.children.clear();
    }
    Ok(Saved { location, tree })
}
unsafe fn remove_key(root: HKEY, path: &str, view: REG_SAM_FLAGS) -> Result<()> {
    let Some(key) = open(root, path, KEY_ALL_ACCESS | view)? else {
        return Ok(());
    };
    RegDeleteTreeW(key.0, None).ok()?;
    drop(key);
    let status = RegDeleteKeyExW(root, PCWSTR(wide(path).as_ptr()), view.0, Some(0));
    if status != ERROR_FILE_NOT_FOUND {
        status.ok()?;
    }
    Ok(())
}
unsafe fn write_tree(handle: HKEY, tree: &Tree) -> Result<()> {
    // Reconcile surviving keys in place, retaining their ACL and owner.
    let actual = read_tree(handle)?;
    for name in actual
        .values
        .keys()
        .filter(|name| !tree.values.contains_key(*name))
    {
        RegDeleteValueW(handle, PCWSTR(wide(name).as_ptr())).ok()?;
    }
    for name in actual
        .children
        .keys()
        .filter(|name| !tree.children.contains_key(*name))
    {
        remove_key(handle, name, REG_SAM_FLAGS(0))?;
    }
    for (name, value) in &tree.values {
        RegSetValueExW(
            handle,
            PCWSTR(wide(name).as_ptr()),
            Some(0),
            REG_VALUE_TYPE(value.kind),
            Some(&value.bytes),
        )
        .ok()?;
    }
    for (name, child) in &tree.children {
        let key = create(handle, name, KEY_ALL_ACCESS)?;
        write_tree(key.0, child)?;
    }
    if !tree.security.is_empty() {
        RegSetKeySecurity(
            handle,
            OBJECT_SECURITY_INFORMATION(7),
            PSECURITY_DESCRIPTOR(tree.security.as_ptr().cast_mut().cast()),
        )
        .ok()?;
    }
    Ok(())
}
pub fn restore(saved: &Saved) -> Result<()> {
    unsafe {
        let location = &saved.location;
        if let Some(name) = &location.value {
            let value = saved.tree.as_ref().and_then(|tree| tree.values.get(name));
            if let Some(value) = value {
                let key = create(
                    location.root(),
                    &location.path,
                    KEY_ALL_ACCESS | location.view(),
                )?;
                RegSetValueExW(
                    key.0,
                    PCWSTR(wide(name).as_ptr()),
                    Some(0),
                    REG_VALUE_TYPE(value.kind),
                    Some(&value.bytes),
                )
                .ok()?;
            } else if let Some(key) = open(
                location.root(),
                &location.path,
                KEY_SET_VALUE | location.view(),
            )? {
                let status = RegDeleteValueW(key.0, PCWSTR(wide(name).as_ptr()));
                if status != ERROR_FILE_NOT_FOUND {
                    status.ok()?;
                }
            }
            if saved.tree.is_none() {
                let parent = capture(Location::tree(
                    location.user,
                    location.view(),
                    &location.path,
                ))?;
                if parent
                    .tree
                    .as_ref()
                    .is_some_and(|tree| tree.values.is_empty() && tree.children.is_empty())
                {
                    remove_key(location.root(), &location.path, location.view())?;
                }
            }
        } else if let Some(tree) = &saved.tree {
            let key = create(
                location.root(),
                &location.path,
                KEY_ALL_ACCESS | location.view(),
            )?;
            write_tree(key.0, tree)?;
        } else {
            remove_key(location.root(), &location.path, location.view())?;
        }
    }
    let actual = capture(saved.location.clone())?;
    if actual.tree != saved.tree && saved.location.value.is_none() {
        return Err(format!("登錄回復校驗失敗：{}", saved.location.path).into());
    }
    if let Some(name) = &saved.location.value {
        if actual.tree.as_ref().and_then(|tree| tree.values.get(name))
            != saved.tree.as_ref().and_then(|tree| tree.values.get(name))
        {
            return Err("啟動登錄回復校驗失敗。".into());
        }
    }
    Ok(())
}
