// Only immutable lexicon/model resources are shared. Composition and learning
// policy stay in each Engine, so two clients cannot see each other's text.
use std::sync::{Mutex, OnceLock};
static WINDOWS_LEX_LOADS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static WINDOWS_MODEL_LOADS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
type LexCache = Vec<(
    Vec<(std::path::PathBuf, u64, Option<std::time::SystemTime>)>,
    Arc<Lexicon>,
)>;
fn resource_stamp(
    data: &Path,
    personal: Option<&Path>,
) -> Result<Vec<(std::path::PathBuf, u64, Option<std::time::SystemTime>)>, EngineError> {
    let mut paths: Vec<_> = [
        "mcbpmf-data.txt",
        "overlay-add.tsv",
        "sandhi-add.tsv",
        "names-add.tsv",
        "demote.tsv",
    ]
    .iter()
    .map(|name| data.join(name))
    .collect();
    if let Some(p) = personal {
        paths.push(p.to_owned());
    }
    paths
        .into_iter()
        .map(|p| {
            let m = std::fs::metadata(&p).map_err(|_| EngineError::LoadFailed)?;
            Ok((p, m.len(), m.modified().ok()))
        })
        .collect()
}
impl Engine {
    pub fn new_with_personal(
        data: &Path,
        layout: Layout,
        personal: Option<&Path>,
    ) -> Result<Engine, EngineError> {
        static CACHE: OnceLock<Mutex<LexCache>> = OnceLock::new();
        let stamp = resource_stamp(data, personal)?;
        let mut cache = CACHE
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| EngineError::LoadFailed)?;
        if let Some(index) = cache.iter().position(|(key, _)| key == &stamp) {
            let entry = cache.remove(index);
            let mut engine = Engine::with_lexicon(entry.1.clone(), layout);
            engine.data_dir = Some(data.to_owned());
            cache.push(entry);
            return Ok(engine);
        }
        let engine = Engine::new_with_personal_uncached(data, layout, personal)?;
        WINDOWS_LEX_LOADS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if cache.len() == 4 {
            cache.remove(0);
        }
        cache.push((stamp, engine.lex.clone()));
        Ok(engine)
    }
    pub fn load_lm(&mut self, path: &Path) -> Result<(), EngineError> {
        // Test fixture binary models keep their original behavior.
        if path.extension().is_none_or(|x| x != "winmap") {
            return self.load_lm_uncached(path);
        }
        type Models = Vec<(
            Vec<(std::path::PathBuf, u64, Option<std::time::SystemTime>)>,
            Arc<Lexicon>,
            LmState,
        )>;
        static CACHE: OnceLock<Mutex<Models>> = OnceLock::new();
        // The class pairing and optional side file are part of the load contract,
        // even after another session already holds a valid mapped model.
        let folder = path.parent().ok_or(EngineError::LoadFailed)?;
        let mut paths = vec![path.to_owned(), folder.join("classes.sjc")];
        let side = folder.join("kn.sjkn");
        if side.exists() {
            paths.push(side);
        }
        let stamp: Vec<_> = paths
            .into_iter()
            .map(|p| {
                let m = std::fs::metadata(&p).map_err(|_| EngineError::LoadFailed)?;
                Ok((p, m.len(), m.modified().ok()))
            })
            .collect::<Result<_, EngineError>>()?;
        let mut cache = CACHE
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| EngineError::LoadFailed)?;
        if let Some(index) = cache
            .iter()
            .position(|(key, lex, _)| key == &stamp && Arc::ptr_eq(lex, &self.lex))
        {
            let entry = cache.remove(index);
            if self.predict_on {
                entry.2.capped.predict_index(&entry.2.lm);
            }
            self.lm = Some(entry.2.clone());
            cache.push(entry);
            return Ok(());
        }
        self.load_lm_uncached(path)?;
        WINDOWS_MODEL_LOADS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if cache.len() == 4 {
            cache.remove(0);
        }
        cache.push((stamp, self.lex.clone(), self.lm.as_ref().unwrap().clone()));
        Ok(())
    }
}

/// Read-only word checks share the same immutable resources as input sessions.
/// The base lexicon excludes personal additions and LM vocabulary expansion.
pub fn windows_base_lexicon(data:&Path)->Result<Arc<Lexicon>,EngineError> {
    Ok(Engine::new_with_personal(data,Layout::Standard,None)?.lex.clone())
}
pub fn windows_model_contains(data:&Path,path:&Path,word:&str)->Result<bool,EngineError> {
    let mut engine=Engine::new_with_personal(data,Layout::Standard,None)?;
    engine.load_lm(path)?;
    Ok(engine.lm.as_ref().is_some_and(|state|state.lm.count(word)>0))
}
