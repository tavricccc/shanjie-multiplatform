//! Runs the short-lived update check through Task Scheduler, so the resident core stays offline.
//! The settings program reads the user's on/off choice and exits at once when it is off.
use super::logon_task::{element, escape, missing, root};
use crate::profile::Result;
use std::path::Path;
use windows::{
    core::BSTR,
    Win32::System::{TaskScheduler::*, Variant::VARIANT},
};

pub const PATH: &str = "\\ShanjieUpdateCheck";
const ARGUMENT: &str = "--check-update";
/// BUILTIN\Users: the check runs in every interactive session, as the signed-in user.
const USERS: &str = "S-1-5-32-545";

/// Two minutes after sign-in, then every 24 hours; a run missed while asleep starts when the
/// machine is available again. The time limit covers a card left on screen.
pub fn definition(ui: &Path) -> String {
    let command = escape(&ui.to_string_lossy());
    let directory = escape(&ui.parent().unwrap_or(ui).to_string_lossy());
    format!(
        r#"<Task version="1.3" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Author>善解</Author><Description>檢查善解是否有新版本。</Description><URI>{PATH}</URI></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><Delay>PT2M</Delay><Repetition><Interval>PT24H</Interval><StopAtDurationEnd>false</StopAtDurationEnd></Repetition></LogonTrigger></Triggers>
  <Principals><Principal id="Users"><GroupId>{USERS}</GroupId><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>Parallel</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>true</RunOnlyIfNetworkAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>true</Hidden>
    <ExecutionTimeLimit>PT10M</ExecutionTimeLimit>
  </Settings>
  <Actions Context="Users"><Exec><Command>{command}</Command><Arguments>{ARGUMENT}</Arguments><WorkingDirectory>{directory}</WorkingDirectory></Exec></Actions>
</Task>"#
    )
}
/// Compares what Task Scheduler stored (it rewrites the XML) with what this install needs.
pub fn matches(xml: &str, ui: &Path) -> bool {
    element(xml, "Command").as_deref() == Some(&*ui.to_string_lossy())
        && element(xml, "Arguments").as_deref() == Some(ARGUMENT)
        && element(xml, "GroupId").as_deref() == Some(USERS)
        && xml.contains("<LogonTrigger")
        && xml.contains("<Repetition")
}
pub fn read() -> Result<Option<String>> {
    unsafe {
        let (_com, folder) = root()?;
        match folder.GetTask(&BSTR::from(PATH)) {
            Ok(task) => Ok(Some(task.Xml()?.to_string())),
            Err(error) if missing(&error) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}
pub fn register(xml: &str) -> Result<()> {
    submit(xml, TASK_CREATE_OR_UPDATE)
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
                TASK_LOGON_GROUP,
                &empty,
            )
            .map(drop)
            .or_else(|error| match error.code().is_ok() {
                true if mode == TASK_VALIDATE_ONLY => Ok(()),
                _ => Err(error.into()),
            })
    }
}
pub fn remove() -> Result<()> {
    unsafe {
        let (_com, folder) = root()?;
        match folder.DeleteTask(&BSTR::from(PATH), 0) {
            Err(error) if !missing(&error) => Err(error.into()),
            _ => Ok(()),
        }
    }
}
/// Puts back the task exactly as the checkpoint saw it, including its absence.
pub fn restore(saved: Option<&str>) -> Result<()> {
    match saved {
        Some(xml) => register(xml)?,
        None => remove()?,
    }
    if read()?.is_some() != saved.is_some() {
        return Err("更新檢查排程回復校驗失敗。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn definition_matches_only_this_install() {
        let ui = Path::new(r"C:\Program Files\A&B\versions\new\ui\Shanjie.UI.exe");
        let old = Path::new(r"C:\Program Files\A&B\versions\old\ui\Shanjie.UI.exe");
        let xml = definition(ui);
        assert!(matches(&xml, ui));
        assert!(!matches(&xml, old));
        assert!(!matches(&xml.replace(ARGUMENT, "--setup"), ui));
        assert!(!matches(&xml.replace("Repetition", "Other"), ui));
    }
    #[test]
    fn task_scheduler_accepts_the_definition_without_registering_it() {
        let ui = Path::new(r"C:\Program Files\Shanjie\versions\a\ui\Shanjie.UI.exe");
        submit(&definition(ui), TASK_VALIDATE_ONLY).unwrap();
    }
}
