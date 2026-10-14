//! Windows 10/11 user-language settings: preserve all properties except our TIPs.
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Language {
    pub tag: String,
    pub tips: Vec<String>,
    pub spelling: bool,
    pub handwriting: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub languages: Vec<Language>,
    pub default_tip: Option<String>,
}
#[derive(Serialize)]
struct Request<'a> {
    action: &'a str,
    output: String,
    before: Option<&'a State>,
    desired: Option<&'a State>,
    alternate: Option<&'a State>,
}
static NEXT: AtomicU64 = AtomicU64::new(0);
pub fn bit(tip: &str) -> u32 {
    let tip = tip.to_uppercase();
    for (bit, id) in [
        (1, crate::installation::STANDARD),
        (2, crate::installation::ETEN),
    ] {
        if tip == format!("0404:{}{id}", SERVICE) {
            return bit;
        }
    }
    0
}
pub fn mask(state: &State) -> u32 {
    state
        .languages
        .iter()
        .flat_map(|l| &l.tips)
        .fold(0, |m, t| m | bit(t))
}
fn selected(mask: u32) -> Vec<String> {
    [
        (1, crate::installation::STANDARD),
        (2, crate::installation::ETEN),
    ]
    .into_iter()
    .filter(|(b, _)| mask & b != 0)
    .map(|(_, id)| format!("0404:{SERVICE}{id}"))
    .collect()
}
pub fn plan(before: &State, mask: u32) -> State {
    let mut desired = before.clone();
    let mut inserted = false;
    let selected = selected(mask);
    for language in &mut desired.languages {
        let mut tips = Vec::new();
        for tip in &language.tips {
            if bit(tip) == 0 {
                tips.push(tip.clone());
            } else if !inserted && !selected.is_empty() {
                tips.extend(selected.clone());
                inserted = true;
            }
        }
        language.tips = tips;
    }
    if !inserted && !selected.is_empty() {
        if let Some(language) = desired
            .languages
            .iter_mut()
            .find(|l| l.tag.eq_ignore_ascii_case("zh-Hant-TW"))
        {
            language.tips.extend(selected.clone());
        } else {
            desired.languages.push(Language {
                tag: "zh-Hant-TW".into(),
                tips: selected.clone(),
                spelling: true,
                handwriting: false,
            });
        }
    }
    if before
        .default_tip
        .as_ref()
        .is_some_and(|tip| bit(tip) != 0 && bit(tip) & mask == 0)
    {
        desired.default_tip = selected.first().cloned().or_else(|| {
            desired
                .languages
                .iter()
                .flat_map(|l| &l.tips)
                .next()
                .cloned()
        });
    }
    desired
}
pub fn read(folder: &Path) -> Result<State> {
    run(folder, "read", None, None, None)
}
pub fn apply(folder: &Path, before: &State, desired: &State) -> Result<State> {
    run(folder, "apply", Some(before), Some(desired), None)
}
pub fn restore(folder: &Path, before: &State, attempted: &State) -> Result<State> {
    run(
        folder,
        "restore",
        Some(before),
        Some(before),
        Some(attempted),
    )
}
fn run(
    folder: &Path,
    action: &str,
    before: Option<&State>,
    desired: Option<&State>,
    alternate: Option<&State>,
) -> Result<State> {
    fs::create_dir_all(folder)?;
    safe_tree(folder)?;
    let count = NEXT.fetch_add(1, Ordering::Relaxed);
    let output = folder.join(format!("language-{action}-{count}.json"));
    let request_file = folder.join(format!("language-request-{count}.json"));
    let request = Request {
        action,
        output: output.to_string_lossy().into(),
        before,
        desired,
        alternate,
    };
    transaction::atomic_file(&request_file, &serde_json::to_vec_pretty(&request)?)?;
    let system = if cfg!(target_pointer_width = "32") {
        "Sysnative"
    } else {
        "System32"
    };
    let executable = PathBuf::from(std::env::var_os("WINDIR").ok_or("Windows 目錄無法辨識。")?)
        .join(system)
        .join("WindowsPowerShell/v1.0/powershell.exe");
    // Windows passes Unicode environment values intact. The command evaluates
    // only this compile-time script; request fields are JSON data, never code.
    let mut child = Command::new(executable)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "& ([scriptblock]::Create($env:SHANJIE_LANGUAGE_SCRIPT))",
        ])
        .env("SHANJIE_LANGUAGE_REQUEST", &request_file)
        .env(
            "SHANJIE_TEST_PROFILE",
            if cfg!(test) {
                "1".into()
            } else {
                std::env::var("SHANJIE_TEST_PROFILE").unwrap_or_default()
            },
        )
        .env(
            "SHANJIE_LANGUAGE_SCRIPT",
            include_str!("../../../scripts/user-language-list.ps1"),
        )
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let start = Instant::now();
    while child.try_wait()?.is_none() {
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Windows 語言設定介面等待逾時；請查看安裝紀錄。".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let result = child.wait_with_output()?;
    if !result.status.success() {
        return Err(format!(
            "Windows 語言清單操作失敗：{}",
            String::from_utf8_lossy(&result.stderr).trim()
        )
        .into());
    }
    let state: State = serde_json::from_slice(&fs::read(output)?)?;
    if let Some(desired) = desired {
        if &state != desired {
            return Err(format!(
                "Windows 語言清單未符合預期；預期善解配置 {}，實際 {}。",
                mask(desired),
                mask(&state)
            )
            .into());
        }
    }
    Ok(state)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> State {
        State {
            languages: vec![
                Language {
                    tag: "en-US".into(),
                    tips: vec!["0409:00000409".into()],
                    spelling: false,
                    handwriting: false,
                },
                Language {
                    tag: "zh-Hant-TW".into(),
                    tips: vec![
                        selected(1)[0].clone(),
                        "0404:OTHER".into(),
                        selected(2)[0].clone(),
                    ],
                    spelling: true,
                    handwriting: true,
                },
            ],
            default_tip: Some(selected(2)[0].clone()),
        }
    }
    #[test]
    fn dual_keyboard_migration_preserves_language_properties_and_other_order() {
        let before = fixture();
        for mask in [1, 2, 3, 0] {
            let after = plan(&before, mask);
            assert_eq!(super::mask(&after), mask);
            assert_eq!(after.languages[0], before.languages[0]);
            assert_eq!(after.languages[1].spelling, true);
            assert_eq!(after.languages[1].handwriting, true);
            assert!(after.languages[1].tips.contains(&"0404:OTHER".into()));
            assert_eq!(plan(&after, mask), after);
        }
        let standard = plan(&before, 1);
        assert_eq!(
            standard.languages[1].tips,
            vec![selected(1)[0].clone(), "0404:OTHER".into()]
        );
        assert_eq!(standard.default_tip, Some(selected(1)[0].clone()));
    }
    #[test]
    fn first_install_appends_only_needed_language_and_keeps_foreign_defaults() {
        let mut before = fixture();
        before.languages.pop();
        before.default_tip = Some("0409:00000409".into());
        let after = plan(&before, 2);
        assert_eq!(after.languages[0], before.languages[0]);
        assert_eq!(after.languages[1].tag, "zh-Hant-TW");
        assert_eq!(after.default_tip, before.default_tip);
        assert_eq!(super::mask(&after), 2);
    }
    #[test]
    #[ignore = "Creates real International objects but never calls Set-WinUserLanguageList"]
    fn windows_language_object_round_trip() {
        let folder = PathBuf::from(
            std::env::var_os("SHANJIE_TEST_ROOT").expect("Use an isolated workspace root"),
        )
        .join("language-objects");
        let before = fixture();
        for mask in [1, 2, 3, 0] {
            let desired = plan(&before, mask);
            assert_eq!(
                run(&folder, "preview", Some(&before), Some(&desired), None).unwrap(),
                desired
            );
        }
        let observed = read(&folder).unwrap();
        assert!(!observed.languages.is_empty());
        let error = apply(&folder, &before, &plan(&before, 1)).unwrap_err();
        assert!(error
            .to_string()
            .contains("Isolated verification cannot change"));
    }
}
