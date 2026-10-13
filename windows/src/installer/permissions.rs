//! Repair only the exact AI-bit loss recorded by failed Windows 0.5.2 installs.
use super::*;
use crate::registry_snapshot::{self, Saved};
#[derive(serde::Deserialize)]
struct Legacy {
    registry: Vec<Saved>,
}
pub fn repair(target: &Path, backup: &Path, registry: Vec<Saved>) -> Result<Vec<Saved>> {
    let root = target.join(".installer-recovery");
    safe_tree(&root)?;
    let mut planned = registry.clone();
    let mut changes = 0;
    let mut folders = fs::read_dir(&root)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path() != backup && entry.path().is_dir())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    folders.sort();
    for folder in folders {
        safe_tree(&folder)?;
        if !folder.join("rollback-errors.txt").is_file() {
            continue;
        }
        let file = folder.join("checkpoint.json");
        safe_tree(&file)?;
        if !file.is_file() || file.metadata()?.len() > 10 * 1024 * 1024 {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_slice(&fs::read(&file)?) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if value.get("languages").is_some() {
            continue;
        } // Only the old checkpoint format.
        let old: Legacy = match serde_json::from_value(value) {
            Ok(value) => value,
            Err(_) => continue,
        };
        for current in &mut planned {
            // The whitelist is derived from the installer's current locations.
            if current.location.user || current.location.value.is_some() {
                continue;
            }
            if let Some(original) = old
                .registry
                .iter()
                .find(|old| old.location == current.location)
            {
                let (plan, count) = registry_snapshot::inheritance_plan(current, original);
                *current = plan;
                changes += count;
            }
        }
    }
    if changes == 0 {
        return Ok(registry);
    }
    transaction::atomic_file(
        &backup.join("permission-repair-before.json"),
        &serde_json::to_vec_pretty(&registry)?,
    )?;
    transaction::atomic_file(
        &backup.join("permission-repair-plan.json"),
        &serde_json::to_vec_pretty(&planned)?,
    )?;
    journal::event(
        backup,
        "LegacyPermissions",
        "started",
        &format!("修復 {changes} 個已確認僅缺少 AI 旗標的本程式登錄項目"),
    )?;
    for plan in &planned {
        if registry
            .iter()
            .any(|old| old.location == plan.location && old != plan)
        {
            if let Err(error) = registry_snapshot::repair_inheritance(plan) {
                let mut failures = Vec::new();
                for old in &registry {
                    if planned
                        .iter()
                        .any(|plan| plan.location == old.location && plan != old)
                    {
                        if let Err(error) = registry_snapshot::restore(old) {
                            failures.push(error.to_string());
                        }
                    }
                }
                return Err(format!(
                    "舊版權限修復未完成：{error}；修復前狀態回復：{}",
                    if failures.is_empty() {
                        "完成".into()
                    } else {
                        failures.join("；")
                    }
                )
                .into());
            }
        }
    }
    journal::event(
        backup,
        "LegacyPermissions",
        "succeeded",
        &format!("已修復 {changes} 個繼承旗標，存取規則與資料值保留"),
    )?;
    Ok(planned)
}
