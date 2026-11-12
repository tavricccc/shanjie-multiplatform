// Only immutable lexicon/model resources are shared. Composition and learning
// policy stay in each Engine, so two clients cannot see each other's text.
use std::sync::{Mutex, OnceLock};
static WINDOWS_LEX_LOADS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static WINDOWS_MODEL_LOADS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
mod windows_pack_cache_tests {
    use super::*;

    #[test]
    fn personal_and_cached_lexicons_keep_the_pack_oov_prefix() {
        let dir = std::env::temp_dir().join(format!("shanjie-pack-cache-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(dir.join("packs")).unwrap();
        std::fs::write(dir.join("mcbpmf-data.txt"), "ㄋㄧˇ 你 -1\n").unwrap();
        for name in ["overlay-add.tsv", "sandhi-add.tsv", "names-add.tsv", "demote.tsv", "variant-order.tsv", "rerank-protect.tsv", "std-exempt.tsv"] {
            std::fs::write(dir.join(name), "").unwrap();
        }
        std::fs::write(dir.join("packs/acg-add.tsv"), "ㄇㄠ-ㄆㄠ-ㄇㄠ\t貓泡貓\t-7\tacg\n").unwrap();
        let personal = dir.join("personal.tsv");
        std::fs::write(&personal, "ㄋㄧˇ 妳 -2\n").unwrap();
        for personal in [None, Some(personal.as_path())] {
            let first = Engine::new_with_personal(&dir, Layout::Standard, personal, PACK_ACG).unwrap();
            let cached = Engine::new_with_personal(&dir, Layout::Standard, personal, PACK_ACG).unwrap();
            assert!(Arc::ptr_eq(&first.lex, &cached.lex));
            assert!(first.pack_oov_len > 0);
            assert_eq!(cached.pack_oov_len, first.pack_oov_len);
            assert_eq!(cached.pack_text.as_deref(), first.pack_text.as_deref());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
type LexCache = Vec<(
    (u32, Vec<(std::path::PathBuf, u64, Option<std::time::SystemTime>)>),
    Arc<Lexicon>,
    // Pack rows are part of the capped lexicon later built from this lexicon, so they travel with it.
    (Option<String>, usize),
    // The optional digit count table read from the same data folder (upstream shares it across engines).
    Option<Arc<DigitTable>>,
)>;
fn resource_stamp(
    data: &Path,
    personal: Option<&Path>,
    packs: u32,
) -> Result<Vec<(std::path::PathBuf, u64, Option<std::time::SystemTime>)>, EngineError> {
    let mut paths: Vec<_> = [
        "mcbpmf-data.txt",
        "overlay-add.tsv",
        "sandhi-add.tsv",
        "names-add.tsv",
        "demote.tsv",
        "variant-order.tsv",
        "rerank-protect.tsv",
        "std-exempt.tsv",
    ]
    .iter()
    .map(|name| data.join(name))
    .collect();
    if let Some(p) = personal {
        paths.push(p.to_owned());
    }
    // Optional like the pack: present files are part of the key, a missing one is not an error.
    let digits = data.join("digits.tsv");
    if digits.is_file() {
        paths.push(digits);
    }
    // A missing pack file contributes nothing, so it is absent from the stamp rather than an error.
    for (flag, name) in [(PACK_ACG, "acg-add.tsv"), (PACK_QUOTES, "quotes-add.tsv"), (PACK_IT, "it-add.tsv"), (PACK_BEAUTY, "beauty-add.tsv")] {
        let pack = data.join("packs").join(name);
        if packs & flag != 0 && pack.is_file() {
            paths.push(pack);
        }
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
        packs: u32,
    ) -> Result<Engine, EngineError> {
        if packs & !PACK_ALL != 0 {
            return Err(EngineError::LoadFailed);
        }
        static CACHE: OnceLock<Mutex<LexCache>> = OnceLock::new();
        // The pack set is part of the key: a selected pack whose file is missing leaves the stamp as without it.
        let stamp = (packs, resource_stamp(data, personal, packs)?);
        let mut cache = CACHE
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| EngineError::LoadFailed)?;
        if let Some(index) = cache.iter().position(|(key, _, _, _)| key == &stamp) {
            let entry = cache.remove(index);
            let mut engine = Engine::with_lexicon(entry.1.clone(), layout);
            engine.data_dir = Some(data.to_owned());
            engine.pack_text = entry.2.0.clone();
            engine.pack_oov_len = entry.2.1;
            engine.digit_table = entry.3.clone();
            cache.push(entry);
            return Ok(engine);
        }
        let engine = Engine::new_with_personal_uncached(data, layout, personal, packs)?;
        WINDOWS_LEX_LOADS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if cache.len() == 4 {
            cache.remove(0);
        }
        cache.push((stamp, engine.lex.clone(), (engine.pack_text.clone(), engine.pack_oov_len), engine.digit_table.clone()));
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
            // What upstream's digit_env builds once per engine; every engine of this entry may share it.
            Option<(Arc<DigitTable>, Arc<DigitCache>)>,
        )>;
        static CACHE: OnceLock<Mutex<Models>> = OnceLock::new();
        // The class pairing and optional side file are part of the load contract,
        // even after another session already holds a valid mapped model.
        let folder = path.parent().ok_or(EngineError::LoadFailed)?;
        let mut paths = vec![path.to_owned()];
        if folder.join("classes.sjc").exists(){paths.push(folder.join("classes.sjc"));}
        for side in ["kn.sjkn", "trigram.sjt3", "trigram-chat.sjt3"].map(|name| folder.join(name)) {
            if side.exists() {
                paths.push(side);
            }
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
            .position(|(key, lex, _, _)| key == &stamp && Arc::ptr_eq(lex, &self.lex))
        {
            // What upstream's load_lm does after building the state; the prediction index was built with it.
            let entry = cache.remove(index);
            self.invalidate();
            self.lm = Some(entry.2.clone());
            self.set_tinylm(None);
            // Keyed like digit_env's own check, and only for the table it was built from.
            self.digit_cache = entry.3.as_ref().filter(|(table, _)| self.digit_table.as_ref().is_some_and(|own| Arc::ptr_eq(own, table))).map(|(_, cache)| cache.clone());
            self.en_words = None;
            self.en_cache();
            self.digit_env();
            cache.push(entry);
            return Ok(());
        }
        self.load_lm_uncached(path)?;
        WINDOWS_MODEL_LOADS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if cache.len() == 4 {
            cache.remove(0);
        }
        let digits = self.digit_table.clone().zip(self.digit_cache.clone());
        cache.push((stamp, self.lex.clone(), self.lm.as_ref().unwrap().clone(), digits));
        Ok(())
    }
    /// Whether two engines use the same digit tables (tests).
    #[doc(hidden)]
    pub fn windows_shares_digit_cache(&self, other: &Engine) -> bool {
        self.digit_cache.as_ref().zip(other.digit_cache.as_ref()).is_some_and(|(a, b)| Arc::ptr_eq(a, b))
    }
}
impl Engine{
    pub fn load_tinylm(&mut self,path:&Path)->Result<(),EngineError>{
        type CachedTiny=(std::path::PathBuf,u64,Option<std::time::SystemTime>,String,Option<[u8;32]>,Arc<TinyLm>);
        static CACHE:OnceLock<Mutex<Vec<CachedTiny>>>=OnceLock::new();
        let lm=self.lm.as_ref().ok_or(EngineError::LoadFailed)?;
        let sha=lm.lm.sha256().to_owned();let kn=lm.lm.kn_sha256();
        let meta=std::fs::metadata(path).map_err(|_|EngineError::LoadFailed)?;
        let mut cache=CACHE.get_or_init(Default::default).lock().map_err(|_|EngineError::LoadFailed)?;
        if let Some(entry)=cache.iter().find(|e|e.0==path&&e.1==meta.len()&&e.2==meta.modified().ok()&&e.3==sha&&e.4==kn){
            let tiny=entry.5.clone();self.invalidate();self.set_tinylm(Some(tiny));return Ok(());
        }
        self.load_tinylm_uncached(path)?;
        if cache.len()>=4{cache.remove(0);}
        cache.push((path.to_owned(),meta.len(),meta.modified().ok(),sha,kn,self.tinylm.as_ref().unwrap().clone()));Ok(())
    }
    /// 常談 reads the lexicon's word sets and this engine's pack rows on every load; engines built from the same
    /// data share one table.
    pub fn load_colloquial(&mut self,path:&Path)->Result<(),EngineError>{
        use std::hash::{Hash,Hasher};
        type CachedColloquial=(std::path::PathBuf,u64,Option<std::time::SystemTime>,std::path::PathBuf,u64,Arc<Colloquial>);
        static CACHE:OnceLock<Mutex<Vec<CachedColloquial>>>=OnceLock::new();
        let dir=self.data_dir.clone().ok_or(EngineError::LoadFailed)?;
        let meta=std::fs::metadata(path).map_err(|_|EngineError::LoadFailed)?;
        let mut hasher=std::collections::hash_map::DefaultHasher::new();self.pack_text.hash(&mut hasher);let packs=hasher.finish();
        let key=|e:&CachedColloquial|e.0==path&&e.1==meta.len()&&e.2==meta.modified().ok()&&e.3==dir&&e.4==packs;
        let mut cache=CACHE.get_or_init(Default::default).lock().map_err(|_|EngineError::LoadFailed)?;
        if let Some(entry)=cache.iter().find(|e|key(e)){
            let table=entry.5.clone();self.invalidate();self.colloquial=Some(table);return Ok(());
        }
        self.load_colloquial_uncached(path)?;
        if cache.len()>=4{cache.remove(0);}
        cache.push((path.to_owned(),meta.len(),meta.modified().ok(),dir,packs,self.colloquial.as_ref().unwrap().clone()));Ok(())
    }
}

/// Read-only word checks share the same immutable resources as input sessions.
/// The base lexicon excludes personal additions and LM vocabulary expansion.
pub fn windows_base_lexicon(data:&Path)->Result<Arc<Lexicon>,EngineError> {
    Ok(Engine::new_with_personal(data,Layout::Standard,None,PACK_ALL)?.lex.clone())
}
pub fn windows_model_contains(data:&Path,path:&Path,word:&str)->Result<bool,EngineError> {
    let mut engine=Engine::new_with_personal(data,Layout::Standard,None,PACK_ALL)?;
    engine.load_lm(path)?;
    Ok(engine.lm.as_ref().is_some_and(|state|state.lm.count(word)>0))
}
