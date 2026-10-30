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
    let matches = if std::env::var("SHANJIE_TEST_FAULT_OP").as_deref() == Ok("prediction") {
        matches!(op, Op::Prediction(_))
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
                    let mut states = sessions.lock().unwrap();
                    states.retain(|_, state| {
                        Arc::strong_count(state) > 1
                            || state.lock().unwrap().touched.elapsed() < Duration::from_secs(300)
                    });
                    let existing = states.get(&id).cloned();
                    let state = if let Some(state) = existing {
                        Some(state)
                    } else if states.len() >= 256 {
                        None
                    } else {
                        Engine::open_local(&data, &config)
                        .and_then(|mut engine| {
                            engine.rerank_in_background();
                            if engine.replay(&replay) {
                                let mut initial = reply(Some(&engine), None, true);
                                initial.sequence = sequence;
                                initial.found = true;
                                let state = Arc::new(Mutex::new(State {
                                    config: config.clone(),
                                    engine,
                                    sequence,
                                    cached: initial,
                                    touched: Instant::now(),
                                }));
                                states.insert(id.clone(), state.clone());
                                Some(state)
                            } else {
                                None
                            }
                        })
                    };
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
