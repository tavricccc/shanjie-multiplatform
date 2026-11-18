//! Process-local warm core. No TSF/COM objects or HWNDs cross threads.
use super::service::{option_operations, Data};
use crate::{broker::Op, engine::{BuildSource, Engine}, input::Pinyin, profile::Document};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc, Mutex, OnceLock,
    },
};
use windows::{
    core::{Result, PCWSTR},
    Win32::{
        Foundation::HMODULE,
        System::LibraryLoader::{
            GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            GET_MODULE_HANDLE_EX_FLAG_PIN,
        },
    },
};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Config {
    data: PathBuf,
    keyboard: u32,
    // A replaced package under the same id must not reuse the spare engine.
    model_revision: String,
    engine: crate::broker::Config,
}
impl Config {
    pub fn new(data: &Path, p: &Document, profile_layout: u32) -> Self {
        let available = crate::installation::available();
        let keyboard = crate::installation::keyboard(p.number("input", "keyboard", 0), available);
        let layout = crate::installation::layout(keyboard, profile_layout, available);
        Self {
            data: data.to_owned(),
            keyboard,
            model_revision: p.get("engine", "model_revision", "").into(),
            engine: crate::broker::Config { ranking: p.ranking(program().as_deref()), ..crate::broker::Config::from_profile(p, layout) },
        }
    }
    fn load(&self, source: Option<BuildSource>) -> Option<(Engine, Pinyin)> {
        let engine = Engine::open_reusing(&self.data, &self.engine, source)?;
        let pinyin = if self.keyboard >= 3 {
            Pinyin::load(&self.data, self.keyboard == 4).ok()?
        } else {
            Pinyin::default()
        };
        Some((engine, pinyin))
    }
}

enum Load {
    Loading(mpsc::Receiver<Option<(Engine, Pinyin)>>),
    Ready(Engine, Pinyin),
}
pub(super) struct Pending {
    config: Config,
    load: Load,
}
/// The executable this input method is loaded into, for `[engine] ranking=auto`; never logged or sent to the core.
fn program() -> Option<String> {
    Some(std::env::current_exe().ok()?.file_name()?.to_str()?.to_owned())
}
// Keep one spare core per process, including a load still in flight. Active
// services own separate engines; composition state is never shared.
static SPARE: Mutex<Option<Pending>> = Mutex::new(None);
static PINNED: OnceLock<()> = OnceLock::new();
static LOADS: AtomicU32 = AtomicU32::new(0);
static COMPLETED: AtomicU32 = AtomicU32::new(0);
pub fn started() -> bool {
    PINNED.get().is_some()
}

// Background diagnostics: attempts in low 32 bits, finished jobs in high 32.
// No text, paths or user data are exposed.
#[no_mangle]
pub extern "C" fn sj_resident_status() -> u64 {
    ((COMPLETED.load(Ordering::Acquire) as u64) << 32) | LOADS.load(Ordering::Acquire) as u64
}

