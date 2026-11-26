//! Test-only, numeric-only timeline recorder and worker priority switch for rerank scheduling experiments.
//! Compiled only when the `rerank-timing` Cargo feature is enabled; never records text, readings, candidates, or context.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};
use windows::Win32::{
    Foundation::FILETIME,
    System::Threading::{
        GetCurrentProcess, GetCurrentThread, GetCurrentThreadId, GetPriorityClass,
        GetProcessAffinityMask, GetThreadPriority, GetThreadTimes, THREAD_PRIORITY_ABOVE_NORMAL,
        THREAD_PRIORITY_BELOW_NORMAL, THREAD_PRIORITY_NORMAL,
    },
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn QueryPerformanceCounter(lp_performance_count: *mut i64) -> i32;
    fn QueryPerformanceFrequency(lp_frequency: *mut i64) -> i32;
}

pub const ROLE_CORE: u8 = 1;
pub const ROLE_TIP: u8 = 2;

// Event codes aligned with upstream Windows events 72..76 plus full lifecycle timeline events.
pub const EV_KEY_START: u32 = 1;
pub const EV_KEY_END: u32 = 2;
pub const EV_JOB_CREATED: u32 = 10;
pub const EV_JOB_ENQUEUED: u32 = 11;
pub const EV_WORKER_START: u32 = 12;
pub const EV_RESULT_READY: u32 = 13;
pub const EV_POLL_START: u32 = 14;
pub const EV_RERANK_FETCH_START: u32 = 15;
pub const EV_RERANK_FETCH_END: u32 = 16;
pub const EV_RESULT_DROPPED: u32 = 17;
pub const EV_ENTER_WAIT_START: u32 = 18;
pub const EV_ENTER_WAIT_END: u32 = 19;
pub const EV_ENTER_TIMEOUT: u32 = 20;
pub const EV_ENGINE_RESET: u32 = 21;
pub const EV_POLL_END: u32 = 72;
pub const EV_RESULT_APPLIED: u32 = 73;
pub const EV_KEY_PENDING: u32 = 74;
pub const EV_WORKER_FINISH: u32 = 75;
pub const EV_WORKER_INIT: u32 = 76;
pub const EV_WORKER_AFFINITY: u32 = 77;

// Drop reason codes for EV_RESULT_DROPPED (numeric only).
pub const DROP_SUPERSEDED_WAITING: u16 = 1;
pub const DROP_STALE_AFTER_RUN: u16 = 2;
pub const DROP_DISCARDED_RESET: u16 = 3;
pub const DROP_SESSION_MISMATCH: u16 = 4;
pub const DROP_GIVE_UP_TIMEOUT: u16 = 5;
pub const DROP_RUN_ERROR: u16 = 6;
pub const DROP_APPLY_NO_CHANGE: u16 = 7;
pub const DROP_EDIT_SESSION_REFUSED: u16 = 8;
pub const DROP_PRIVACY_PAUSED: u16 = 9;

// Poll result codes for EV_POLL_END (72).
pub const POLL_READY: u16 = 0;
pub const POLL_NOT_READY: u16 = 1;
pub const POLL_WINDOW_OPEN: u16 = 2;
pub const POLL_TIMEOUT: u16 = 3;
pub const POLL_IO_ERROR: u16 = 4;
pub const POLL_GIVE_UP_2S: u16 = 5;

// Apply source codes for EV_RESULT_APPLIED (73).
pub const APPLY_TSF_POLL_CHANGED: u16 = 0;
pub const APPLY_ENTER_WAIT: u16 = 1;
pub const APPLY_TSF_POLL_UNCHANGED: u16 = 2;
pub const APPLY_CORE_OP_RERANK: u16 = 3;

// Priority override codes for EV_WORKER_INIT (76).
pub const PRIO_REQ_DEFAULT: u16 = 0;
pub const PRIO_REQ_BELOW_NORMAL: u16 = 1;
pub const PRIO_REQ_NORMAL: u16 = 2;
pub const PRIO_REQ_ABOVE_NORMAL: u16 = 3;
pub const PRIO_REQ_REJECTED: u16 = 99;

