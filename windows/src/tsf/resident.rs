//! Process-local warm core. No TSF/COM objects or HWNDs cross threads.
use crate::{engine::Engine, input::Pinyin, profile::Document};
use std::{
    path::{Path, PathBuf},
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
    layout: u32,
    ranking: u32,
    right: bool,
    learning: bool,
    big5: bool,
    dictionary: String,
    prediction: bool,
    classes: u32,
}
impl Config {
    pub fn new(data: &Path, p: &Document, profile_layout: u32) -> Self {
        let available = crate::installation::available();
        let keyboard = crate::installation::keyboard(p.number("input", "keyboard", 0), available);
        Self {
            data: data.to_owned(),
            keyboard,
            layout: crate::installation::layout(keyboard, profile_layout, available),
            ranking: (p.get("engine", "ranking", "chat") == "formal") as u32,
            right: p.get("candidates", "anchor", "right") == "right",
            learning: p.flag("input", "learning", true),
            big5: p.flag("candidates", "big5", false),
            dictionary: p.get("input", "dictionary", "dictionary.tsv").to_owned(),
            prediction: p.flag("engine", "prediction", false),
            classes: p.class_table(),
        }
    }
    fn load(&self) -> Option<(Engine, Pinyin)> {
        let engine = Engine::open_variant(
            &self.data,
            self.layout,
            self.ranking,
            self.right,
            self.learning,
            self.big5,
            &self.dictionary,
            self.prediction,
            self.classes,
        )?;
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
    let (sender, receiver) = mpsc::channel();
    let input = config.clone();
    LOADS.fetch_add(1, Ordering::Release);
    std::thread::Builder::new()
        .name("Shanjie core preload".into())
        .spawn(move || {
            let result = input.load();
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
            crate::bridge::failure(format!("engine preload: {}", self.config.data.display()));
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
