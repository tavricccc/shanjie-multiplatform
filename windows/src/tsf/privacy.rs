//! Check the document boundary before any left-text read or learning operation.
use super::service::{compartment, range, selection, Data};
use windows::{core::*, Win32::{Foundation::E_FAIL, System::Com::CoTaskMemFree,
    UI::{Input::KeyboardAndMouse::GetFocus, TextServices::*, WindowsAndMessaging::*}}};

fn app_allows_learning(name: Option<&str>) -> bool {
    let Some(name) = name.filter(|n| !n.is_empty()) else { return false; };
    !["1password.exe", "bitwarden.exe", "keepass.exe", "keepassxc.exe", "enpass.exe", "dashlane.exe"]
        .iter().any(|excluded| name.eq_ignore_ascii_case(excluded))
}

impl Data {
    pub(super) fn update_privacy(&self, context: &ITfContext, cookie: u32) -> Result<()> {
        let app = std::env::current_exe().ok();
        let known_app = app.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str());
        let allowed = app_allows_learning(known_app) && unsafe {
            !compartment(context, &GUID_COMPARTMENT_KEYBOARD_DISABLED)
                && !compartment(context, &GUID_COMPARTMENT_EMPTYCONTEXT)
                && context.GetStatus()?.dwDynamicFlags & TF_SD_READONLY == 0
                // A temporarily unavailable selection must not block composition.
                // Pause learning/context until the document can be identified.
                && !private_scope(context, cookie).unwrap_or(true)
        };
        let mut s = self.state.borrow_mut();
        s.engine.set_privacy_allowed(allowed).ok_or(E_FAIL)?;
        if !allowed { s.spacing_prev = None; }
        Ok(())
    }
}

unsafe fn private_scope(context: &ITfContext, cookie: u32) -> Result<bool> {
    let focus = GetFocus();
    let mut class = [0u16; 128];
    let length = GetClassNameW(focus, &mut class);
    let class = String::from_utf16_lossy(&class[..length as usize]).to_ascii_lowercase();
    if (class == "edit" || class.starts_with("richedit"))
        && GetWindowLongW(focus, GWL_STYLE) as u32 & ES_PASSWORD as u32 != 0 { return Ok(true); }
    let selected = range(selection(context, cookie)?);
    // A host without scope attributes is a normal document, like upstream's
    // known application without Secure Event Input. No text is read here.
    if let Ok(value) = context.GetAppProperty(&GUID_PROP_INPUTSCOPE).and_then(|p| p.GetValue(cookie, &selected)) {
        if let Ok(scope) = IUnknown::try_from(&value).and_then(|u| u.cast::<ITfInputScope>()) {
            let mut scopes = std::ptr::null_mut(); let mut count = 0;
            scope.GetInputScopes(&mut scopes, &mut count)?;
            let private = !scopes.is_null() && std::slice::from_raw_parts(scopes, count as usize)
                .iter().any(|s| [IS_PRIVATE, IS_PASSWORD, IS_NUMERIC_PASSWORD, IS_NUMERIC_PIN, IS_ALPHANUMERIC_PIN].contains(s));
            CoTaskMemFree(Some(scopes.cast()));
            return Ok(private);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_identity_and_windows_password_managers_pause_learning() {
        for app in [None, Some(""), Some("1Password.exe"), Some("KeePassXC.exe"), Some("bitwarden.exe")] {
            assert!(!app_allows_learning(app));
        }
        for app in ["notepad.exe", "ApplicationFrameHost.exe", "shanjie_tsf_compat.exe"] {
            assert!(app_allows_learning(Some(app)));
        }
    }
}
