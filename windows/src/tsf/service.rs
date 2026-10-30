use super::{candidates, mode, resident, Count};
use crate::{
    engine::{Engine, Snapshot},
    input::{Associated, Pinyin, Special},
    native::{ControlChannel, Indicator},
    profile::{self, Document},
};
use shanjie_core::ffi::*;
use std::{
    cell::{Cell, RefCell},
    ffi::{c_void, CString},
    path::PathBuf,
    rc::Rc,
};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        System::{Com::*, Variant::*},
        UI::{HiDpi::*, Shell::ShellExecuteW, TextServices::*, WindowsAndMessaging::*},
    },
};

pub fn output(s: Option<Snapshot>) -> Result<Snapshot> {
    s.ok_or(E_FAIL.into())
}
pub unsafe fn compartment(owner: &impl Interface, id: &GUID) -> bool {
    let get = || -> Result<bool> {
        let manager: ITfCompartmentMgr = owner.cast()?;
        let value = manager.GetCompartment(id)?.GetValue()?;
        Ok(i32::try_from(&value)? != 0)
    };
    get().unwrap_or(false)
}
pub unsafe fn selection(context: &ITfContext, cookie: u32) -> Result<TF_SELECTION> {
    let mut values = [TF_SELECTION::default()];
    let mut fetched = 0;
    context.GetSelection(cookie, TF_DEFAULT_SELECTION, &mut values, &mut fetched)?;
    if fetched == 0 {
        return Err(E_FAIL.into());
    }
    Ok(std::mem::take(&mut values[0]))
}
pub fn range(s: TF_SELECTION) -> ITfRange {
    std::mem::ManuallyDrop::into_inner(s.range).unwrap()
}

pub struct State {
    pub thread: Option<ITfThreadMgr>,
    pub client: u32,
    pub context: Option<ITfContext>,
    pub composition: Option<ITfComposition>,
    thread_sink: u32,
    focus_sink: u32,
    layout_sink: u32,
    text_sink: u32,
    pub candidates: Option<candidates::Handle>,
    button: Option<ITfLangBarItemButton>,
    bar: Option<ITfLangBarItemMgr>,
    pub(super) watcher: Option<Box<ControlChannel>>,
    pub engine: Engine,
    preload: Option<resident::Pending>,
    pub pinyin: Pinyin,
    pub special: Special,
    pub associated: Associated,
    pub association_caret: Option<ITfRange>,
    pub snapshot: Snapshot,
    pub prefs: Document,
    pub data: PathBuf,
    binary: PathBuf,
    pub(super) profile_layout: u32,
    pub full_width: bool,
    pub shift_tap: bool,
    pub shift_used: bool,
    pub shift_physical: u32,
    pub shift_epoch: u64,
    observer: Option<super::modifiers::Observer>,
    pub raw_reading: String,
    /// The character before the cursor for 中英文之間自動加空格 (upstream cjk-spacing section 1); `None` when unknown.
    pub spacing_prev: Option<char>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            thread: None,
            client: 0,
            context: None,
            composition: None,
            thread_sink: TF_INVALID_COOKIE,
            focus_sink: TF_INVALID_COOKIE,
            layout_sink: TF_INVALID_COOKIE,
            text_sink: TF_INVALID_COOKIE,
            candidates: None,
            button: None,
            bar: None,
            watcher: None,
            engine: Engine::default(),
            preload: None,
            pinyin: Pinyin::default(),
            special: Special::default(),
            associated: Associated::default(),
            association_caret: None,
            snapshot: Snapshot::default(),
            prefs: Document::default(),
            data: PathBuf::new(),
            binary: PathBuf::new(),
            profile_layout: 0,
            full_width: false,
            shift_tap: false,
            shift_used: false,
            shift_physical: 0,
            shift_epoch: 0,
            observer: None,
            raw_reading: String::new(),
            spacing_prev: None,
        }
    }
}
impl State {
    fn core_config(&self) -> resident::Config {
        resident::Config::new(&self.data, &self.prefs, self.profile_layout)
    }
    fn park_engine(&mut self) {
        if self.engine.is_open() {
            resident::park(
                self.core_config(),
                std::mem::take(&mut self.engine),
                std::mem::take(&mut self.pinyin),
            );
        } else if let Some(pending) = self.preload.take() {
            pending.park();
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.park_engine();
    }
}
pub struct Data {
    pub state: RefCell<State>,
    pub ending: Cell<bool>,
    pub editing: Cell<bool>,
    pub mode_revision: Cell<u64>,
    pub(super) profile_update: RefCell<Option<super::profile_refresh::Pending>>,
    pub(super) generation: Cell<u64>,
    pub(super) recovery: RefCell<super::recovery::Recovery>,
    pub(super) application_written: Cell<bool>,
    pub(super) retiring: RefCell<Vec<(u64, ITfContext)>>,
    pub indicator: RefCell<Box<Indicator>>,
    pub(super) word_popup_revision: Cell<u64>,
    pub(super) word_popup: RefCell<Option<Box<crate::word_popup::Popup>>>,
    pub(super) word_revision: Cell<u64>,
    pub(super) word_request: RefCell<Option<super::add_word::Request>>,
    pub(super) word_lookup: RefCell<Option<crate::selection_text::Lookup>>,
    pub(super) word_notice: RefCell<Option<crate::selection_notice::Notice>>,
    pub(super) word_selection: RefCell<Option<super::add_word::Selection>>,
    pub(super) focus_handoff: RefCell<Option<super::focus_handoff::Pending>>,
    pub(super) ctrl_gesture: RefCell<super::add_word::Gesture>,
    pub(super) rerank_ticket: Cell<u64>,
    pub(super) rerank_wait: Cell<Option<(u64, std::time::Instant)>>,
    watcher_context: RefCell<Option<Box<std::rc::Weak<Data>>>>,
    owner: Cell<*mut c_void>,
    _count: Count,
}
#[implement(
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfThreadMgrEventSink,
    ITfTextLayoutSink,
    ITfTextEditSink,
    ITfThreadFocusSink
)]
pub(super) struct Service {
    pub(super) data: Rc<Data>,
}
pub fn new() -> ITfTextInputProcessorEx {
    let data = Rc::new(Data {
        state: RefCell::new(State::default()),
        ending: Cell::new(false),
        editing: Cell::new(false),
        mode_revision: Cell::new(0),
        profile_update: RefCell::new(None),
        generation: Cell::new(0),
        recovery: RefCell::new(Default::default()),
        application_written: Cell::new(false),
        retiring: RefCell::new(Vec::new()),
        indicator: RefCell::new(Box::new(Indicator::default())),
        word_popup_revision: Cell::new(0),
        word_popup: RefCell::new(None),
        word_revision: Cell::new(0),
        word_request: RefCell::new(None),
        word_lookup: RefCell::new(None),
        word_notice: RefCell::new(None),
        word_selection: RefCell::new(None),
        focus_handoff: RefCell::new(None),
        ctrl_gesture: RefCell::new(Default::default()),
        rerank_ticket: Cell::new(0),
        rerank_wait: Cell::new(None),
        watcher_context: RefCell::new(None),
        owner: Cell::new(std::ptr::null_mut()),
        _count: Count::new(),
    });
    let service: ITfTextInputProcessorEx = Service { data: data.clone() }.into();
    data.owner.set(service.as_raw());
    service
}