const BUFFER_CAPACITY: usize = 65_536;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct TimingRecord {
    pub qpc: i64,
    pub qpc_end: i64,
    pub kind: u32,
    pub role: u8,
    pub flags: u8,
    pub reason: u16,
    pub pid: u32,
    pub tid: u32,
    pub seq: u64,
    pub job_id: u64,
    pub a: u64,
    pub b: u64,
}

struct Recorder {
    dir: PathBuf,
    records: Mutex<Vec<TimingRecord>>,
}

static RECORDER: OnceLock<Option<Recorder>> = OnceLock::new();
static QPC_FREQ: OnceLock<i64> = OnceLock::new();
static DROPPED: AtomicU64 = AtomicU64::new(0);
static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_TIP_KEY_SEQ: AtomicU64 = AtomicU64::new(1);
static NEXT_CORE_KEY_SEQ: AtomicU64 = AtomicU64::new(1);
static NEXT_POLL_SEQ: AtomicU64 = AtomicU64::new(1);
static TIP_APPLIED_COUNT: AtomicU64 = AtomicU64::new(0);
static TIP_LAST_APPLIED_JOB: AtomicU64 = AtomicU64::new(0);

pub fn qpc_freq() -> i64 {
    *QPC_FREQ.get_or_init(|| {
        let mut f = 0i64;
        unsafe {
            QueryPerformanceFrequency(&mut f);
        }
        f.max(1)
    })
}

pub fn qpc_now() -> i64 {
    let mut v = 0i64;
    unsafe {
        QueryPerformanceCounter(&mut v);
    }
    v
}

pub fn qpc_delta_us(start: i64, end: i64) -> u64 {
    let delta = end.saturating_sub(start).max(0) as u128;
    let freq = qpc_freq() as u128;
    ((delta * 1_000_000) / freq) as u64
}

fn recorder() -> Option<&'static Recorder> {
    RECORDER
        .get_or_init(|| {
            if std::env::var("SHANJIE_TEST_PROFILE").as_deref() != Ok("1") {
                return None;
            }
            let dir = std::env::var_os("SHANJIE_RERANK_TIMING_DIR")?;
            let dir = PathBuf::from(dir);
            if dir.as_os_str().is_empty() {
                return None;
            }
            let _ = qpc_freq();
            Some(Recorder {
                dir,
                records: Mutex::new(Vec::with_capacity(BUFFER_CAPACITY)),
            })
        })
        .as_ref()
}

pub fn next_job_id() -> u64 {
    NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn next_tip_key_seq() -> u64 {
    NEXT_TIP_KEY_SEQ.fetch_add(1, Ordering::Relaxed)
}

pub fn next_core_key_seq() -> u64 {
    NEXT_CORE_KEY_SEQ.fetch_add(1, Ordering::Relaxed)
}

pub fn next_poll_seq() -> u64 {
    NEXT_POLL_SEQ.fetch_add(1, Ordering::Relaxed)
}

pub fn mark_tip_applied(job_id: u64) {
    TIP_LAST_APPLIED_JOB.store(job_id, Ordering::Release);
    TIP_APPLIED_COUNT.fetch_add(1, Ordering::Release);
}

#[no_mangle]
pub extern "C" fn sj_rerank_timing_applied_count() -> u64 {
    TIP_APPLIED_COUNT.load(Ordering::Acquire)
}

#[no_mangle]
pub extern "C" fn sj_rerank_timing_last_applied_job() -> u64 {
    TIP_LAST_APPLIED_JOB.load(Ordering::Acquire)
}

/// Parse the isolated test-only priority override. Never accepts TIME_CRITICAL or unknown values.
pub fn configured_worker_priority(default_priority: i32) -> (i32, u16) {
    if std::env::var("SHANJIE_TEST_PROFILE").as_deref() != Ok("1") {
        return (default_priority, PRIO_REQ_DEFAULT);
    }
    let Ok(raw) = std::env::var("SHANJIE_TEST_RERANK_PRIORITY") else {
        return (default_priority, PRIO_REQ_DEFAULT);
    };
    parse_worker_priority(&raw, default_priority)
}

fn parse_worker_priority(raw: &str, default_priority: i32) -> (i32, u16) {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "default" => (default_priority, PRIO_REQ_DEFAULT),
        "below_normal" => (THREAD_PRIORITY_BELOW_NORMAL.0, PRIO_REQ_BELOW_NORMAL),
        "normal" => (THREAD_PRIORITY_NORMAL.0, PRIO_REQ_NORMAL),
        "above_normal" => (THREAD_PRIORITY_ABOVE_NORMAL.0, PRIO_REQ_ABOVE_NORMAL),
        _ => (default_priority, PRIO_REQ_REJECTED),
    }
}

