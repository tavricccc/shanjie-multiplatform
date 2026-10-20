//! Deletes what an update or uninstall could not, because open programs still held it
//! (usually Shanjie.dll). After signing out or restarting nothing holds it any more, so a
//! one-off SYSTEM task runs at the next sign-in or boot, deletes exactly those paths and then
//! removes itself. Windows' own restart-time deletion is queued as well.
use super::logon_task::{escape, missing, root};
use crate::profile::Result;
use std::path::{Path, PathBuf};
use windows::{
    core::BSTR,
    Win32::System::{TaskScheduler::*, Variant::VARIANT},
};

pub const PATH: &str = "\\ShanjieCleanup";
const SYSTEM: &str = "S-1-5-18";

fn system32(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()))
        .join("System32")
        .join(name)
}
fn quoted(path: &Path) -> Result<String> {
    let text = path.to_string_lossy();
    // cmd expands % even inside quotes; a quote would end the argument.
    if text.contains(['"', '%']) || !path.is_absolute() {
        return Err(format!("清除路徑不合法：{text}").into());
    }
    Ok(format!("\"{text}\""))
}
/// `remove` are deleted with everything in them; `empty` folders only if nothing is left.
pub fn command(remove: &[PathBuf], empty: &[PathBuf]) -> Result<String> {
    let mut steps = Vec::new();
    for path in remove {
        let path = quoted(path)?;
        steps.push(format!("rd /s /q {path} 2>nul"));
        steps.push(format!("del /f /q /a {path} 2>nul"));
    }
    for path in empty {
        steps.push(format!("rd {} 2>nul", quoted(path)?));
    }
    let mut done = String::new();
    for path in remove {
        done.push_str(&format!("if not exist {} ", quoted(path)?));
    }
    let schtasks = quoted(&system32("schtasks.exe"))?;
    steps.push(format!("{done}{schtasks} /delete /tn {} /f", &PATH[1..]));
    Ok(format!("/d /s /c \"{}\"", steps.join(" & ")))
}
pub fn definition(remove: &[PathBuf], empty: &[PathBuf]) -> Result<String> {
    let cmd = escape(&system32("cmd.exe").to_string_lossy());
    let arguments = escape(&command(remove, empty)?);
    Ok(format!(
        r#"<Task version="1.3" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Author>善解</Author><Description>刪除善解更新或解除安裝時仍在使用中的檔案，完成後自動移除這個排程。</Description><URI>{PATH}</URI></RegistrationInfo>
  <Triggers><BootTrigger><Enabled>true</Enabled></BootTrigger><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers>
  <Principals><Principal id="System"><UserId>{SYSTEM}</UserId><RunLevel>HighestAvailable</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>true</Hidden>
    <ExecutionTimeLimit>PT10M</ExecutionTimeLimit>
  </Settings>
  <Actions Context="System"><Exec><Command>{cmd}</Command><Arguments>{arguments}</Arguments></Exec></Actions>
</Task>"#
    ))
}
fn submit(xml: &str, mode: TASK_CREATION) -> Result<()> {
    unsafe {
        let (_com, folder) = root()?;
        let empty = VARIANT::default();
        folder
            .RegisterTask(
                &BSTR::from(PATH),
                &BSTR::from(xml),
                mode.0,
                &empty,
                &empty,
                TASK_LOGON_SERVICE_ACCOUNT,
                &empty,
            )
            .map(drop)
            .or_else(|error| match error.code().is_ok() {
                true if mode == TASK_VALIDATE_ONLY => Ok(()),
                _ => Err(error.into()),
            })
    }
}
/// Replaces any earlier cleanup; with nothing left over, removes it.
pub fn schedule(remove: &[PathBuf], empty: &[PathBuf]) -> Result<()> {
    if remove.is_empty() && !empty.iter().any(|p| p.exists()) {
        return cancel();
    }
    submit(&definition(remove, empty)?, TASK_CREATE_OR_UPDATE)
}
pub fn cancel() -> Result<()> {
    unsafe {
        let (_com, folder) = root()?;
        match folder.DeleteTask(&BSTR::from(PATH), 0) {
            Err(error) if !missing(&error) => Err(error.into()),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_deletes_only_the_given_paths_then_itself() {
        let old = PathBuf::from(r"C:\Program Files\Shanjie\versions\old");
        let root = PathBuf::from(r"C:\Program Files\Shanjie");
        let text = command(&[old], &[root.join("versions"), root]).unwrap();
        assert!(text.starts_with("/d /s /c \"rd /s /q \"C:\\Program Files\\Shanjie\\versions\\old\" 2>nul & "));
        assert!(text.contains("& rd \"C:\\Program Files\\Shanjie\\versions\" 2>nul & rd \"C:\\Program Files\\Shanjie\" 2>nul & "));
        assert!(text.contains("if not exist \"C:\\Program Files\\Shanjie\\versions\\old\" \""));
        assert!(text.ends_with("schtasks.exe\" /delete /tn ShanjieCleanup /f\""));
        assert!(!text.contains("rd /s /q \"C:\\Program Files\\Shanjie\" "));
    }
    #[test]
    fn paths_cmd_would_reinterpret_are_refused() {
        for bad in [r"C:\a%PATH%b", r#"C:\a"b"#, r"relative\x"] {
            assert!(command(&[PathBuf::from(bad)], &[]).is_err(), "{bad}");
        }
    }
    #[test]
    fn task_scheduler_accepts_the_definition_without_registering_it() {
        let old = PathBuf::from(r"C:\Program Files\Shanjie\versions\a&b <old>");
        submit(&definition(&[old], &[]).unwrap(), TASK_VALIDATE_ONLY).unwrap();
    }
}
