//! Exact registry checkpoints for the installer; unrelated startup values stay intact.
use crate::{profile::Result, wide};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::*,
        Security::{
            Authorization::{SetSecurityInfo, SE_REGISTRY_KEY},
            GetSecurityDescriptorControl, GetSecurityDescriptorDacl, GetSecurityDescriptorGroup,
            GetSecurityDescriptorOwner, OBJECT_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
        },
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
    if !tree.security.is_empty() && actual.security != tree.security {
        restore_security(handle, &tree.security)?;
    }
    Ok(())
}
unsafe fn restore_security(handle: HKEY, bytes: &[u8]) -> Result<()> {
    let descriptor = PSECURITY_DESCRIPTOR(bytes.as_ptr().cast_mut().cast());
    let mut control = 0;
    let mut revision = 0;
    GetSecurityDescriptorControl(descriptor, &mut control, &mut revision)?;
    // RegSetKeySecurity uses the legacy inheritance model and clears AI.
    // Never call it for an unchanged descriptor. Restore modern ACLs through
    // SetSecurityInfo so Windows preserves automatic inheritance.
    let protection = if control & 0x1000 != 0 {
        0x80000000
    } else {
        0x20000000
    };
    if control & 0x0400 == 0 {
        RegSetKeySecurity(
            handle,
            OBJECT_SECURITY_INFORMATION(7 | protection),
            descriptor,
        )
        .ok()?;
    } else {
        let mut owner = PSID::default();
        let mut group = PSID::default();
        let mut defaulted = windows::core::BOOL::default();
        GetSecurityDescriptorOwner(descriptor, &mut owner, &mut defaulted)?;
        GetSecurityDescriptorGroup(descriptor, &mut group, &mut defaulted)?;
        let mut present = windows::core::BOOL::default();
        let mut dacl = std::ptr::null_mut();
        GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted)?;
        if !present.as_bool() {
            return Err("無法回復缺少 DACL 的登錄權限。".into());
        }
        SetSecurityInfo(
            HANDLE(handle.0),
            SE_REGISTRY_KEY,
            OBJECT_SECURITY_INFORMATION(7 | protection),
            Some(owner),
            Some(group),
            Some(dacl),
            None,
        )
        .ok()?;
    }
    Ok(())
}
// Use legacy records only as evidence for the single AI bit. Data, ownership,
// access rules and key names always come from the current live capture.
pub fn inheritance_plan(current: &Saved, original: &Saved) -> (Saved, usize) {
    fn visit(current: &mut Tree, old: &Tree) -> usize {
        let mut count = 0;
        if current.security.len() >= 4 && old.security.len() >= 4 {
            let old_control = u16::from_le_bytes([old.security[2], old.security[3]]);
            let mut legacy = old.security.clone();
            legacy[3] &= !4;
            if old_control & 0x400 != 0
                && legacy == current.security
                && old.security != current.security
            {
                current.security = old.security.clone();
                count += 1;
            }
        }
        for (name, child) in &mut current.children {
            if let Some(old) = old.children.get(name) {
                count += visit(child, old);
            }
        }
        count
    }
    let mut proposed = current.clone();
    let count = if current.location == original.location {
        match (&mut proposed.tree, &original.tree) {
            (Some(current), Some(old)) => visit(current, old),
            _ => 0,
        }
    } else {
        0
    };
    (proposed, count)
}
pub fn repair_inheritance(plan: &Saved) -> Result<()> {
    unsafe fn visit(handle: HKEY, tree: &Tree) -> Result<()> {
        let actual = read_tree(handle)?;
        if actual.security != tree.security {
            let mut legacy = tree.security.clone();
            if legacy.len() < 4 {
                return Err("權限繼承紀錄有誤。".into());
            }
            legacy[3] &= !4;
            if legacy != actual.security {
                return Err("權限已被其他操作修改，未覆寫現有存取規則。".into());
            }
            restore_security(handle, &tree.security)?;
        }
        for (name, child) in &tree.children {
            let key = open(handle, name, KEY_ALL_ACCESS)?.ok_or("權限修復期間登錄項目已變更。")?;
            visit(key.0, child)?;
        }
        Ok(())
    }
    if let Some(tree) = &plan.tree {
        unsafe {
            let key = open(
                plan.location.root(),
                &plan.location.path,
                KEY_ALL_ACCESS | plan.location.view(),
            )?
            .ok_or("原安裝登錄項目已不存在。")?;
            visit(key.0, tree)?;
        }
    }
    let observed = capture(plan.location.clone())?;
    // Repair never writes registry data, and only the planned metadata may differ.
    if observed != *plan {
        return Err("舊版權限繼承修復校驗失敗。".into());
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
#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(String);
    impl Drop for Fixture {
        fn drop(&mut self) {
            unsafe {
                let _ = remove_key(HKEY_CURRENT_USER, &self.0, KEY_WOW64_64KEY);
            }
        }
    }
    unsafe fn control(bytes: &[u8]) -> u16 {
        let mut control = 0;
        let mut revision = 0;
        GetSecurityDescriptorControl(
            PSECURITY_DESCRIPTOR(bytes.as_ptr().cast_mut().cast()),
            &mut control,
            &mut revision,
        )
        .unwrap();
        control
    }
    #[test]
    #[ignore = "Uses an isolated HKCU Software key with no Windows configuration role"]
    fn native_registry_inheritance_and_partial_write_round_trip() {
        assert!(
            std::env::var_os("SHANJIE_TEST_ROOT").is_some(),
            "Use an isolated workspace test root"
        );
        let name = format!(
            "Software\\ShanjieInstallerTest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let _fixture = Fixture(name.clone());
        let location = Location::tree(true, KEY_WOW64_64KEY, &name);
        unsafe {
            let root = create(HKEY_CURRENT_USER, &name, KEY_ALL_ACCESS | KEY_WOW64_64KEY).unwrap();
            RegSetValueExW(
                root.0,
                PCWSTR(wide("version").as_ptr()),
                Some(0),
                REG_DWORD,
                Some(&1u32.to_le_bytes()),
            )
            .ok()
            .unwrap();
            let child = create(root.0, "profile", KEY_ALL_ACCESS).unwrap();
            RegSetValueExW(
                child.0,
                PCWSTR(wide("enabled").as_ptr()),
                Some(0),
                REG_DWORD,
                Some(&1u32.to_le_bytes()),
            )
            .ok()
            .unwrap();
            drop(child);
            // RegCreateKeyEx may create a legacy descriptor even under a modern
            // parent. Convert this isolated subtree to the observed 0.5.1 model.
            let initial = read_tree(root.0).unwrap();
            let descriptor = PSECURITY_DESCRIPTOR(initial.security.as_ptr().cast_mut().cast());
            let mut present = windows::core::BOOL::default();
            let mut defaulted = windows::core::BOOL::default();
            let mut dacl = std::ptr::null_mut();
            GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted).unwrap();
            SetSecurityInfo(
                HANDLE(root.0 .0),
                SE_REGISTRY_KEY,
                OBJECT_SECURITY_INFORMATION(4 | 0x20000000),
                None,
                None,
                Some(dacl),
                None,
            )
            .ok()
            .unwrap();
            let original = capture(location.clone()).unwrap();
            assert_ne!(
                control(&original.tree.as_ref().unwrap().security) & 0x400,
                0
            );
            // Partial registration writes values and recreates a profile key.
            RegSetValueExW(
                root.0,
                PCWSTR(wide("version").as_ptr()),
                Some(0),
                REG_DWORD,
                Some(&2u32.to_le_bytes()),
            )
            .ok()
            .unwrap();
            remove_key(root.0, "profile", REG_SAM_FLAGS(0)).unwrap();
            let child = create(root.0, "profile", KEY_ALL_ACCESS).unwrap();
            RegSetValueExW(
                child.0,
                PCWSTR(wide("wrong").as_ptr()),
                Some(0),
                REG_DWORD,
                Some(&9u32.to_le_bytes()),
            )
            .ok()
            .unwrap();
            drop(child);
            restore(&original).unwrap();
            assert_eq!(capture(location.clone()).unwrap(), original);
            // Reproduce 0.5.2's exact native ACL failure, then repair it.
            RegSetKeySecurity(
                root.0,
                OBJECT_SECURITY_INFORMATION(7),
                PSECURITY_DESCRIPTOR(
                    original
                        .tree
                        .as_ref()
                        .unwrap()
                        .security
                        .as_ptr()
                        .cast_mut()
                        .cast(),
                ),
            )
            .ok()
            .unwrap();
            assert_eq!(
                control(&capture(location.clone()).unwrap().tree.unwrap().security) & 0x400,
                0
            );
            restore(&original).unwrap();
            assert_eq!(capture(location.clone()).unwrap(), original);
            // Existing legacy descriptors must also remain byte-for-byte intact.
            RegSetKeySecurity(
                root.0,
                OBJECT_SECURITY_INFORMATION(7),
                PSECURITY_DESCRIPTOR(
                    original
                        .tree
                        .as_ref()
                        .unwrap()
                        .security
                        .as_ptr()
                        .cast_mut()
                        .cast(),
                ),
            )
            .ok()
            .unwrap();
            let legacy = capture(location.clone()).unwrap();
            RegSetValueExW(
                root.0,
                PCWSTR(wide("version").as_ptr()),
                Some(0),
                REG_DWORD,
                Some(&3u32.to_le_bytes()),
            )
            .ok()
            .unwrap();
            restore(&legacy).unwrap();
            assert_eq!(capture(location.clone()).unwrap(), legacy);
            let (plan, count) = inheritance_plan(&legacy, &original);
            assert!(count > 0);
            repair_inheritance(&plan).unwrap();
            assert_eq!(capture(location.clone()).unwrap(), plan);
            assert_eq!(
                plan.tree.as_ref().unwrap().values,
                legacy.tree.as_ref().unwrap().values
            );
            let mut changed = legacy.clone();
            changed.tree.as_mut().unwrap().security.push(0);
            assert_eq!(inheritance_plan(&changed, &original).1, 0);
        }
        println!("PASS real Windows registry: unchanged AI, partial writes, recreated profile, 0.5.2 legacy-API failure, repair and legacy descriptor preservation");
    }
}
