//! Deadline-based pipe I/O. Cancellation completes before buffers are released.
use std::{ffi::c_void, fs::File, io, os::windows::io::AsRawHandle, time::Instant};
use windows::Win32::System::IO::OVERLAPPED;
type Raw = *mut c_void;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(attributes: Raw, manual: i32, initial: i32, name: *const u16) -> Raw;
    fn CloseHandle(handle: Raw) -> i32;
    fn WaitForSingleObject(handle: Raw, milliseconds: u32) -> u32;
    fn ReadFile(file: Raw, bytes: Raw, size: u32, count: *mut u32, overlap: *mut OVERLAPPED)
        -> i32;
    fn WriteFile(
        file: Raw,
        bytes: *const c_void,
        size: u32,
        count: *mut u32,
        overlap: *mut OVERLAPPED,
    ) -> i32;
    fn GetOverlappedResult(file: Raw, overlap: *mut OVERLAPPED, count: *mut u32, wait: i32) -> i32;
    fn CancelIoEx(file: Raw, overlap: *mut OVERLAPPED) -> i32;
}
struct Event(Raw);
impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub struct Pipe(pub File);
pub struct SendError {
    pub error: io::Error,
    pub possible: bool,
}
impl Pipe {
    fn transfer(&self, bytes: &mut [u8], writing: bool, deadline: Instant) -> io::Result<usize> {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Core request deadline exceeded",
            ));
        }
        unsafe {
            let event = Event(CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()));
            if event.0.is_null() {
                return Err(io::Error::last_os_error());
            }
            let mut overlap = OVERLAPPED::default();
            overlap.hEvent = windows::Win32::Foundation::HANDLE(event.0);
            let file = self.0.as_raw_handle();
            let mut count = 0;
            let ok = if writing {
                WriteFile(
                    file,
                    bytes.as_ptr().cast(),
                    bytes.len() as u32,
                    &mut count,
                    &mut overlap,
                )
            } else {
                ReadFile(
                    file,
                    bytes.as_mut_ptr().cast(),
                    bytes.len() as u32,
                    &mut count,
                    &mut overlap,
                )
            };
            if ok != 0 {
                return Ok(count as usize);
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(997) {
                return Err(error);
            }
            let remaining = deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(u32::MAX as u128 - 1) as u32;
            let wait = WaitForSingleObject(event.0, remaining);
            if wait != 0 {
                CancelIoEx(file, &mut overlap);
                // A pending OVERLAPPED and its buffer must outlive cancellation.
                GetOverlappedResult(file, &mut overlap, &mut count, 1);
                return Err(if wait == 258 {
                    io::Error::new(io::ErrorKind::TimedOut, "Core request deadline exceeded")
                } else {
                    io::Error::last_os_error()
                });
            }
            if GetOverlappedResult(file, &mut overlap, &mut count, 0) == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(count as usize)
        }
    }
    pub fn read(&self, bytes: &mut [u8], deadline: Instant) -> io::Result<()> {
        let mut at = 0;
        while at < bytes.len() {
            let count = self.transfer(&mut bytes[at..], false, deadline)?;
            if count == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            at += count;
        }
        Ok(())
    }
    pub fn send<T: serde::Serialize>(&self, value: &T, deadline: Instant) -> io::Result<()> {
        self.send_tracked(value, deadline).map_err(|e| e.error)
    }
    pub fn send_tracked<T: serde::Serialize>(
        &self,
        value: &T,
        deadline: Instant,
    ) -> Result<(), SendError> {
        let bytes = serde_json::to_vec(value).map_err(|e| SendError {
            error: e.into(),
            possible: false,
        })?;
        if bytes.len() > 1024 * 1024 {
            return Err(SendError {
                error: io::Error::other("Core message too large"),
                possible: false,
            });
        }
        let mut frame = (bytes.len() as u32).to_le_bytes().to_vec();
        frame.extend(bytes);
        let mut at = 0;
        while at < frame.len() {
            let count = self
                .transfer(&mut frame[at..], true, deadline)
                .map_err(|error| SendError {
                    possible: at != 0 || error.kind() == io::ErrorKind::TimedOut,
                    error,
                })?;
            if count == 0 {
                return Err(SendError {
                    error: io::ErrorKind::WriteZero.into(),
                    possible: at != 0,
                });
            }
            at += count;
        }
        Ok(())
    }
    pub fn receive<T: serde::de::DeserializeOwned>(&self, deadline: Instant) -> io::Result<T> {
        let mut size = [0; 4];
        self.read(&mut size, deadline)?;
        let size = u32::from_le_bytes(size) as usize;
        if size > 1024 * 1024 {
            return Err(io::Error::other("Core message too large"));
        }
        let mut bytes = vec![0; size];
        self.read(&mut bytes, deadline)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}
