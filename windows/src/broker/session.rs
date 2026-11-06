use super::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
pub type Sessions = Arc<Mutex<HashMap<String, Arc<Mutex<State>>>>>;
pub struct State {
    config: Config,
    engine: Engine,
    sequence: u64,
    cached: Reply,
    touched: Instant,
}
static FAULT_USED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn fault(sequence: u64, op: &Op) -> Option<String> {
    if std::env::var_os("SHANJIE_TEST_PROFILE").is_none() {
        return None;
    }
    let selector = std::env::var("SHANJIE_TEST_FAULT_OP");
    let matches = if selector.as_deref() == Ok("prediction") {
        matches!(op, Op::Prediction(_))
    } else if selector.as_deref() == Ok("rerank") {
        matches!(op, Op::Rerank)
    } else {
        sequence == std::env::var("SHANJIE_TEST_FAULT_SEQUENCE").ok()?.parse::<u64>().ok()?
    };
    if !matches || FAULT_USED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return None;
    }
    std::env::var("SHANJIE_TEST_FAULT").ok()
}
fn response(state: &State, snapshot: Option<Snapshot>, ok: bool) -> Reply {
    let mut result = reply(Some(&state.engine), snapshot, ok);
    result.sequence = state.sequence;
    result.found = true;
    result
}
impl State {
    fn apply(&mut self, sequence: u64, op: Op) -> Reply {
        self.touched = Instant::now();
        if sequence == self.sequence && sequence != 0 {
            return self.cached.clone();
        }
        if sequence != self.sequence + 1 {
            let mut r = response(self, None, false);
            r.failure = Some(Failure::Ambiguous);
            return r;
        }
        let snapshot = self.engine.perform(op);
        if snapshot.is_none() {
            let mut r = response(self, None, false);
            r.failure = self.engine.failure();
            return r;
        }
        self.sequence = sequence;
        let result = response(self, snapshot, true);
        self.cached = result.clone();
        result
    }
}
/// A detached session is kept this long for its client to resume.
const IDLE_SESSION: Duration = Duration::from_secs(300);
const MAX_SESSIONS: usize = 256;
/// The session `id`, built when there is none. Building can load the resources for seconds, so it runs outside the
/// lock: other clients keep opening and resuming. If a retry of the same client got there first, its session stands.
fn open_or_attach<T>(
    sessions: &Mutex<HashMap<String, Arc<Mutex<T>>>>,
    id: &str,
    keep_detached: impl Fn(&T) -> bool,
    build: impl FnOnce() -> Option<T>,
) -> Option<Arc<Mutex<T>>> {
    {
        let mut states = sessions.lock().unwrap();
        states.retain(|_, state| Arc::strong_count(state) > 1 || keep_detached(&state.lock().unwrap()));
        if let Some(state) = states.get(id) {
            return Some(state.clone());
        }
        if states.len() >= MAX_SESSIONS {
            return None;
        }
    }
    let built = Arc::new(Mutex::new(build()?));
    let mut states = sessions.lock().unwrap();
    if let Some(state) = states.get(id) {
        return Some(state.clone());
    }
    if states.len() >= MAX_SESSIONS {
        return None;
    }
    states.insert(id.to_owned(), built.clone());
    Some(built)
}
pub fn run(mut pipe: File, data: PathBuf, sessions: Sessions) {
    let mut attached: Option<(String, Arc<Mutex<State>>)> = None;
    while let Ok(request) = read::<Request>(&mut pipe) {
        let injection = if let Request::Apply { sequence, op } = &request {
            fault(*sequence, op)
        } else {
            None
        };
        if injection.as_deref() == Some("before-apply") {
            std::thread::sleep(Duration::from_millis(1200));
        }
        let result = match request {
            Request::Open {
                id,
                config,
                replay,
                sequence,
            } => {
                if id.len() > 128
                    || Path::new(&config.dictionary).components().count() != 1
                    || !config.dictionary.ends_with(".tsv")
                {
                    reply(None, None, false)
                } else {
                    let state = open_or_attach(&sessions, &id, |state| state.touched.elapsed() < IDLE_SESSION, || {
                        let mut engine = Engine::open_local(&data, &config)?;
                        engine.rerank_in_background();
                        if !engine.replay(&replay) {
                            return None;
                        }
                        let mut initial = reply(Some(&engine), None, true);
                        initial.sequence = sequence;
                        initial.found = true;
                        Some(State {
                            config: config.clone(),
                            engine,
                            sequence,
                            cached: initial,
                            touched: Instant::now(),
                        })
                    });
                    if let Some(state) = state {
                        let result = {
                            let state = state.lock().unwrap();
                            if state.config == config {
                                state.cached.clone()
                            } else {
                                reply(None, None, false)
                            }
                        };
                        if result.ok {
                            attached = Some((id, state));
                        }
                        result
                    } else {
                        let mut r = reply(None, None, false);
                        r.failure = Some(Failure::Busy);
                        r
                    }
                }
            }
            Request::Resume(id) => {
                let found = sessions.lock().unwrap().get(&id).cloned();
                if let Some(state) = found {
                    let result = state.lock().unwrap().cached.clone();
                    attached = Some((id, state));
                    result
                } else {
                    reply(None, None, false)
                }
            }
            Request::Apply { sequence, op } => {
                if let Some((_, state)) = attached.as_ref() {
                    let mut state=state.lock().unwrap();
                    if injection.as_deref()==Some("hold-state") {std::thread::sleep(Duration::from_millis(6500));}
                    state.apply(sequence, op)
                } else {
                    reply(None, None, false)
                }
            }
            Request::Close => {
                if let Some((id, _)) = attached.take() {
                    sessions.lock().unwrap().remove(&id);
                }
                break;
            }
            Request::Status => reply(None, None, true),
            Request::RerankReady => {
                let mut r = reply(None, None, attached.is_some());
                r.rerank_ready = attached.as_ref().is_some_and(|(_, state)| state.lock().unwrap().engine.rerank_ready());
                r
            }
            Request::Profile => {
                let mut r = reply(None, None, true);
                r.profile = std::fs::read_to_string(crate::profile::path()).ok();
                r
            }
            Request::Word { word, reading } => {
                let result = if let Some(reading) = reading {
                    crate::word_store::add(&word, &reading).map(|_| String::new())
                } else { crate::word_store::suggest(&data, &word) };
                let mut r = reply(None, None, result.is_ok());
                r.word_result = Some(result.map_err(|e| e.to_string())); r
            }
            Request::InspectWord { word, reading } => {
                let result=crate::word_store::inspect(&data,&word,reading.as_deref())
                    .and_then(|info|Ok(serde_json::to_string(&info)?));
                let mut r=reply(None,None,result.is_ok());r.word_result=Some(result.map_err(|e|e.to_string()));r
            }
            Request::Stop => {
                let _ = write(&mut pipe, &reply(None, None, true));
                std::process::exit(0);
            }
        };
        if injection.as_deref() == Some("drop-reply") {
            break;
        }
        if injection.as_deref() == Some("after-apply") {
            std::thread::sleep(Duration::from_millis(1200));
        }
        if write(&mut pipe, &result).is_err() {
            break;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    type Table = Mutex<HashMap<String, Arc<Mutex<u32>>>>;
    #[test]
    fn building_a_session_leaves_the_table_free() {
        let table = Table::default();
        let state = open_or_attach(&table, "a", |_| true, || {
            assert!(table.try_lock().is_ok(), "other clients can open and resume during a load");
            Some(1)
        });
        assert_eq!(*state.unwrap().lock().unwrap(), 1);
        assert!(table.lock().unwrap().contains_key("a"));
    }
    #[test]
    fn an_existing_session_is_attached_and_not_rebuilt() {
        let table = Table::default();
        let first = open_or_attach(&table, "a", |_| true, || Some(1)).unwrap();
        let again = open_or_attach(&table, "a", |_| true, || panic!("must not rebuild")).unwrap();
        assert!(Arc::ptr_eq(&first, &again));
    }
    #[test]
    fn a_session_opened_during_the_build_wins() {
        let table = Table::default();
        let state = open_or_attach(&table, "a", |_| true, || {
            table.lock().unwrap().insert("a".into(), Arc::new(Mutex::new(1)));
            Some(2)
        });
        assert_eq!(*state.unwrap().lock().unwrap(), 1);
    }
    #[test]
    fn a_failed_build_leaves_no_session() {
        let table = Table::default();
        assert!(open_or_attach(&table, "a", |_| true, || None).is_none());
        assert!(table.lock().unwrap().is_empty());
    }
    #[test]
    fn the_session_limit_holds_before_and_after_the_build() {
        let table = Table::default();
        for index in 0..MAX_SESSIONS {
            let state = open_or_attach(&table, &index.to_string(), |_| true, || Some(0));
            std::mem::forget(state);
        }
        assert!(open_or_attach(&table, "over", |_| true, || panic!("must not load past the limit")).is_none());
        table.lock().unwrap().remove("0");
        let state = open_or_attach(&table, "over", |_| true, || {
            table.lock().unwrap().insert("0".into(), Arc::new(Mutex::new(0)));
            Some(1)
        });
        assert!(state.is_none());
    }
    #[test]
    fn only_expired_detached_sessions_are_dropped() {
        let table = Table::default();
        let attached = open_or_attach(&table, "attached", |_| true, || Some(1)).unwrap();
        drop(open_or_attach(&table, "detached", |_| true, || Some(1)));
        drop(open_or_attach(&table, "next", |_| false, || Some(1)));
        let states = table.lock().unwrap();
        assert!(states.contains_key("attached") && states.contains_key("next") && !states.contains_key("detached"));
        drop(attached);
    }
}
