//! The release build contains no capture backend unless the diagnostic feature is enabled.
use super::service::Data;
use windows::{
    Win32::{Foundation::*, UI::TextServices::*},
};
#[cfg(feature = "input-diagnostics")]
use windows::{core::Interface, Win32::UI::Input::KeyboardAndMouse::*};

impl Data {
    #[cfg(feature = "input-diagnostics")]
    pub(super) fn diagnostic_shift_probe(&self, key: WPARAM, flags: LPARAM, physical: u32, side: u32, enabled: bool) {
        use crate::input_diagnostics as log;
        if !log::enabled() { return; }
        let held = |vk: VIRTUAL_KEY| unsafe { GetKeyState(vk.0 as i32) as u16 & 0x8000 != 0 };
        let control = held(VK_CONTROL);
        let alt = held(VK_MENU);
        let matches_side = side == 0 || physical == if side == 1 { 160 } else { 161 };
        log::emit("shift.probe", serde_json::json!({
            "message":log::key_metadata(key.0,flags.0),
            "physical_side":match physical {160=>"LEFT",161=>"RIGHT",_=>"UNKNOWN"},
            "configured_side":side,"toggle_enabled":enabled,"matches_side":matches_side,
            "control_held":control,"alt_held":alt,
            "left_shift_held":held(VK_LSHIFT),"right_shift_held":held(VK_RSHIFT),
            "gate_observed":enabled && matches_side && !control && !alt
        }));
    }
    pub(super) fn diagnostic_focus(&self, current: Option<&ITfDocumentMgr>, previous: Option<&ITfDocumentMgr>) {
        #[cfg(feature = "input-diagnostics")]
        {
            use crate::input_diagnostics as log;
            if !log::enabled() { return; }
            // Read identities only; do not enumerate document text or switch focus.
            let context = match self.state.try_borrow() {Ok(s)=>s.context.clone(),Err(_)=>return};
            let owner = context.as_ref().map(|c| unsafe { c.GetDocumentMgr() });
            let document = owner.as_ref().and_then(|r| r.as_ref().ok());
            log::emit("document_focus.details",serde_json::json!({
                "current_document":current.map(identity),"previous_document":previous.map(identity),
                "watched_context":context.as_ref().map(identity),"context_document":document.map(identity),
                "owner_lookup_result":owner.as_ref().map(|r|match r {Ok(_)=>0,Err(e)=>e.code().0}),
                "context_matches_current":document.is_some() && document == current,
                "context_matches_previous":document.is_some() && document == previous,
                "generation":self.generation.get()
            }));
        }
        #[cfg(not(feature = "input-diagnostics"))]
        let _ = (current, previous);
    }
    pub(super) fn diagnostic_retirement(&self, event: &'static str, retirement: u64,
        context: &ITfContext, composition: &ITfComposition, length: usize, result: Option<i32>) {
        #[cfg(feature = "input-diagnostics")]
        if crate::input_diagnostics::enabled() {
            crate::input_diagnostics::emit(event,serde_json::json!({
                "retirement":retirement,"context":identity(context),"composition":identity(composition),
                "generation":self.generation.get(),"write_utf16":length,"result":result
            }));
        }
        #[cfg(not(feature = "input-diagnostics"))]
        let _ = (event, retirement, context, composition, length, result);
    }
    pub(super) fn diagnostic(
        &self,
        event: &'static str,
        context: Option<&ITfContext>,
        key: Option<WPARAM>,
        result: Option<i32>,
    ) {
        #[cfg(feature = "input-diagnostics")]
        {
            use crate::input_diagnostics as log;
            if !log::enabled() {
                return;
            }
            let Ok(s) = self.state.try_borrow() else {
                return;
            };
            log::emit(
                event,
                serde_json::json!({
                    "context":context.map(identity),"watched_context":s.context.as_ref().map(identity),
                    "composition":s.composition.as_ref().map(identity),"generation":self.generation.get(),
                    "key":key.map(|k|log::key_name(k.0)),"result":result,
                    "preedit_utf16":s.snapshot.preedit.encode_utf16().count(),"cursor":s.snapshot.cursor,
                    "shift_tap":s.shift_tap,"shift_used":s.shift_used,"shift_physical":s.shift_physical,
                    "shift_epoch":s.shift_epoch,"observer_epoch":super::modifiers::epoch(),
                    "editing":self.editing.get(),"ending":self.ending.get(),
                    "shift_side":s.prefs.number("shortcuts","shift_side",0),
                    "shift_toggle":s.prefs.flag("shortcuts","shift_toggle",true),
                    "mixed_input":s.prefs.flag("input","mixed_input",false),
                    "upstream_english":s.prefs.flag("input","upstream_english",false)
                }),
            );
        }
        #[cfg(not(feature = "input-diagnostics"))]
        let _ = (event, context, key, result);
    }
    pub(super) fn diagnostic_range(
        &self,
        event: &'static str,
        range: &ITfRange,
        cookie: u32,
        length: usize,
    ) {
        #[cfg(feature = "input-diagnostics")]
        if crate::input_diagnostics::enabled() {
            let extent = unsafe {
                range.cast::<ITfRangeACP>().ok().and_then(|r| {
                    let mut start = 0;
                    let mut count = 0;
                    r.GetExtent(&mut start, &mut count)
                        .ok()
                        .map(|_| (start, count))
                })
            };
            // No GetText call: even a selected private document is represented only by positions.
            crate::input_diagnostics::emit(
                event,
                serde_json::json!({"range":identity(range),
                "extent":extent,"cookie":cookie,"write_utf16":length}),
            );
        }
        #[cfg(not(feature = "input-diagnostics"))]
        let _ = (event, range, cookie, length);
    }
}
#[cfg(feature = "input-diagnostics")]
fn identity<T: Interface>(object: &T) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (object.as_raw() as usize).hash(&mut hash);
    hash.finish()
}

pub(super) fn message(key: WPARAM, flags: LPARAM, down: bool, system: bool) {
    #[cfg(feature = "input-diagnostics")]
    if crate::input_diagnostics::enabled() {
        crate::input_diagnostics::emit(
            "message_queue",
            serde_json::json!({
            "key":crate::input_diagnostics::key_name(key.0),"down":down,"system":system,
            "message":crate::input_diagnostics::key_metadata(key.0,flags.0),
            "counts_as_other_key":!super::modifiers::is_shift_message(key,flags),
            "observer_epoch":super::modifiers::epoch()}),
        );
    }
    #[cfg(not(feature = "input-diagnostics"))]
    let _ = (key, flags, down, system);
}
