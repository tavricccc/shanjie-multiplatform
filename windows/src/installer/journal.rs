//! Append durable install diagnostics before attempting rollback.
use super::*;
use std::io::Write;
pub fn event(folder: &Path, phase: &str, status: &str, message: &str) -> Result<()> {
    progress::step(phase, status, message);
    fs::create_dir_all(folder)?;
    safe_tree(folder)?;
    let value = serde_json::json!({"time_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis(),"version":env!("CARGO_PKG_VERSION"),"phase":phase,"status":status,"message":message});
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(folder.join("events.jsonl"))?;
    serde_json::to_writer(&mut file, &value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
pub fn error(folder: &Path, phase: &str, message: &str) -> Result<()> {
    fs::create_dir_all(folder)?;
    safe_tree(folder)?;
    // This file preserves the initial error even if rollback fails afterwards.
    transaction::atomic_file(
        &folder.join("error.txt"),
        format!(
            "版本：{}\n階段：{phase}\n{message}\n",
            env!("CARGO_PKG_VERSION")
        )
        .as_bytes(),
    )?;
    event(folder, phase, "failed", message)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_error_survives_later_rollback_failure() {
        let root = std::env::var_os("SHANJIE_TEST_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!(
                "installer-journal-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        error(&root, "Validate", "Windows 語言清單：預期 1，實際 3").unwrap();
        event(&root, "Rollback", "failed", "different rollback failure").unwrap();
        assert!(fs::read_to_string(root.join("error.txt"))
            .unwrap()
            .contains("預期 1，實際 3"));
        let events = fs::read_to_string(root.join("events.jsonl")).unwrap();
        assert_eq!(events.lines().count(), 2);
        for line in events.lines() {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(value["time_unix_ms"].is_number());
        }
    }
}
