//! Starts the resident core at sign-in through Task Scheduler. A logon trigger fires before
//! Explorer's delayed Run entries, so the first key after sign-in rarely waits for a cold core.
use crate::profile::Result;
use std::path::Path;
use windows::{
    core::BSTR,
    Win32::System::{Com::*, TaskScheduler::*, Variant::VARIANT},
};

pub const PATH: &str = "\\ShanjieCore";
/// BUILTIN\Users: the task runs once in every interactive session, as the signed-in user.
const USERS: &str = "S-1-5-32-545";

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}
/// Normal priority (Task Scheduler defaults to below normal) and no time limit (the default
/// stops tasks after 72 hours); parallel instances so each signed-in user gets a core.
pub fn definition(core: &Path) -> String {
    let command = escape(&core.to_string_lossy());
    let directory = escape(&core.parent().unwrap_or(core).to_string_lossy());
    format!(
        r#"<Task version="1.3" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Author>善解</Author><Description>登入後預載善解常駐核心。</Description><URI>{PATH}</URI></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers>
  <Principals><Principal id="Users"><GroupId>{USERS}</GroupId><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>Parallel</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>true</Hidden>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Users"><Exec><Command>{command}</Command><Arguments>--serve</Arguments><WorkingDirectory>{directory}</WorkingDirectory></Exec></Actions>
</Task>"#
    )
}
fn element(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml[start..].find(&format!("</{name}>"))?;
    Some(unescape(xml[start..end].trim()))
}
/// Compares what Task Scheduler stored (it rewrites the XML) with what this install needs.
pub fn matches(xml: &str, core: &Path) -> bool {
    element(xml, "Command").as_deref() == Some(&*core.to_string_lossy())
        && element(xml, "Arguments").as_deref() == Some("--serve")
        && element(xml, "GroupId").as_deref() == Some(USERS)
        && xml.contains("<LogonTrigger")
        && element(xml, "ExecutionTimeLimit").as_deref() == Some("PT0S")
}

struct Com(bool);
impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}
unsafe fn root() -> Result<(Com, ITaskFolder)> {
    let com = Com(CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok());
    let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
    let empty = VARIANT::default();
    service.Connect(&empty, &empty, &empty, &empty)?;
    let folder = service.GetFolder(&BSTR::from("\\"))?;
    Ok((com, folder))
}
fn missing(error: &windows::core::Error) -> bool {
    // ERROR_FILE_NOT_FOUND or ERROR_PATH_NOT_FOUND as HRESULTs.
    matches!(error.code().0 as u32, 0x80070002 | 0x80070003)
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
        folder.RegisterTask(
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
            // Validation succeeds without returning a registered task.
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
        return Err("登入啟動排程回復校驗失敗。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn definition_round_trips_paths_that_need_escaping() {
        let core = Path::new(r"C:\Program Files\A&B <x>\tools\shanjie-core.exe");
        let xml = definition(core);
        assert!(xml.contains("A&amp;B &lt;x&gt;"));
        assert!(matches(&xml, core));
        assert_eq!(
            element(&xml, "WorkingDirectory").as_deref(),
            Some(r"C:\Program Files\A&B <x>\tools")
        );
    }
    #[test]
    fn task_scheduler_accepts_the_definition_without_registering_it() {
        let core = Path::new(r"C:\Program Files\Shanjie\versions\a\tools\shanjie-core.exe");
        submit(&definition(core), TASK_VALIDATE_ONLY).unwrap();
    }
    #[test]
    fn other_versions_and_edited_tasks_do_not_match() {
        let core = Path::new(r"C:\Program Files\Shanjie\versions\new\tools\shanjie-core.exe");
        let old = Path::new(r"C:\Program Files\Shanjie\versions\old\tools\shanjie-core.exe");
        assert!(!matches(&definition(old), core));
        let limited = definition(core).replace("PT0S", "PT72H");
        assert!(!matches(&limited, core));
        let single = definition(core).replace(USERS, "S-1-5-21-1-2-3-1001");
        assert!(!matches(&single, core));
    }
}