#[implement(ITfEditSession)]
struct Edit {
    _owner: IUnknown,
    data: Rc<Data>,
    operation: RefCell<Option<Box<dyn FnOnce(u32) -> Result<()>>>>,
}
impl ITfEditSession_Impl for Edit_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        let already_editing = self.data.editing.replace(true);
        let result = self.operation.borrow_mut().take().unwrap()(cookie);
        self.data.editing.set(already_editing);
        if let Err(e) = &result {
            super::trace(format_args!("edit session: {e}"));
        }
        result
    }
}
impl Data {
    pub fn owner(&self) -> IUnknown {
        unsafe {
            IUnknown::from_raw_borrowed(&self.owner.get())
                .unwrap()
                .clone()
        }
    }
    pub fn request(
        self: &Rc<Self>,
        context: &ITfContext,
        flags: TF_CONTEXT_EDIT_CONTEXT_FLAGS,
        operation: impl FnOnce(u32) -> Result<()> + 'static,
    ) -> Result<()> {
        let client = self.state.borrow().client;
        let edit: ITfEditSession = Edit {
            _owner: self.owner(),
            data: self.clone(),
            operation: RefCell::new(Some(Box::new(operation))),
        }
        .into();
        unsafe { context.RequestEditSession(client, &edit, flags)?.ok() }
    }
    pub fn chinese(&self) -> bool {
        self.state
            .borrow()
            .thread
            .as_ref()
            .is_some_and(|t| unsafe { compartment(t, &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) })
    }
    // Delayed work may use the current engine only while both identities match.
    // Finishing a captured old composition deliberately uses request instead.
    pub fn request_current(
        self: &Rc<Self>,
        context: &ITfContext,
        flags: TF_CONTEXT_EDIT_CONTEXT_FLAGS,
        operation: impl FnOnce(u32) -> Result<()> + 'static,
    ) -> Result<()> {
        let generation = self.generation.get();
        let current = context.clone();
        let data = self.clone();
        self.request(context, flags, move |cookie| {
            if data.generation.get() != generation
                || data.state.borrow().context.as_ref() != Some(&current)
            {
                return Ok(());
            }
            operation(cookie)
        })
    }
    pub fn settings(&self) {
        // Resolve the current registration: old hosts may still hold a retired DLL.
        let binary = crate::installer::current_binaries().ok().flatten()
            .unwrap_or_else(|| self.state.borrow().binary.clone());
        let path = binary.join("ui/Shanjie.UI.exe");
        unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(crate::wide(&path.to_string_lossy()).as_ptr()),
                None,
                PCWSTR(crate::wide(&path.parent().unwrap().to_string_lossy()).as_ptr()),
                SW_SHOWNORMAL,
            );
        }
    }
    pub fn set_mode(&self, chinese: bool) -> Result<()> {
        self.diagnostic("mode.set.request",None,None,Some(chinese as i32));
        let (thread, client, width, button) = {
            let s = self.state.borrow();
            (
                s.thread.clone().ok_or(E_FAIL)?,
                s.client,
                s.full_width,
                s.button.clone(),
            )
        };
        unsafe {
            let manager: ITfCompartmentMgr = thread.cast()?;
            let value = VARIANT::from(
                (if chinese { TF_CONVERSIONMODE_NATIVE } else { 0 }
                    | if width {
                        TF_CONVERSIONMODE_FULLSHAPE
                    } else {
                        0
                    }) as i32,
            );
            manager
                .GetCompartment(&GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION)?
                .SetValue(client, &value)?;
            manager
                .GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)?
                .SetValue(client, &VARIANT::from(chinese as i32))?;
        }
        if let Some(button) = button {
            mode::refresh(&button);
        }
        self.diagnostic("mode.set.completed",None,None,Some(chinese as i32));
        Ok(())
    }
    pub fn cancel(self: &Rc<Self>) -> Result<()> {
        self.diagnostic("cancel.enter",None,None,None);
        self.clear_handoff();
        self.watch_rerank(false);
        let updating=self.profile_update.borrow_mut().take().is_some();
        self.close_word();
        self.generation.set(self.generation.get().wrapping_add(1));
        self.recovery.borrow_mut().clear();
        if let Some(watcher) = self.state.borrow().watcher.as_ref() {
            watcher.stop_retry();
        }
        self.mode_revision.set(self.mode_revision.get() + 1);
        unsafe {
            self.indicator.borrow_mut().hide();
        }
        let (context, composition, final_text) = {
            let mut s = self.state.borrow_mut();
            if let Some(context) = s.context.as_ref() {
                unsafe {
                    if let Ok(source) = context.cast::<ITfSource>() {
                        if s.layout_sink != TF_INVALID_COOKIE {
                            let _ = source.UnadviseSink(s.layout_sink);
                        }
                        if s.text_sink != TF_INVALID_COOKIE {
                            let _ = source.UnadviseSink(s.text_sink);
                        }
                    }
                }
            }
            s.layout_sink = TF_INVALID_COOKIE;
            s.text_sink = TF_INVALID_COOKIE;
            let final_text = if s.snapshot.preedit.is_empty() {
                String::new()
            } else if !s.pinyin.pending.is_empty() || [1, 2, 3].contains(&s.special.mode) {
                let final_text = s.snapshot.preedit.clone();
                let _ = s.engine.reset(true);
                final_text
            } else {
                s.engine
                    .reset(false)
                    .map(|o| o.commit)
                    .unwrap_or_else(|| s.snapshot.preedit.clone())
            };
            if updating || s.engine.recovery_pending() {
                s.engine = Engine::default();
                s.preload = None;
            }
            (s.context.clone(), s.composition.take(), final_text)
        };
        if let (Some(context), Some(composition)) = (context, composition) {
            let retirement = self.generation.get();
            let final_length = final_text.encode_utf16().count();
            self.diagnostic_retirement("retirement.queued",retirement,&context,&composition,final_length,None);
            self.retiring
                .borrow_mut()
                .push((retirement, context.clone()));
            let data = self.clone();
            let captured_context = context.clone();
            let captured_composition = composition.clone();
            let requested = self.request(
                &context,
                TF_ES_READWRITE | TF_ES_ASYNCDONTCARE,
                move |cookie| unsafe {
                    data.diagnostic_retirement("retirement.edit.begin",retirement,&captured_context,&composition,final_length,None);
                    if !data
                        .retiring
                        .borrow()
                        .iter()
                        .any(|(id, _)| *id == retirement)
                    {
                        data.diagnostic_retirement("retirement.skipped",retirement,&captured_context,&composition,final_length,None);
                        return Ok(());
                    }
                    let range = composition.GetRange();
                    data.diagnostic_retirement("retirement.range",retirement,&captured_context,&composition,final_length,
                        Some(match &range {Ok(_)=>0,Err(e)=>e.code().0}));
                    let range = range?;
                    data.diagnostic_range("retirement.write.begin",&range,cookie,final_length);
                    let written = range.SetText(cookie, 0, &final_text.encode_utf16().collect::<Vec<_>>());
                    data.diagnostic_retirement("retirement.write.end",retirement,&captured_context,&composition,final_length,
                        Some(match &written {Ok(_)=>0,Err(e)=>e.code().0}));
                    written?;
                    let ended = composition.EndComposition(cookie);
                    data.diagnostic_retirement("retirement.end",retirement,&captured_context,&composition,final_length,
                        Some(match &ended {Ok(_)=>0,Err(e)=>e.code().0}));
                    ended?;
                    data.retiring
                        .borrow_mut()
                        .retain(|(id, _)| *id != retirement);
                    Ok(())
                },
            );
            self.diagnostic_retirement("retirement.request.return",retirement,&context,&captured_composition,final_length,
                Some(match &requested {Ok(_)=>0,Err(e)=>e.code().0}));
            requested?;
        }
        let mut s = self.state.borrow_mut();
        s.context = None;
        s.shift_tap = false;
        s.shift_used = false;
        s.special.clear();
        s.pinyin.clear();
        s.associated.clear();
        s.association_caret = None;
        s.raw_reading.clear();
        s.spacing_prev = None;
        s.snapshot = Snapshot::default();
        if let Some(c) = s.candidates.as_ref() {
            c.data.close();
        }
        Ok(())
    }
    pub fn deactivate(self: &Rc<Self>) -> Result<()> {
        self.diagnostic("deactivate",None,None,None);
        let cancelled=self.cancel();
        // Unadvising TSF and destroying the last native window can synchronously
        // call OnPopContext. Detach ownership before any external cleanup.
        let (thread,client,sinks,bar,button,candidates,watcher,observer)={
            let mut s=self.state.borrow_mut();
            let sinks=[s.thread_sink,s.focus_sink];s.thread_sink=TF_INVALID_COOKIE;s.focus_sink=TF_INVALID_COOKIE;
            let client=std::mem::replace(&mut s.client,0);
            s.park_engine();
            (s.thread.take(),client,sinks,s.bar.take(),s.button.take(),s.candidates.take(),s.watcher.take(),s.observer.take())
        };
        if let Some(thread)=thread {unsafe{
            if let Ok(keys)=thread.cast::<ITfKeystrokeMgr>(){let _=keys.UnadviseKeyEventSink(client);}
            if let Ok(source)=thread.cast::<ITfSource>(){for cookie in sinks{if cookie!=TF_INVALID_COOKIE{let _=source.UnadviseSink(cookie);}}}
        }}
        if let (Some(bar),Some(button))=(&bar,&button){unsafe{let _=bar.RemoveItem(button);}}
        if let Some(c)=candidates{c.data.release();}
        drop((watcher,observer,button,bar));
        cancelled
    }
    pub fn watch(self: &Rc<Self>, context: &ITfContext) -> Result<()> {
        self.diagnostic("watch",Some(context),None,None);
        if self.state.borrow().context.as_ref() == Some(context) {
            return Ok(());
        }
        self.cancel()?;
        let source: ITfSource = context.cast()?;
        let layout = self.owner().cast::<ITfTextLayoutSink>()?;
        let text = self.owner().cast::<ITfTextEditSink>()?;
        unsafe {
            let layout_cookie = source.AdviseSink(&ITfTextLayoutSink::IID, &layout)?;
            let mut s = self.state.borrow_mut();
            s.context = Some(context.clone());
            s.layout_sink = layout_cookie;
            let text_cookie = source.AdviseSink(&ITfTextEditSink::IID, &text)?;
            s.text_sink = text_cookie;
        }
        Ok(())
    }
    pub fn pick(self: &Rc<Self>, index: u32, revision: u64) -> Result<()> {
        let Some(context) = self.state.borrow().context.clone() else {
            return Ok(());
        };
        let data = self.clone();
        let current = context.clone();
        self.request_current(
            &context,
            TF_ES_READWRITE | TF_ES_ASYNCDONTCARE,
            move |cookie| {
                {
                    let s = data.state.borrow();
                    if s.snapshot.reading_hint
                        || s.context.as_ref() != Some(&current)
                        || s.candidates
                            .as_ref()
                            .is_none_or(|c| c.data.revision.get() != revision)
                    {
                        return Ok(());
                    }
                }
                data.dispatch_pick(&current, cookie, index)
            },
        )
    }
    pub(super) fn pick_at(
        self: &Rc<Self>,
        context: &ITfContext,
        cookie: u32,
        index: u32,
    ) -> Result<()> {
        let out = {
            let mut s = self.state.borrow_mut();
            if s.associated.active() {
                pick_associated(&mut s, index)?
            } else if s.special.mode != 0 {
                s.special.pick(index as usize)
            } else {
                output(s.engine.perform(crate::broker::Op::Pick(index)))?
            }
        };
        let conversion =
            !self.state.borrow().snapshot.prediction && !out.associated && !out.preedit.is_empty();
        self.apply(context, cookie, out, conversion)
    }
    pub fn apply(
        self: &Rc<Self>,
        context: &ITfContext,
        cookie: u32,
        out: Snapshot,
        selected: bool,
    ) -> Result<()> {
        self.diagnostic("apply.enter",Some(context),None,None);
        self.watch_rerank(out.rerank && !out.preedit.is_empty());
        #[cfg(feature="input-diagnostics")]
        if crate::input_diagnostics::enabled() {
            crate::input_diagnostics::emit("apply.output",serde_json::json!({
                "handled":out.handled,"commit_utf16":out.commit.encode_utf16().count(),
                "preedit_utf16":out.preedit.encode_utf16().count(),"cursor":out.cursor}));
        }
        unsafe {
            if !out.handled && out.commit.is_empty() && out.preedit.is_empty() {
                // The application handles the key itself, so what is before the cursor is no longer known.
                self.state.borrow_mut().spacing_prev = None;
                return Ok(());
            }
            let mut composition = self.state.borrow().composition.clone();
            let range = if let Some(c) = &composition {
                c.GetRange()?
            } else {
                context
                    .cast::<ITfInsertAtSelection>()?
                    .InsertTextAtSelection(cookie, TF_IAS_QUERYONLY, &[])?
            };
            self.diagnostic_range("apply.initial_range",&range,cookie,0);
            let width = self.state.borrow().full_width;
            let committed = if width {
                out.commit
                    .chars()
                    .map(|c| {
                        if c == ' ' {
                            '\u{3000}'
                        } else if ('!'..='~').contains(&c) {
                            char::from_u32(c as u32 + 0xfee0).unwrap()
                        } else {
                            c
                        }
                    })
                    .collect::<String>()
            } else {
                out.commit.clone()
            };
            // Only the text sent to the application gains the spaces; the core's commit and learning stay as they are.
            let committed = {
                let mut s = self.state.borrow_mut();
                let committed = if s.prefs.flag("input", "cjk_spacing", false) {
                    shanjie_shell::spacing::apply(s.spacing_prev, &committed)
                } else {
                    committed
                };
                if !committed.is_empty() { s.spacing_prev = shanjie_shell::spacing::last(&committed); }
                // A key the application handles itself may move the cursor or type text.
                if !out.handled { s.spacing_prev = None; }
                committed
            };
            if !out.commit.is_empty() {
                // Commit only the final prefix. A remaining preedit gets its
                // own composition and must not be sent as committed text.
                self.diagnostic_range("write.commit.begin",&range,cookie,committed.encode_utf16().count());
                let result=range.SetText(cookie, 0, &committed.encode_utf16().collect::<Vec<_>>());
                self.diagnostic("write.commit.end",Some(context),None,Some(result.as_ref().err().map_or(0,|e|e.code().0)));
                result?;
                if let Some(c) = composition.take() {
                    self.ending.set(true);
                    let result = c.EndComposition(cookie);
                    self.ending.set(false);
                    self.state.borrow_mut().composition = None;
                    result?;
                }
                range.Collapse(cookie, TF_ANCHOR_END)?;
            }
            if !out.preedit.is_empty() {
                if composition.is_none() {
                    let sink: ITfCompositionSink = self.owner().cast()?;
                    self.diagnostic("composition.start.begin",Some(context),None,None);
                    let created = context
                        .cast::<ITfContextComposition>()?
                        .StartComposition(cookie, &range, &sink);
                    self.diagnostic("composition.start.end",Some(context),None,Some(created.as_ref().err().map_or(0,|e|e.code().0)));
                    self.state.borrow_mut().composition = Some(created?);
                }
                // Start before the first write, avoiding TSF's temporary
                // composition that otherwise commits the first character.
                self.diagnostic_range("write.preedit.begin",&range,cookie,out.preedit.encode_utf16().count());
                let result=range.SetText(cookie, 0, &out.preedit.encode_utf16().collect::<Vec<_>>());
                self.diagnostic("write.preedit.end",Some(context),None,Some(result.as_ref().err().map_or(0,|e|e.code().0)));
                result?;
            } else if let Some(c) = composition {
                self.diagnostic_range("write.clear.begin",&range,cookie,0);
                range.SetText(cookie, 0, &[])?;
                self.state.borrow_mut().composition = None;
                self.ending.set(true);
                let result = c.EndComposition(cookie);
                self.ending.set(false);
                result?;
            }
            if out.preedit.is_empty() && !self.state.borrow().snapshot.preedit.is_empty() {
                self.generation.set(self.generation.get().wrapping_add(1));
            }
            // If a later caret update fails, do not repeat text already sent.
            self.application_written.set(true);
            self.state.borrow_mut().snapshot = out.clone();
            let caret = range.Clone()?;
            caret.Collapse(cookie, TF_ANCHOR_START)?;
            let mut moved = 0;
            caret.ShiftEnd(cookie, out.cursor as i32, &mut moved, std::ptr::null())?;
            caret.Collapse(cookie, TF_ANCHOR_END)?;
            let selection = TF_SELECTION {
                range: std::mem::ManuallyDrop::new(Some(caret.clone())),
                style: TF_SELECTIONSTYLE {
                    ase: TF_AE_NONE,
                    fInterimChar: false.into(),
                },
            };
            let mut selections = [selection];
            let result = context.SetSelection(cookie, &selections);
            drop(std::mem::ManuallyDrop::take(&mut selections[0].range));
            self.diagnostic_range("selection.request",&caret,cookie,0);
            self.diagnostic("selection.result",Some(context),None,Some(result.as_ref().err().map_or(0,|e|e.code().0)));
            result?;
            let candidates = {
                let mut s = self.state.borrow_mut();
                s.context = Some(context.clone());
                s.snapshot = out.clone();
                if s.prefs.flag("input", "associations", false)
                    && selected
                    && out.candidates.is_empty()
                    && out.cursor != 0
                    && !out.preedit.is_empty()
                {
                    let before: Vec<_> = out
                        .preedit
                        .encode_utf16()
                        .take(out.cursor as usize)
                        .collect();
                    let text = String::from_utf16(&before).unwrap();
                    let mut chars: Vec<_> = text.chars().rev().take(2).collect();
                    chars.reverse();
                    let prefix = CString::new(chars.iter().collect::<String>()).unwrap();
                    let mut suggestions = output(s.engine.perform(crate::broker::Op::Associate(
                        prefix.to_str().unwrap().into(),
                    )))?;
                    if suggestions.candidates.is_empty() && chars.len() == 2 {
                        let last = CString::new(chars[1].to_string()).unwrap();
                        suggestions =
                            output(s.engine.perform(crate::broker::Op::Associate(
                                last.to_str().unwrap().into(),
                            )))?;
                    }
                    s.associated.open(suggestions.candidates, out);
                    s.snapshot = s.associated.view();
                    s.association_caret = Some(caret.Clone()?);
                }
                s.candidates.as_ref().map(|c| c.data.clone())
            };
            if let Some(candidates) = candidates {
                candidates.update(
                    context,
                    cookie,
                    &caret,
                    self.state.borrow().snapshot.clone(),
                )?;
            }
            Ok(())
        }
    }
    pub fn toggle(self: &Rc<Self>, input: Option<ITfContext>) -> Result<()> {
        let chinese = !self.chinese();
        let (composition, context) = {
            let mut s = self.state.borrow_mut();
            s.associated.clear();
            if let Some(c) = &s.candidates {
                c.data.close();
            }
            (s.composition.clone(), s.context.clone())
        };
        if composition.is_none() {
            self.set_mode(chinese)?;
            return self.notify_mode(input, chinese);
        }
        let context = context.ok_or(E_FAIL)?;
        let current = context.clone();
        let data = self.clone();
        self.request_current(
            &context,
            TF_ES_ASYNCDONTCARE | TF_ES_READWRITE,
            move |cookie| {
                let out = {
                    let mut s = data.state.borrow_mut();
                    let pending =
                        if !s.pinyin.pending.is_empty() || [1, 2, 3].contains(&s.special.mode) {
                            Some(s.snapshot.preedit.clone())
                        } else {
                            None
                        };
                    let mut out = output(s.engine.reset(false))?;
                    if let Some(pending) = pending {
                        out.commit = pending;
                    }
                    s.special.clear();
                    s.pinyin.clear();
                    s.raw_reading.clear();
                    out
                };
                data.apply(&current, cookie, out, false)?;
                data.set_mode(chinese)?;
                data.notify_mode(Some(current), chinese)
            },
        )
    }
    pub fn notify_mode(self: &Rc<Self>, input: Option<ITfContext>, chinese: bool) -> Result<()> {
        if !self
            .state
            .borrow()
            .prefs
            .flag("appearance", "mode_indicator", true)
        {
            return Ok(());
        }
        let revision = self.mode_revision.get() + 1;
        self.mode_revision.set(revision);
        let input = input.or_else(|| unsafe {
            self.state
                .borrow()
                .thread
                .as_ref()?
                .GetFocus()
                .ok()?
                .GetTop()
                .ok()
        });
        let Some(context) = input else {
            return Ok(());
        };
        let current = context.clone();
        let data = self.clone();
        self.request(
            &context,
            TF_ES_READ | TF_ES_ASYNCDONTCARE,
            move |cookie| unsafe {
                if revision != data.mode_revision.get() || data.chinese() != chinese {
                    return Ok(());
                }
                let range = range(selection(&current, cookie)?);
                range.Collapse(cookie, TF_ANCHOR_END)?;
                let view = current.GetActiveView()?;
                let mut caret = RECT::default();
                let mut clipped = BOOL::default();
                view.GetTextExt(cookie, &range, &mut caret, &mut clipped)?;
                if clipped.as_bool() {
                    return Ok(());
                }
                let owner = view.GetWnd()?;
                physical(owner, &mut caret)?;
                data.indicator.borrow_mut().show(
                    caret,
                    if owner.is_invalid() {
                        96
                    } else {
                        GetDpiForWindow(owner)
                    },
                    chinese,
                )
            },
        )
    }
    pub fn layout(self: &Rc<Self>, context: &ITfContext, code: TfLayoutCode) -> Result<()> {
        if code == TF_LC_DESTROY {
            return self.cancel();
        }
        if self.state.borrow().context.as_ref() != Some(context)
            || {
                let s = self.state.borrow();
                s.snapshot.candidates.is_empty() && s.snapshot.preedit.is_empty()
            }
        {
            return Ok(());
        }
        let data = self.clone();
        let current = context.clone();
        self.request_current(
            context,
            TF_ES_READ | TF_ES_ASYNCDONTCARE,
            move |cookie| unsafe {
                if data.state.borrow().context.as_ref() != Some(&current) {
                    return Ok(());
                }
                let range = range(selection(&current, cookie)?);
                range.Collapse(cookie, TF_ANCHOR_END)?;
                let (c, state) = {
                    let s = data.state.borrow();
                    (
                        s.candidates.as_ref().map(|c| c.data.clone()),
                        s.snapshot.clone(),
                    )
                };
                if let Some(c) = c {
                    c.update(&current, cookie, &range, state)?;
                }
                Ok(())
            },
        )
    }
    pub fn refresh(self: &Rc<Self>) -> Result<()> {
        if self.profile_update.borrow().is_some() {
            self.recovery.borrow_mut().refresh=true;
            self.wake_profile_update();
            self.schedule_retry();
            return Ok(());
        }
        if !self.recovery.borrow().is_empty() {
            self.recovery.borrow_mut().refresh = true;
            return Ok(());
        }
        let current = profile::load(&profile::path()).map_err(|_| Error::from_hresult(E_FAIL))?;
        let old = self.state.borrow().prefs.clone();
        let rebuild = profile::REBUILD
        .iter()
        .any(|(s, k)| current.get(s, k, "") != old.get(s, k, ""));
        let mode_changed = current.get("input", "chinese", "") != old.get("input", "chinese", "");
        let width_changed =
            current.get("input", "full_width", "") != old.get("input", "full_width", "");
        if rebuild || mode_changed {
            self.cancel()?;
        }
        let prediction_changed =
            current.get("engine", "prediction", "") != old.get("engine", "prediction", "");
        let demote_changed = current.get("engine", "demote", "") != old.get("engine", "demote", "");
        {
            let mut s = self.state.borrow_mut();
            s.prefs = current.clone();
            if rebuild {
                s.engine = Engine::default();
                s.preload = None;
                s.pinyin = Pinyin::default();
            }
            if width_changed {
                s.full_width = current.flag("input", "full_width", false);
            }
            if !current.flag("input", "associations", false) && s.associated.active() {
                s.associated.clear();
                s.association_caret = None;
                s.snapshot.associated = false;
                s.snapshot.candidates.clear();
                s.snapshot.selected = None;
                if let Some(c) = &s.candidates {
                    c.data.close();
                }
            }
            if let Some(c) = &s.candidates {
                c.data.theme.set(current.number("appearance", "theme", 0));
                c.data
                    .size
                    .set(current.number("appearance", "font_size", 18));
                c.data
                    .vertical
                    .set(current.get("candidates", "layout", "horizontal") == "vertical");
                c.data
                    .anchor_right
                    .set(current.get("candidates", "anchor", "right") == "right");
            }
        }
        if let Some(popup)=self.word_popup.borrow().as_ref(){let _=popup.set_theme(current.number("appearance","theme",0));}
        self.prepare_engine()?;
        if self.state.borrow().engine.is_open() {
            let mut operations=std::collections::VecDeque::from(option_operations(&current));
            if demote_changed {operations.push_back(crate::broker::Op::Demote(current.flag("engine","demote",true)));}
            if prediction_changed {operations.push_back(crate::broker::Op::Prediction(current.flag("engine","prediction",false)));}
            let context=self.state.borrow().context.clone();
            if prediction_changed || demote_changed {
                let mut s=self.state.borrow_mut();s.associated.clear();s.association_caret=None;
            }
            *self.profile_update.borrow_mut()=Some(super::profile_refresh::Pending::new(context,self.generation.get(),operations));
            self.retry_profile_update();
        }
        if mode_changed || width_changed {
            self.set_mode(if mode_changed {
                self.state.borrow().prefs.flag("input", "chinese", true)
            } else {
                self.chinese()
            })?;
        }
        let context = self.state.borrow().context.clone();
        if let Some(context) = context.filter(|_|self.profile_update.borrow().is_none()) {
            self.layout(&context, TF_LC_CHANGE)?;
        }
        Ok(())
    }
    pub fn restart(self: &Rc<Self>) -> Result<()> {
        self.cancel()?;
        {
            let mut s = self.state.borrow_mut();
            s.engine = Engine::default();
            s.preload = None;
            s.pinyin = Pinyin::default();
        }
        resident::invalidate();
        self.refresh()
    }
    fn prepare_engine(&self) -> Result<()> {
        let mut s = self.state.borrow_mut();
        if !s.engine.is_open() {
            let config = s.core_config();
            if !s.preload.as_ref().is_some_and(|p| p.matches(&config)) {
                s.preload = Some(resident::prepare(config)?);
            }
        }
        Ok(())
    }
    pub fn ensure_engine(&self) -> Result<()> {
        self.prepare_engine()?;
        let mut s = self.state.borrow_mut();
        if !s.engine.is_open() {
            // Activation already started the load. An unusually early key joins
            // that same job; it never starts another copy on the input thread.
            let (engine, pinyin) = s.preload.take().and_then(|p| p.take()).ok_or(E_FAIL)?;
            s.engine = engine;
            s.pinyin = pinyin;
            configure_options(&mut s)?;
            let demote = s.prefs.flag("engine", "demote", true);
            output(s.engine.perform(crate::broker::Op::Demote(demote)))?;
        }
        Ok(())
    }
}
pub fn pick_associated(s: &mut State, index: u32) -> Result<Snapshot> {
    let text = s.associated.pick(index as usize);
    if text.associated {
        return Ok(text);
    }
    let mut out = output(s.engine.reset(false))?;
    let mut units: Vec<_> = out.commit.encode_utf16().collect();
    let at = (s.snapshot.cursor as usize).min(units.len());
    units.splice(at..at, text.commit.encode_utf16());
    out.commit = String::from_utf16(&units).unwrap();
    out.handled = true;
    s.association_caret = None;
    Ok(out)
}
pub(super) unsafe fn physical(owner: HWND, caret: &mut RECT) -> Result<()> {
    if !owner.is_invalid()
        && GetAwarenessFromDpiAwarenessContext(GetWindowDpiAwarenessContext(owner))
            != DPI_AWARENESS_PER_MONITOR_AWARE
    {
        let mut start = POINT {
            x: caret.left,
            y: caret.top,
        };
        let mut end = POINT {
            x: caret.right,
            y: caret.bottom,
        };
        LogicalToPhysicalPointForPerMonitorDPI(Some(owner), &mut start).ok()?;
        LogicalToPhysicalPointForPerMonitorDPI(Some(owner), &mut end).ok()?;
        *caret = RECT {
            left: start.x,
            top: start.y,
            right: end.x,
            bottom: end.y,
        };
    }
    Ok(())
}
fn configure_options(s: &mut State) -> Result<()> {
    if !s.engine.is_open() {
        return Ok(());
    }
    for op in option_operations(&s.prefs) {output(s.engine.perform(op))?;}
    Ok(())
}
fn option_operations(p:&Document) -> [crate::broker::Op;4] {
    let keyboard=p.number("input","keyboard",0);
    let zhuyin=keyboard<3;
    let upstream=zhuyin && keyboard!=2 && p.flag("input","upstream_english",false);
    let inline=p.flag("input","english_inline",true);
    [crate::broker::Op::Options(p.input_options()),
     crate::broker::Op::English(upstream as u32 | (inline as u32)<<1),
     crate::broker::Op::Mixed(zhuyin && p.flag("input","mixed_input",false)),
     crate::broker::Op::Tolerance(zhuyin && p.flag("input","mixed_tolerance",false))]
}
unsafe extern "C" fn changed(context: *mut c_void, kind: u32) {
    let weak = &*(context as *const std::rc::Weak<Data>);
    if let Some(data) = weak.upgrade() {
        match kind {
            1 => {
                let _ = data.restart();
            }
            2 => data.retry_pending(),
            4 => data.open_word_popup(),
            5 => data.expire_handoff(),
            6 => { let _=data.read_word(); },
            7 => data.fallback_word(),
            8 => data.poll_word(),
            9 => data.word_closed(),
            10 => data.poll_rerank(),
            _ => {
                let _ = data.refresh();
            }
        }
    }
}
impl ITfTextInputProcessor_Impl for Service_Impl {
    fn Activate(&self, thread: Ref<ITfThreadMgr>, client: u32) -> Result<()> {
        ITfTextInputProcessorEx_Impl::ActivateEx(self, thread, client, 0)
    }
    fn Deactivate(&self) -> Result<()> {
        self.data.deactivate()
    }
}
impl ITfTextInputProcessorEx_Impl for Service_Impl {
    fn ActivateEx(&self, thread: Ref<ITfThreadMgr>, client: u32, _: u32) -> Result<()> {
        self.data.diagnostic("activate.enter",None,None,None);
        let result = (|| unsafe {
            let thread = thread.ok()?.clone();
            let preferences =
                profile::load(&profile::path()).map_err(|_| Error::from_hresult(E_FAIL))?;
            let mut path = [0u16; 32768];
            let length = windows::Win32::System::LibraryLoader::GetModuleFileNameW(
                Some(super::module()),
                &mut path,
            );
            if length == 0 {
                return Err(E_FAIL.into());
            }
            let binary = PathBuf::from(String::from_utf16(&path[..length as usize]).unwrap())
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .to_owned();
            let mut layout = 0;
            let profiles: ITfInputProcessorProfileMgr =
                CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
            let mut active = TF_INPUTPROCESSORPROFILE::default();
            if profiles
                .GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut active)
                .is_ok()
                && active.guidProfile == GUID::from_u128(0x338145b4047a4564a0de223dc22c1ae1)
            {
                layout = 1;
            }
            {
                let mut s = self.data.state.borrow_mut();
                s.thread = Some(thread.clone());
                s.client = client;
                s.prefs = preferences.clone();
                s.binary = binary.clone();
                s.data = binary.join("data");
                s.profile_layout = layout;
                s.full_width = preferences.flag("input", "full_width", false);
                s.observer = super::modifiers::Observer::new().ok();
            }
            self.data
                .set_mode(preferences.flag("input", "chinese", true))
                .map_err(|e| {
                    crate::bridge::failure(format!("initial input compartment: {e}"));
                    e
                })?;
            let candidates = candidates::new(
                &thread,
                Rc::downgrade(&self.data),
                preferences.number("appearance", "theme", 0),
                preferences.number("appearance", "font_size", 18),
            )?;
            candidates
                .data
                .vertical
                .set(preferences.get("candidates", "layout", "horizontal") == "vertical");
            candidates
                .data
                .anchor_right
                .set(preferences.get("candidates", "anchor", "right") == "right");
            self.data.state.borrow_mut().candidates = Some(candidates);
            let keys: ITfKeystrokeMgr = thread.cast()?;
            keys.AdviseKeyEventSink(client, &self.to_interface::<ITfKeyEventSink>(), true)?;
            let source: ITfSource = thread.cast()?;
            let thread_sink = source.AdviseSink(
                &ITfThreadMgrEventSink::IID,
                &self.to_interface::<ITfThreadMgrEventSink>(),
            )?;
            self.data.state.borrow_mut().thread_sink = thread_sink;
            let focus_sink = source.AdviseSink(
                &ITfThreadFocusSink::IID,
                &self.to_interface::<ITfThreadFocusSink>(),
            )?;
            {
                let mut s = self.data.state.borrow_mut();
                s.thread_sink = thread_sink;
                s.focus_sink = focus_sink;
            }
            let weak = Box::new(Rc::downgrade(&self.data));
            let watcher = ControlChannel::new(
                changed,
                (&*weak as *const std::rc::Weak<Data>).cast_mut().cast(),
            )?; // ControlChannel context is retained alongside its native window.
            self.data.state.borrow_mut().watcher = Some(watcher);
            self.data.watcher_context.replace(Some(weak));
            if let Ok(bar) = thread.cast::<ITfLangBarItemMgr>() {
                let button = mode::new(Rc::downgrade(&self.data));
                // TSF may host text input without exposing a language bar (including the unregistered harness).
                if bar.AddItem(&button).is_ok() {
                    let mut s = self.data.state.borrow_mut();
                    s.bar = Some(bar);
                    s.button = Some(button);
                }
            }
            self.data.prepare_engine()?;
            Ok(())
        })();
        if let Err(e) = &result {
            super::trace(format_args!("ActivateEx: {e}"));
            let _ = self.data.deactivate();
        }
        result
    }
}
impl ITfCompositionSink_Impl for Service_Impl {
    fn OnCompositionTerminated(&self, _: u32, ended: Ref<ITfComposition>) -> Result<()> {
        self.data.diagnostic("composition.terminated",None,None,Some((self.data.state.borrow().composition.as_ref()==ended.as_ref()) as i32));
        if !self.data.ending.get()
            && self.data.state.borrow().composition.as_ref() == ended.as_ref()
        {
            self.data.state.borrow_mut().composition = None;
            self.data.cancel()?;
        }
        Ok(())
    }
}
impl ITfThreadFocusSink_Impl for Service_Impl {
    fn OnSetThreadFocus(&self) -> Result<()> {
        self.data.diagnostic("thread_focus.gained",None,None,None);
        Ok(())
    }
    fn OnKillThreadFocus(&self) -> Result<()> {
        self.data.diagnostic("thread_focus.lost",None,None,None);
        self.data.cancel()
    }
}
impl ITfThreadMgrEventSink_Impl for Service_Impl {
    fn OnInitDocumentMgr(&self, _: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnUninitDocumentMgr(&self, _: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnSetFocus(
        &self,
        current: Ref<ITfDocumentMgr>,
        previous: Ref<ITfDocumentMgr>,
    ) -> Result<()> {
        self.data.diagnostic("document_focus",None,None,Some((current.as_ref()!=previous.as_ref()) as i32));
        self.data.diagnostic_focus(current.as_ref(),previous.as_ref());
        self.data.document_focus(current.as_ref(),previous.as_ref())
    }
    fn OnPushContext(&self, context: Ref<ITfContext>) -> Result<()> {
        self.data.diagnostic("context.push",context.as_ref(),None,None);
        Ok(())
    }
    fn OnPopContext(&self, context: Ref<ITfContext>) -> Result<()> {
        self.data.diagnostic("context.pop",context.as_ref(),None,None);
        if self.data.state.borrow().context.as_ref() == context.as_ref() {
            self.data.cancel()?;
        }
        Ok(())
    }
}
impl ITfTextLayoutSink_Impl for Service_Impl {
    fn OnLayoutChange(
        &self,
        context: Ref<ITfContext>,
        code: TfLayoutCode,
        _: Ref<ITfContextView>,
    ) -> Result<()> {
        if !self.data.editing.get() {
            self.data.layout(context.ok()?, code)?;
        }
        Ok(())
    }
}
impl ITfTextEditSink_Impl for Service_Impl {
    fn OnEndEdit(
        &self,
        context: Ref<ITfContext>,
        cookie: u32,
        record: Ref<ITfEditRecord>,
    ) -> Result<()> {
        self.data.diagnostic("edit.notification",context.as_ref(),None,None);
        unsafe {
            if self.data.editing.get()
                || self.data.state.borrow().context.as_ref() != context.as_ref()
            {
                return Ok(());
            }
            let record=record.ok()?;
            let selection_changed=record.GetSelectionStatus()?.as_bool();
            self.data.diagnostic("edit.selection_changed",context.as_ref(),None,Some(selection_changed as i32));
            let word_open=self.data.word_popup.borrow().as_ref().is_some_and(|p|p.active())
                || self.data.word_selection.borrow().is_some() || self.data.word_lookup.borrow().is_some();
            let text_changed=word_open && record.GetTextAndPropertyUpdates(TF_GTP_INCL_TEXT,&[]).is_ok_and(|updated| {
                let mut ranges=[None]; let mut count=0;
                updated.Next(&mut ranges,&mut count).is_ok() && count!=0
            });
            if selection_changed || text_changed { self.data.close_word(); }
            if !selection_changed { return Ok(()); }
            let context = context.ok()?;
            let selected = selection(context, cookie)?;
            let anchor = if selected.style.ase == TF_AE_START {
                TF_ANCHOR_START
            } else {
                TF_ANCHOR_END
            };
            let caret = range(selected);
            self.data.diagnostic_range("selection.host",&caret,cookie,0);
            let (composition, association) = {
                let s = self.data.state.borrow();
                (s.composition.clone(), s.association_caret.clone())
            };
            if let Some(composition) = composition {
                caret.Collapse(cookie, anchor)?;
                let range = composition.GetRange()?;
                let before = caret.CompareStart(cookie, &range, TF_ANCHOR_START)?;
                let after = caret.CompareStart(cookie, &range, TF_ANCHOR_END)?;
                if before < 0 || after > 0 {
                    return self.data.cancel();
                }
                let left = range.Clone()?;
                left.ShiftEndToRange(cookie, &caret, TF_ANCHOR_START)?;
                let size = self
                    .data
                    .state
                    .borrow()
                    .snapshot
                    .preedit
                    .encode_utf16()
                    .count();
                let mut text = vec![0u16; size];
                let mut target = 0;
                left.GetText(cookie, 0, &mut text, &mut target)?;
                let mut s = self.data.state.borrow_mut();
                if target == s.snapshot.cursor {
                    return Ok(());
                }
                if s.associated.active() {
                    s.associated.clear();
                    s.association_caret = None;
                } else if !s.snapshot.candidates.is_empty() {
                    output(s.engine.key(ShanjieKey {
                        kind: 6,
                        ch: 0,
                        modifiers: 0,
                    }))?;
                }
                let mut out = output(s.engine.key(ShanjieKey {
                    kind: 11,
                    ch: 0,
                    modifiers: 0,
                }))?;
                while out.cursor < target {
                    let next = output(s.engine.key(ShanjieKey {
                        kind: 8,
                        ch: 0,
                        modifiers: 0,
                    }))?;
                    if next.cursor <= out.cursor {
                        break;
                    }
                    out = next;
                }
                s.snapshot = out;
                if let Some(c) = &s.candidates {
                    c.data.close();
                }
            } else if let Some(association) = association {
                let start = caret.CompareStart(cookie, &association, TF_ANCHOR_START)?;
                let end = caret.CompareEnd(cookie, &association, TF_ANCHOR_END)?;
                if start != 0 || end != 0 {
                    let mut s = self.data.state.borrow_mut();
                    s.associated.clear();
                    s.association_caret = None;
                    s.snapshot = Snapshot::default();
                    if let Some(c) = &s.candidates {
                        c.data.close();
                    }
                }
            }
            Ok(())
        }
    }
}
