//! WER policy belongs to our resident executable, never to a process loading the TIP.
use std::io;
use windows::core::{HRESULT, PCWSTR};

const HOST: &str = "shanjie-core.exe";
#[link(name = "wer")]
unsafe extern "system" {
    fn WerAddExcludedApplication(name: PCWSTR, all_users: i32) -> HRESULT;
    fn WerRemoveExcludedApplication(name: PCWSTR, all_users: i32) -> HRESULT;
}

/// Like upstream, refuse to load input/model state when exclusion cannot be set.
pub fn protect_host() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    protect(exe.file_name().and_then(|n| n.to_str()).unwrap_or(""), |name| {
        // Isolated harnesses exercise the injected policy backend in unit tests;
        // their fixture host must not change this user's actual WER registry.
        if std::env::var("SHANJIE_TEST_PROFILE").as_deref() == Ok("1") { return Ok(()); }
        unsafe { WerAddExcludedApplication(PCWSTR(crate::wide(name).as_ptr()), 0).ok() }
            .map_err(|e| io::Error::other(e.to_string()))
    })
}

fn protect(name: &str, add: impl FnOnce(&str) -> io::Result<()>) -> io::Result<()> {
    if !name.eq_ignore_ascii_case(HOST) {
        return Err(io::Error::other("WER policy is restricted to shanjie-core.exe"));
    }
    add(HOST)
}

/// Versions share a filename-wide HKCU exclusion. Keep it across exits, upgrades,
/// rollback, and deferred removal of DLLs which can still start their old host.
pub(crate) fn uninstall(retained_versions: bool) -> io::Result<()> {
    cleanup(retained_versions, || {
        let result = unsafe { WerRemoveExcludedApplication(PCWSTR(crate::wide(HOST).as_ptr()), 0) };
        // An installation that has never started has no exclusion to remove.
        if result.0 as u32 == 0x80070002 { return Ok(()); }
        result.ok().map_err(|e| io::Error::other(e.to_string()))
    })
}

fn cleanup(retained_versions: bool, remove: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
    if retained_versions { Ok(()) } else { remove() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_our_host_gets_a_per_user_exclusion_and_failure_stops_startup() {
        for name in ["notepad.exe", "Shanjie.dll", "other-core.exe"] {
            assert!(protect(name, |_| panic!("must not change another application's policy")).is_err());
        }
        assert!(protect("SHANJIE-CORE.EXE", |name| { assert_eq!(name, HOST); Ok(()) }).is_ok());
        assert!(protect(HOST, |_| Err(io::Error::from(io::ErrorKind::PermissionDenied))).is_err());
    }
    #[test]
    fn coexistence_and_deferred_uninstall_keep_the_shared_exclusion() {
        cleanup(true, || panic!("another version still needs the policy")).unwrap();
        let mut removed = false;
        cleanup(false, || { removed = true; Ok(()) }).unwrap();
        assert!(removed);
        assert!(cleanup(false, || Err(io::Error::from(io::ErrorKind::PermissionDenied))).is_err());
    }
}
