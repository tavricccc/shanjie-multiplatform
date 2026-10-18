//! flock(2) locks on files in the profile folder, standing in for the Windows named mutexes. A lock
//! belongs to the open file description, so two threads of one process exclude each other too.
use std::{
    fs::{File, OpenOptions},
    os::fd::AsRawFd,
    path::Path,
    time::{Duration, Instant},
};

extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}
const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;
const LOCK_UN: i32 = 8;

/// The learning lock (Windows "ShanjieLearning-<folder>").
pub const LEARNING: &str = "learning.lock";
/// The profile write lock (Windows "ShanjieProfileWrite-<path>").
pub const PROFILE: &str = "profile.lock";
/// The dictionary publish lock (Windows "ShanjieDictionary-<folder>\<name>").
pub const DICTIONARY: &str = "dictionary.lock";

/// Opens (creating it if needed) a lock file in `folder`.
pub fn open(folder: &Path, name: &str) -> std::io::Result<File> {
    std::fs::create_dir_all(folder)?;
    OpenOptions::new().read(true).write(true).create(true).truncate(false).open(folder.join(name))
}

/// Waits up to `wait` for the lock.
pub fn acquire(file: &File, wait: Duration) -> bool {
    let start = Instant::now();
    loop {
        if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0 {
            return true;
        }
        if start.elapsed() >= wait {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub fn release(file: &File) {
    unsafe { flock(file.as_raw_fd(), LOCK_UN) };
}

/// Held until dropped.
pub struct Guard(File);
impl Drop for Guard {
    fn drop(&mut self) {
        release(&self.0);
    }
}

pub fn hold(folder: &Path, name: &str, wait: Duration) -> std::io::Result<Option<Guard>> {
    let file = open(folder, name)?;
    Ok(acquire(&file, wait).then_some(Guard(file)))
}
