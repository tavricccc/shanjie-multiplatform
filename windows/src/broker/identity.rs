//! File identity uses the NT namespace, which does not require access to a DOS
//! device mapping in AppContainer hosts. Never use this path to launch a process.
use super::*;
use windows::Win32::Storage::FileSystem::{GetFinalPathNameByHandleW, VOLUME_NAME_NT};
use windows::Win32::Foundation::HANDLE;

pub(super) fn file_path(file: &File) -> io::Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        GetFinalPathNameByHandleW(HANDLE(file.as_raw_handle()), &mut buffer, VOLUME_NAME_NT)
    } as usize;
    if length == 0 { return Err(io::Error::last_os_error()); }
    if length >= buffer.len() { return Err(io::Error::other("Core identity path too long")); }
    Ok(PathBuf::from(String::from_utf16_lossy(&buffer[..length]).to_lowercase()))
}
pub(super) fn path(path: &Path) -> io::Result<PathBuf> {
    file_path(&File::open(path)?)
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn identity_does_not_use_dos_namespace() {
        let path = std::env::current_exe().unwrap();
        let native = self::path(&path).unwrap();
        assert!(native.to_string_lossy().starts_with(r"\device\"));
        assert_eq!(native, self::path(&path.canonicalize().unwrap()).unwrap());
    }
}