fn filetime_to_100ns(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

/// Returns (user_100ns, kernel_100ns) of the current thread.
pub fn thread_cpu_times_100ns() -> (u64, u64) {
    let (mut create, mut exit, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    unsafe {
        if GetThreadTimes(
            GetCurrentThread(),
            &mut create,
            &mut exit,
            &mut kernel,
            &mut user,
        )
        .is_ok()
        {
            return (filetime_to_100ns(user), filetime_to_100ns(kernel));
        }
    }
    (0, 0)
}

pub fn capture_worker_init(requested_code: u16, set_ok: bool) {
    unsafe {
        let actual_prio = GetThreadPriority(GetCurrentThread());
        let prio_class = GetPriorityClass(GetCurrentProcess());
        let mut proc_mask: usize = 0;
        let mut sys_mask: usize = 0;
        let aff_ok =
            GetProcessAffinityMask(GetCurrentProcess(), &mut proc_mask, &mut sys_mask).is_ok();
        record(
            ROLE_CORE,
            EV_WORKER_INIT,
            0,
            0,
            actual_prio as i64 as u64,
            prio_class as u64,
            set_ok as u8,
            requested_code,
        );
        record(
            ROLE_CORE,
            EV_WORKER_AFFINITY,
            0,
            0,
            proc_mask as u64,
            sys_mask as u64,
            aff_ok as u8,
            0,
        );
    }
}

/// Lock-avoiding, allocation-free hot-path record into the preallocated buffer.
#[allow(clippy::too_many_arguments)]
pub fn record_span(
    role: u8,
    kind: u32,
    qpc: i64,
    qpc_end: i64,
    seq: u64,
    job_id: u64,
    a: u64,
    b: u64,
    flags: u8,
    reason: u16,
) {
    let Some(rec) = recorder() else {
        return;
    };
    let Ok(mut guard) = rec.records.try_lock() else {
        DROPPED.fetch_add(1, Ordering::Relaxed);
        return;
    };
    if guard.len() >= BUFFER_CAPACITY {
        DROPPED.fetch_add(1, Ordering::Relaxed);
        return;
    }
    guard.push(TimingRecord {
        qpc,
        qpc_end,
        kind,
        role,
        flags,
        reason,
        pid: std::process::id(),
        tid: unsafe { GetCurrentThreadId() },
        seq,
        job_id,
        a,
        b,
    });
}

#[allow(clippy::too_many_arguments)]
pub fn record(
    role: u8,
    kind: u32,
    seq: u64,
    job_id: u64,
    a: u64,
    b: u64,
    flags: u8,
    reason: u16,
) {
    record_span(role, kind, qpc_now(), 0, seq, job_id, a, b, flags, reason);
}

pub fn event_name(kind: u32) -> &'static str {
    match kind {
        EV_KEY_START => "key_start",
        EV_KEY_END => "key_end",
        EV_JOB_CREATED => "job_created",
        EV_JOB_ENQUEUED => "job_enqueued",
        EV_WORKER_START => "worker_start",
        EV_RESULT_READY => "result_ready",
        EV_POLL_START => "poll_start",
        EV_RERANK_FETCH_START => "rerank_fetch_start",
        EV_RERANK_FETCH_END => "rerank_fetch_end",
        EV_RESULT_DROPPED => "result_dropped",
        EV_ENTER_WAIT_START => "enter_wait_start",
        EV_ENTER_WAIT_END => "enter_wait_end",
        EV_ENTER_TIMEOUT => "enter_timeout",
        EV_ENGINE_RESET => "engine_reset",
        EV_POLL_END => "poll_end",
        EV_RESULT_APPLIED => "result_applied",
        EV_KEY_PENDING => "key_pending",
        EV_WORKER_FINISH => "worker_finish",
        EV_WORKER_INIT => "worker_init",
        EV_WORKER_AFFINITY => "worker_affinity",
        _ => "unknown",
    }
}

fn format_record_jsonl(r: &TimingRecord, freq: i64) -> String {
    let role_str = if r.role == ROLE_CORE { "core" } else { "tip" };
    format!(
        "{{\"event\":\"{}\",\"code\":{},\"role\":\"{}\",\"qpc\":{},\"qpc_end\":{},\"qpc_freq\":{},\"pid\":{},\"tid\":{},\"seq\":{},\"job_id\":{},\"a\":{},\"b\":{},\"flags\":{},\"reason\":{}}}\n",
        event_name(r.kind),
        r.kind,
        role_str,
        r.qpc,
        r.qpc_end,
        freq,
        r.pid,
        r.tid,
        r.seq,
        r.job_id,
        r.a,
        r.b,
        r.flags,
        r.reason
    )
}

pub fn flush(role_tag: &str) {
    let Some(rec) = recorder() else {
        return;
    };
    let drained = {
        let mut guard = rec.records.lock().unwrap();
        if guard.is_empty() {
            return;
        }
        let mut out = Vec::with_capacity(guard.len());
        out.append(&mut *guard);
        out
    };
    let freq = qpc_freq();
    let dropped = DROPPED.load(Ordering::Relaxed);
    if fs::create_dir_all(&rec.dir).is_err() {
        return;
    }
    let path = rec
        .dir
        .join(format!("events-{}-{}.jsonl", role_tag, std::process::id()));
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let mut buf = String::with_capacity(drained.len() * 140 + 128);
    for r in &drained {
        buf.push_str(&format_record_jsonl(r, freq));
    }
    buf.push_str(&format!(
        "{{\"event\":\"flush_summary\",\"code\":8,\"role\":\"{}\",\"qpc\":{},\"qpc_end\":0,\"qpc_freq\":{},\"pid\":{},\"tid\":{},\"seq\":0,\"job_id\":0,\"a\":{},\"b\":{},\"flags\":0,\"reason\":0}}\n",
        role_tag,
        qpc_now(),
        freq,
        std::process::id(),
        unsafe { GetCurrentThreadId() },
        dropped,
        drained.len()
    ));
    let _ = file.write_all(buf.as_bytes());
    let _ = file.flush();
}

#[no_mangle]
pub extern "C" fn sj_rerank_timing_flush() {
    flush("tip");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_override_accepts_only_safe_allowlist() {
        assert_eq!(parse_worker_priority("", -1), (-1, PRIO_REQ_DEFAULT));
        assert_eq!(parse_worker_priority("default", -1), (-1, PRIO_REQ_DEFAULT));
        assert_eq!(parse_worker_priority("", 0), (0, PRIO_REQ_DEFAULT));
        assert_eq!(parse_worker_priority("default", 0), (0, PRIO_REQ_DEFAULT));
        assert_eq!(parse_worker_priority("below_normal", 0), (-1, PRIO_REQ_BELOW_NORMAL));
        assert_eq!(parse_worker_priority("normal", -1), (0, PRIO_REQ_NORMAL));
        assert_eq!(parse_worker_priority("above_normal", -1), (1, PRIO_REQ_ABOVE_NORMAL));
        assert_eq!(parse_worker_priority("time_critical", -1), (-1, PRIO_REQ_REJECTED));
        assert_eq!(parse_worker_priority("highest", -1), (-1, PRIO_REQ_REJECTED));
        assert_eq!(parse_worker_priority("realtime", 0), (0, PRIO_REQ_REJECTED));
    }

    #[test]
    fn timing_record_jsonl_contains_only_fixed_schema_and_numbers() {
        let rec = TimingRecord {
            qpc: 123456,
            qpc_end: 124456,
            kind: EV_WORKER_FINISH,
            role: ROLE_CORE,
            flags: 1,
            reason: 0,
            pid: 42,
            tid: 99,
            seq: 7,
            job_id: 11,
            a: 500,
            b: 25,
        };
        let line = format_record_jsonl(&rec, 10_000_000);
        let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(parsed["event"], "worker_finish");
        assert_eq!(parsed["code"], EV_WORKER_FINISH);
        assert_eq!(parsed["role"], "core");
        assert_eq!(parsed["job_id"], 11);
        assert_eq!(parsed["a"], 500);
    }
}