pub(super) fn prepare(config: Config) -> Result<Pending> {
    if !started() {
        // Workers and the spare core live beyond a COM service. Pin the DLL
        // until process exit, including when a host explicitly FreeLibrary's it.
        unsafe {
            let mut module = HMODULE::default();
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                PCWSTR(super::DllGetClassObject as *const u16),
                &mut module,
            )?;
        }
        let _ = PINNED.set(());
    }
    let spare = SPARE.lock().unwrap().take();
    if let Some(spare) = spare {
        if spare.config == config {
            return Ok(spare);
        }
    }
    spawn(config, |input| input.load(None))
}
fn spawn(config: Config, load: impl FnOnce(Config) -> Option<(Engine, Pinyin)> + Send + 'static) -> Result<Pending> {
    let (sender, receiver) = mpsc::channel();
    let input = config.clone();
    LOADS.fetch_add(1, Ordering::Release);
    std::thread::Builder::new()
        .name("Shanjie core preload".into())
        .spawn(move || {
            let result = load(input);
            let _ = sender.send(result);
            COMPLETED.fetch_add(1, Ordering::Release);
        })
        .map_err(|_| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?;
    Ok(Pending {
        config,
        load: Load::Loading(receiver),
    })
}
impl Pending {
    pub fn matches(&self, config: &Config) -> bool {
        &self.config == config
    }
    pub fn take(self) -> Option<(Engine, Pinyin)> {
        let result = match self.load {
            Load::Ready(engine, pinyin) => Some((engine, pinyin)),
            Load::Loading(receiver) => receiver.recv().ok().flatten(),
        };
        if result.is_none() {
            crate::runtime_log::io("preload.empty", &std::io::Error::other("Engine unavailable"));
        }
        result
    }
    pub fn park(self) {
        *SPARE.lock().unwrap() = Some(self);
    }
}
pub(super) fn park(config: Config, mut engine: Engine, mut pinyin: Pinyin) {
    // A new TSF service must not inherit uncommitted text or prediction state.
    engine.reset(true);
    pinyin.clear();
    Pending {
        config,
        load: Load::Ready(engine, pinyin),
    }
    .park();
}
pub(super) fn invalidate() {
    *SPARE.lock().unwrap() = None;
}

pub(super) fn retire(engine: Engine) {
    // Ownership has left the TSF service. Free memory and close the pipe off its thread.
    let _ = std::thread::Builder::new().name("Shanjie core retire".into()).spawn(move || drop(engine));
}

/// At most one worker; a newer profile invalidates the result even before its own job starts.
#[derive(Default)]
pub(super) struct Rebuild {
    generation: u64,
    desired: Option<Document>,
    building: Option<(u64, Pending)>,
    ready: Option<(Engine, Pinyin)>,
}
impl Rebuild {
    pub fn request(&mut self, desired: Option<Document>) {
        if self.desired.as_ref().map(|p| &p.0) == desired.as_ref().map(|p| &p.0) {
            return;
        }
        self.generation += 1;
        self.desired = desired;
        if let Some((engine, _)) = self.ready.take() { retire(engine); }
    }

    pub fn reset(&mut self) {
        self.request(None);
        self.building = None;
    }
}

impl Data {
    pub(super) fn poll_rebuild(self: &Rc<Self>) -> Result<()> {
        let mut slot = self.rebuild.borrow_mut();
        if let Some((generation, build)) = slot.building.as_ref() {
            let Load::Loading(receiver) = &build.load else { unreachable!("rebuilds start on workers") };
            let received = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(None),
            };
            if let Some(result) = received {
                let latest = *generation == slot.generation;
                slot.building.take();
                if latest {
                    slot.ready = result;
                    if slot.ready.is_none() {
                        slot.desired = None;
                        crate::bridge::failure("無法套用輸入法設定，已保留目前的引擎。");
                    }
                } else if let Some((engine, _)) = result {
                    retire(engine);
                }
            }
        }
        if slot.building.is_none() && slot.ready.is_none() {
            if let Some(prefs) = slot.desired.clone() {
                let (config, source) = {
                    let state = self.state.borrow();
                    let old = state.core_config();
                    let config = Config::new(&state.data, &prefs, state.profile_layout);
                    // A changed package under the same ID must load from disk.
                    let source = (old.engine.model == config.engine.model && old.model_revision == config.model_revision)
                        .then(|| state.engine.build_source()).flatten();
                    (config, source)
                };
                match spawn(config, move |input| {
                    let (mut engine, pinyin) = input.load(source)?;
                    for op in option_operations(&prefs) { engine.perform(op)?; }
                    engine.perform(Op::Demote(prefs.flag("engine", "demote", true)))?;
                    Some((engine, pinyin))
                }) {
                    Ok(build) => slot.building = Some((slot.generation, build)),
                    Err(error) => {
                        slot.desired = None;
                        super::trace(format_args!("engine rebuild worker: {error}"));
                        crate::bridge::failure("無法啟動引擎建置，已保留目前的引擎。");
                    }
                }
            }
        }
        let input_ready = {
            let recovery = self.recovery.borrow();
            recovery.is_empty() && !recovery.refresh
        };
        let can_swap = slot.ready.is_some() && input_ready
            && self.profile_update.borrow().is_none() && {
                let s = self.state.borrow();
                s.composition.is_none() && s.snapshot.preedit.is_empty()
                    && s.pinyin.pending.is_empty() && s.special.mode == 0
            };
        let swap = if can_swap { slot.desired.take().zip(slot.ready.take()) } else { None };
        if let Some(watcher) = self.state.borrow().watcher.as_ref() {
            watcher.rebuild_poll(slot.desired.is_some() || slot.building.is_some());
        }
        drop(slot);
        if let Some((prefs, engine)) = swap {
            self.apply_profile(prefs, false, Some(engine))?;
        }
        Ok(())
    }
}
