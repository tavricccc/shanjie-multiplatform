//! Read-only, aligned Windows model arrays. The mapping denies write/delete sharing
//! for its entire lifetime; packaging never modifies an installed model in place.
use std::{fs::{File, OpenOptions}, io::{self, Write}, marker::PhantomData, ops::Deref, os::windows::{fs::OpenOptionsExt, io::AsRawHandle}, path::Path, sync::Arc};
type Handle = *mut std::ffi::c_void;
#[link(name = "kernel32")]
extern "system" {
    fn CreateFileMappingW(file: Handle, attributes: Handle, protection: u32, high: u32, low: u32, name: *const u16) -> Handle;
    fn MapViewOfFile(mapping: Handle, access: u32, high: u32, low: u32, bytes: usize) -> Handle;
    fn UnmapViewOfFile(address: Handle) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
}
pub(crate) struct Mapping { _file: File, handle: Handle, data: Handle, len: usize }
// All views are read-only and the backing file cannot be opened for mutation.
unsafe impl Send for Mapping {}
unsafe impl Sync for Mapping {}
impl Drop for Mapping { fn drop(&mut self) { unsafe { UnmapViewOfFile(self.data); CloseHandle(self.handle); } } }
impl Mapping {
    pub fn open(path: &Path) -> io::Result<Arc<Self>> {
        let file = OpenOptions::new().read(true).share_mode(1).open(path)?;
        let len = usize::try_from(file.metadata()?.len()).map_err(|_| io::ErrorKind::InvalidData)?;
        if len == 0 { return Err(io::ErrorKind::InvalidData.into()); }
        unsafe {
            let handle = CreateFileMappingW(file.as_raw_handle(),std::ptr::null_mut(),2,0,0,std::ptr::null());
            if handle.is_null() { return Err(io::Error::last_os_error()); }
            let data = MapViewOfFile(handle,4,0,0,0);
            if data.is_null() { CloseHandle(handle); return Err(io::Error::last_os_error()); }
            Ok(Arc::new(Self { _file: file, handle, data, len }))
        }
    }
    pub fn bytes(&self) -> &[u8] { unsafe { std::slice::from_raw_parts(self.data.cast(),self.len) } }
}
// Closed set of types with every bit pattern valid, no pointers and no padding.
pub(crate) trait Scalar: Copy {}
impl Scalar for u8 {} impl Scalar for u32 {} impl Scalar for u64 {} impl Scalar for f64 {}
pub(crate) enum Array<T: Scalar> {
    Owned(Vec<T>),
    Mapped { owner: Arc<Mapping>, offset: usize, len: usize, marker: PhantomData<T> },
}
impl<T: Scalar> From<Vec<T>> for Array<T> { fn from(value: Vec<T>) -> Self { Self::Owned(value) } }
impl<T: Scalar> Deref for Array<T> {
    type Target = [T];
    fn deref(&self) -> &[T] { match self {
        Self::Owned(value) => value,
        Self::Mapped { owner, offset, len, .. } => unsafe { std::slice::from_raw_parts(owner.bytes().as_ptr().add(*offset).cast(),*len) },
    }}
}
pub(crate) fn read<T: Scalar>(owner: &Arc<Mapping>, cursor: &mut usize) -> io::Result<Array<T>> {
    let bytes = owner.bytes();
    let count_end = cursor.checked_add(8).ok_or(io::ErrorKind::InvalidData)?;
    let count = u64::from_le_bytes(bytes.get(*cursor..count_end).ok_or(io::ErrorKind::InvalidData)?.try_into().unwrap());
    let len = usize::try_from(count).map_err(|_| io::ErrorKind::InvalidData)?;
    let offset = count_end;
    let end = offset.checked_add(len.checked_mul(std::mem::size_of::<T>()).ok_or(io::ErrorKind::InvalidData)?).ok_or(io::ErrorKind::InvalidData)?;
    if end > bytes.len() || offset % std::mem::align_of::<T>() != 0 { return Err(io::ErrorKind::InvalidData.into()); }
    *cursor = end.checked_add(7).ok_or(io::ErrorKind::InvalidData)? & !7;
    Ok(Array::Mapped { owner: owner.clone(),offset,len,marker: PhantomData })
}
pub(crate) fn write<T: Scalar>(file: &mut File, values: &[T]) -> io::Result<()> {
    file.write_all(&(values.len() as u64).to_le_bytes())?;
    let bytes = unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(),std::mem::size_of_val(values)) };
    file.write_all(bytes)?;
    let padding = (8 - bytes.len() % 8) % 8;
    file.write_all(&[0; 8][..padding])
}
