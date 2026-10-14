//! Checkpoint every install effect before touching registration or launch paths.
use super::*;
use crate::registry_snapshot::{self, Location, Saved};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Register64,
    Register32,
    Profiles,
    LaunchPaths,
    AppEntry,
    Login,
    Validate,
}
const PHASES: [Phase; 7] = [
    Phase::Register64,
    Phase::Register32,
    Phase::Profiles,
    Phase::LaunchPaths,
    Phase::AppEntry,
    Phase::Login,
    Phase::Validate,
];
pub trait Effects {
    type Checkpoint;
    fn checkpoint(&mut self) -> Result<Self::Checkpoint>;
    fn apply(&mut self, phase: Phase) -> Result<()>;
    fn restore(&mut self, checkpoint: &Self::Checkpoint) -> Result<()>;
    fn failed(&mut self, _phase: Option<Phase>, _message: &str) -> Result<()> {
        Ok(())
    }
    fn succeeded(&mut self, _phase: Phase) -> Result<()> {
        Ok(())
    }
    fn log_directory(&self) -> Option<&Path> {
        None
    }
}
pub fn execute<E: Effects>(effects: &mut E) -> Result<()> {
    let checkpoint = match effects.checkpoint() {
        Ok(saved) => saved,
        Err(error) => {
            let logged = effects.failed(None, &error.to_string());
            return Err(format!(
                "安裝初始化未完成：{error}{}{}",
                log_failure(logged),
                log_path(effects)
            )
            .into());
        }
    };
    for phase in PHASES {
        if let Err(error) = effects.apply(phase).and_then(|_| effects.succeeded(phase)) {
            let logged = effects.failed(Some(phase), &error.to_string());
            let logging = log_failure(logged);
            return match effects.restore(&checkpoint) {
                Ok(()) => Err(format!("安裝未完成，已回復安裝前狀態：{error}{logging}{}",log_path(effects)).into()),
                Err(rollback) => Err(format!(
                    "安裝未完成：{error}；回復仍有失敗項目：{rollback}。相關版本與回復紀錄已保留。{logging}{}",log_path(effects)
                )
                .into()),
            };
        }
    }
    Ok(())
}
fn log_failure(result: Result<()>) -> String {
    result.err().map_or(String::new(), |error| {
        format!("\n另有紀錄保存失敗：{error}")
    })
}
fn log_path<E: Effects>(effects: &E) -> String {
    effects
        .log_directory()
        .map_or(String::new(), |path| format!("\n紀錄：{}", path.display()))
}
#[derive(Clone, Serialize, Deserialize)]
struct FileState {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    registry: Vec<Saved>,
    files: Vec<FileState>,
    languages: language_list::State,
}
pub struct Native<'a> {
    pub target: &'a Path,
    pub package: &'a Path,
    pub binaries: &'a Path,
    pub version: &'a str,
    pub keyboards: u32,
    pub backup: &'a Path,
    pub previous: Option<Checkpoint>,
}
pub fn locations() -> Vec<Location> {
    let mut paths = Vec::new();
    for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
        paths.push(Location::tree(
            false,
            view,
            format!("SOFTWARE\\Classes\\CLSID\\{SERVICE}"),
        ));
        paths.push(Location::tree(
            false,
            view,
            format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{SERVICE}"),
        ));
        paths.push(Location::tree(
            true,
            view,
            format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{SERVICE}"),
        ));
    }
    paths.push(Location::tree(
        true,
        REG_SAM_FLAGS(0),
        "Control Panel\\International\\User Profile",
    ));
    paths.push(Location::tree(false, KEY_WOW64_64KEY, APP_KEY));
    paths.push(Location::value(
        false,
        KEY_WOW64_64KEY,
        RUN_KEY,
        "ShanjieCore",
    ));
    // International settings and legacy TSF notifications maintain these lists.
    for path in [
        "Keyboard Layout\\Preload",
        "Keyboard Layout\\Substitutes",
        "SOFTWARE\\Microsoft\\CTF\\SortOrder",
        "SOFTWARE\\Microsoft\\CTF\\Assemblies",
    ] {
        paths.push(Location::tree(true, REG_SAM_FLAGS(0), path));
    }
    paths
}
pub fn atomic_file(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let temporary = path.with_extension(format!("tmp-{}-{stamp:x}", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = remove_tree(&temporary);
    }
    result
}
fn save_files(target: &Path) -> Result<Vec<FileState>> {
    [
        "installed.json",
        "ui-path.txt",
        "manager-path.txt",
        "settings.cmd",
        "uninstall.cmd",
    ]
    .iter()
    .map(|name| {
        let path = target.join(name);
        safe_tree(&path)?;
        let bytes = if path.try_exists()? {
            Some(fs::read(&path)?)
        } else {
            None
        };
        Ok(FileState { path, bytes })
    })
    .collect()
}
fn restore_file(state: &FileState) -> Result<()> {
    let actual = if state.path.try_exists()? {
        Some(fs::read(&state.path)?)
    } else {
        None
    };
    if actual == state.bytes {
        return Ok(());
    }
    if state.path.exists() && remove_tree(&state.path)? != 0 {
        return Err(format!("啟動檔案仍在使用中：{}", state.path.display()).into());
    }
    if let Some(bytes) = &state.bytes {
        atomic_file(&state.path, bytes)?;
    }
    let actual = if state.path.try_exists()? {
        Some(fs::read(&state.path)?)
    } else {
        None
    };
    if actual != state.bytes {
        return Err("安裝檔案回復校驗失敗。".into());
    }
    Ok(())
}
impl Effects for Native<'_> {
    type Checkpoint = Checkpoint;
    fn checkpoint(&mut self) -> Result<Checkpoint> {
        journal::event(
            self.backup,
            "Checkpoint",
            "started",
            "保存註冊、語言清單及啟動檔案",
        )?;
        let registry = locations()
            .into_iter()
            .map(registry_snapshot::capture)
            .collect::<Result<Vec<_>>>()?;
        let registry = permissions::repair(self.target, self.backup, registry)?;
        let checkpoint = Checkpoint {
            registry,
            files: save_files(self.target)?,
            languages: language_list::read(self.backup)?,
        };
        fs::create_dir_all(self.backup)?;
        atomic_file(
            &self.backup.join("checkpoint.json"),
            &serde_json::to_vec_pretty(&checkpoint)?,
        )?;
        self.previous = Some(checkpoint.clone());
        journal::event(self.backup, "Checkpoint", "succeeded", "")?;
        Ok(checkpoint)
    }
    fn apply(&mut self, phase: Phase) -> Result<()> {
        atomic_file(
            &self.backup.join("phase.txt"),
            format!("{phase:?}").as_bytes(),
        )?;
        journal::event(self.backup, &format!("{phase:?}"), "started", "")?;
        match phase {
            Phase::Register64 | Phase::Register32 => {
                let arch = if phase == Phase::Register64 {
                    "x64"
                } else {
                    "x86"
                };
                // The checkpoint precedes the attempt, including partial regsvr failure.
                regsvr_selected(
                    &self.binaries.join(arch).join("Shanjie.dll"),
                    arch,
                    false,
                    self.keyboards,
                )?;
            }
            Phase::Profiles => {
                let previous = self.previous.as_ref().ok_or("安裝前狀態尚未記錄。")?;
                let listed = language_list::mask(&previous.languages);
                let removed = listed & !self.keyboards;
                if removed != 0 {
                    profiles_selected(removed, true)?;
                }
                profiles_selected(self.keyboards, false)?;
                let desired = language_list::plan(&previous.languages, self.keyboards);
                atomic_file(
                    &self.backup.join("expected-language-list.json"),
                    &serde_json::to_vec_pretty(&desired)?,
                )?;
                language_list::apply(self.backup, &previous.languages, &desired)?;
            }
            Phase::LaunchPaths => {
                atomic_file(
                    &self.target.join("ui-path.txt"),
                    format!("versions\\{}\\ui\\Shanjie.UI.exe", self.version).as_bytes(),
                )?;
                atomic_file(
                    &self.target.join("manager-path.txt"),
                    format!("versions\\{}\\tools\\shanjie-manager.exe", self.version).as_bytes(),
                )?;
                for name in ["settings.cmd", "uninstall.cmd"] {
                    atomic_file(&self.target.join(name), &fs::read(self.package.join(name))?)?;
                }
                let installed = crate::installation::Installed {
                    version: self.version.into(),
                    keyboards: Some(self.keyboards),
                };
                atomic_file(
                    &self.target.join("installed.json"),
                    &serde_json::to_vec_pretty(&installed)?,
                )?;
            }
            Phase::AppEntry => unsafe {
                app_entry(self.target, self.binaries)?;
            },
            Phase::Login => unsafe {
                core_login(Some(&self.binaries.join("tools/shanjie-core.exe")))?;
            },
            Phase::Validate => {
                let previous = self.previous.as_ref().ok_or("安裝前狀態尚未記錄。")?;
                let languages = language_list::read(self.backup)?;
                let expected = language_list::plan(&previous.languages, self.keyboards);
                let registered64 = crate::installation::registered(KEY_WOW64_64KEY)?;
                let registered32 = crate::installation::registered(KEY_WOW64_32KEY)?;
                atomic_file(
                    &self.backup.join("validation.json"),
                    &serde_json::to_vec_pretty(
                        &serde_json::json!({"expected_keyboards":self.keyboards,"registered64":registered64,"registered32":registered32,"expected_languages":expected,"actual_languages":languages,"dll64":registry_dll(KEY_WOW64_64KEY)?,"dll32":registry_dll(KEY_WOW64_32KEY)?}),
                    )?,
                )?;
                for (arch, view) in [("x64", KEY_WOW64_64KEY), ("x86", KEY_WOW64_32KEY)] {
                    let dll = self.binaries.join(arch).join("Shanjie.dll");
                    if registry_dll(view)?.as_ref() != Some(&dll)
                        || crate::installation::registered(view)? != self.keyboards
                        || digest(&dll)? != digest(&self.package.join(arch).join("Shanjie.dll"))?
                    {
                        return Err(format!("{arch} 安裝註冊校驗失敗。").into());
                    }
                }
                if languages != expected {
                    return Err(format!("Windows 語言清單校驗失敗：預期善解配置 {}，實際 {}；完整清單見 validation.json。",self.keyboards,language_list::mask(&languages)).into());
                }
                let state = crate::installation::load(self.target)?.ok_or("安裝配置尚未保存。")?;
                if state.version != self.version || state.keyboards != Some(self.keyboards) {
                    return Err("安裝配置校驗失敗。".into());
                }
                for (name, expected) in [
                    (
                        "ui-path.txt",
                        format!("versions\\{}\\ui\\Shanjie.UI.exe", self.version),
                    ),
                    (
                        "manager-path.txt",
                        format!("versions\\{}\\tools\\shanjie-manager.exe", self.version),
                    ),
                ] {
                    if fs::read_to_string(self.target.join(name))? != expected
                        || !self.target.join(expected).is_file()
                    {
                        return Err("啟動程式路徑校驗失敗。".into());
                    }
                }
                let app =
                    registry_snapshot::capture(Location::tree(false, KEY_WOW64_64KEY, APP_KEY))?;
                if app
                    .tree
                    .as_ref()
                    .and_then(|tree| tree.values.get("DisplayVersion"))
                    .map(|v| v.bytes.clone())
                    != Some(
                        crate::wide(env!("CARGO_PKG_VERSION"))
                            .into_iter()
                            .flat_map(u16::to_le_bytes)
                            .collect(),
                    )
                {
                    return Err("Installed Apps 版本校驗失敗。".into());
                }
                let run = registry_snapshot::capture(Location::value(
                    false,
                    KEY_WOW64_64KEY,
                    RUN_KEY,
                    "ShanjieCore",
                ))?;
                let expected = crate::wide(&format!(
                    "\"{}\" --serve",
                    self.binaries.join("tools/shanjie-core.exe").display()
                ))
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
                if run
                    .tree
                    .as_ref()
                    .and_then(|tree| tree.values.get("ShanjieCore"))
                    .map(|v| &v.bytes)
                    != Some(&expected)
                {
                    return Err("登入啟動路徑校驗失敗。".into());
                }
                atomic_file(&self.backup.join("completed.txt"), b"committed")?;
            }
        }
        Ok(())
    }
    fn restore(&mut self, checkpoint: &Checkpoint) -> Result<()> {
        let mut failures = Vec::new();
        journal::event(self.backup, "Rollback", "started", "")?;
        // Restore available machine profiles before notifying the language service.
        for saved in &checkpoint.registry[..6] {
            if let Err(error) = registry_snapshot::restore(saved) {
                failures.push(format!("{}：{error}", saved.location.path));
            }
        }
        let old_listed = language_list::mask(&checkpoint.languages);
        if old_listed != 0 {
            if let Err(error) = profiles_selected(old_listed, false) {
                failures.push(format!("原清單通知：{error}"));
            }
        }
        let attempted = language_list::plan(&checkpoint.languages, self.keyboards);
        let language_restored =
            match language_list::restore(self.backup, &checkpoint.languages, &attempted) {
                Ok(_) => true,
                Err(error) => {
                    failures.push(format!("原語言清單：{error}"));
                    false
                }
            };
        // Preserve exact registry metadata after the documented service notification.
        for (index, saved) in checkpoint.registry.iter().enumerate().skip(6) {
            if !language_restored && (index == 6 || index >= 9) {
                continue;
            }
            if let Err(error) = registry_snapshot::restore(saved) {
                failures.push(format!("{}：{error}", saved.location.path));
            }
        }
        for file in &checkpoint.files {
            if let Err(error) = restore_file(file) {
                failures.push(error.to_string());
            }
        }
        if language_restored {
            match language_list::read(self.backup) {
                Ok(actual) if actual == checkpoint.languages => {}
                Ok(_) => failures.push("回復後 Windows 語言清單仍與安裝前不同。".into()),
                Err(error) => failures.push(format!("回復後語言清單校驗：{error}")),
            }
        }
        // Do not archive either version unless every rollback check succeeds.
        if failures.is_empty() {
            atomic_file(&self.backup.join("completed.txt"), b"rolled back")?;
            journal::event(self.backup, "Rollback", "succeeded", "")?;
            Ok(())
        } else {
            atomic_file(
                &self.backup.join("rollback-errors.txt"),
                failures.join("\n").as_bytes(),
            )?;
            journal::event(self.backup, "Rollback", "failed", &failures.join("；"))?;
            Err(failures.join("；").into())
        }
    }
    fn failed(&mut self, phase: Option<Phase>, message: &str) -> Result<()> {
        journal::error(
            self.backup,
            &phase.map_or("Checkpoint".into(), |phase| format!("{phase:?}")),
            message,
        )
    }
    fn succeeded(&mut self, phase: Phase) -> Result<()> {
        journal::event(self.backup, &format!("{phase:?}"), "succeeded", "")
    }
    fn log_directory(&self) -> Option<&Path> {
        Some(self.backup)
    }
}
pub fn rolled_back(backup: &Path) -> bool {
    fs::read(backup.join("completed.txt")).ok().as_deref() == Some(b"rolled back")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Clone, Default, PartialEq, Debug)]
    struct State {
        dlls: [Option<String>; 2],
        profiles: [u32; 2],
        categories: [u32; 2],
        listed: Vec<String>,
        ui: Option<String>,
        manager: Option<String>,
        apps: Option<String>,
        run: Option<String>,
        choice: Option<u32>,
    }
    #[derive(Clone, Copy, PartialEq)]
    enum Fault {
        Phase(Phase),
        Com(usize),
        Profile(usize),
        Category(usize, u32),
    }
    struct Fake {
        state: State,
        fail: Option<Fault>,
        restore_fails: bool,
        mask: u32,
        errors: Vec<String>,
    }
    impl Effects for Fake {
        type Checkpoint = State;
        fn checkpoint(&mut self) -> Result<State> {
            Ok(self.state.clone())
        }
        fn apply(&mut self, phase: Phase) -> Result<()> {
            match phase {
                Phase::Register64 | Phase::Register32 => {
                    let arch = if phase == Phase::Register64 { 0 } else { 1 };
                    self.state.dlls[arch] = Some(format!("new{arch}"));
                    if self.fail == Some(Fault::Com(arch)) {
                        return Err("injected after COM write".into());
                    }
                    self.state.profiles[arch] = self.mask;
                    if self.fail == Some(Fault::Profile(arch)) {
                        return Err("injected during profile registration".into());
                    }
                    for category in 0..4 {
                        self.state.categories[arch] |= 1 << category;
                        if self.fail == Some(Fault::Category(arch, category)) {
                            return Err("injected during category registration".into());
                        }
                    }
                }
                Phase::Profiles => {
                    self.state.listed.retain(|s| s != "standard" && s != "eten");
                    self.state
                        .listed
                        .push(if self.mask == 1 { "standard" } else { "eten" }.into());
                }
                Phase::LaunchPaths => {
                    self.state.ui = Some("new ui".into());
                    self.state.manager = Some("new manager".into());
                    self.state.choice = Some(self.mask);
                }
                Phase::AppEntry => self.state.apps = Some("new app".into()),
                Phase::Login => self.state.run = Some("new core".into()),
                Phase::Validate => {}
            }
            if self.fail == Some(Fault::Phase(phase)) {
                Err("injected partial write".into())
            } else {
                Ok(())
            }
        }
        fn restore(&mut self, old: &State) -> Result<()> {
            if self.restore_fails {
                Err("injected rollback failure".into())
            } else {
                self.state = old.clone();
                Ok(())
            }
        }
        fn failed(&mut self, phase: Option<Phase>, message: &str) -> Result<()> {
            self.errors.push(format!("{phase:?}: {message}"));
            Ok(())
        }
    }
    #[test]
    fn every_failed_phase_restores_first_legacy_and_single_layout_installs() {
        for previous in [None, Some(1), Some(2), Some(3)] {
            let old = State {
                dlls: if previous.is_some() {
                    [Some("old64".into()), Some("old32".into())]
                } else {
                    [None, None]
                },
                profiles: [previous.unwrap_or(0); 2],
                categories: if previous.is_some() { [15; 2] } else { [0; 2] },
                listed: {
                    let mut list = vec!["other one".into(), "other two".into()];
                    if previous.unwrap_or(0) & 1 != 0 {
                        list.push("standard".into());
                    }
                    if previous.unwrap_or(0) & 2 != 0 {
                        list.push("eten".into());
                    }
                    list
                },
                choice: previous.filter(|mask| *mask != 3),
                ui: previous.map(|_| "old ui".into()),
                manager: previous.map(|_| "old manager".into()),
                apps: previous.map(|_| "old app".into()),
                run: previous.map(|_| "old core".into()),
            };
            let mut faults = PHASES.into_iter().map(Fault::Phase).collect::<Vec<_>>();
            for arch in 0..2 {
                faults.push(Fault::Com(arch));
                faults.push(Fault::Profile(arch));
                for category in 0..4 {
                    faults.push(Fault::Category(arch, category));
                }
            }
            for mask in [1, 2] {
                for fault in &faults {
                    let mut fake = Fake {
                        state: old.clone(),
                        fail: Some(*fault),
                        restore_fails: false,
                        mask,
                        errors: Vec::new(),
                    };
                    assert!(execute(&mut fake).is_err());
                    assert_eq!(fake.state, old);
                    assert_eq!(fake.errors.len(), 1);
                    assert!(fake.errors[0].contains("injected"));
                }
            }
        }
    }
    #[test]
    fn successful_install_changes_only_requested_membership_and_launchers() {
        for mask in [1, 2] {
            let mut fake = Fake {
                state: State {
                    listed: vec![
                        "other one".into(),
                        "other two".into(),
                        "standard".into(),
                        "eten".into(),
                    ],
                    ..Default::default()
                },
                fail: None,
                restore_fails: false,
                mask,
                errors: Vec::new(),
            };
            execute(&mut fake).unwrap();
            assert_eq!(fake.state.profiles, [mask; 2]);
            assert_eq!(&fake.state.listed[..2], &["other one", "other two"]);
            assert_eq!(fake.state.choice, Some(mask));
        }
    }
    #[test]
    fn failed_rollback_reports_incomplete_state_instead_of_success() {
        let mut fake = Fake {
            state: State::default(),
            fail: Some(Fault::Com(0)),
            restore_fails: true,
            mask: 1,
            errors: Vec::new(),
        };
        let error = execute(&mut fake).unwrap_err().to_string();
        assert!(error.contains("回復仍有失敗"));
        assert_eq!(fake.state.dlls[0].as_deref(), Some("new0"));
        assert!(fake.errors[0].contains("after COM write"));
    }
}
