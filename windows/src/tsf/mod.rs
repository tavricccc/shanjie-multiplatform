mod candidates;
mod keys;
mod mode;
mod recovery;
mod resident;
mod service;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    OnceLock,
};
use windows::{
    core::*,
    Win32::{
        Foundation::*,
        System::{
            Com::*,
            LibraryLoader::{
                GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            },
        },
    },
};
static MODULE: OnceLock<isize> = OnceLock::new();
static OBJECTS: AtomicU32 = AtomicU32::new(0);
#[no_mangle]
pub extern "C" fn sj_tsf_objects() -> u32 {
    OBJECTS.load(Ordering::Relaxed)
}
pub struct Count;
impl Count {
    pub fn new() -> Self {
        OBJECTS.fetch_add(1, Ordering::Relaxed);
        Self
    }
}
impl Drop for Count {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::Relaxed);
    }
}
pub fn module() -> HMODULE {
    HMODULE(*MODULE.get_or_init(|| unsafe {
        let mut module = HMODULE::default();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(DllGetClassObject as *const u16),
            &mut module,
        )
        .unwrap();
        module.0 as isize
    }) as _)
}
/// Failures only, never input: sandboxed hosts give no other way to see TSF errors,
/// and their package temp folder is writable.
pub fn trace(message: impl std::fmt::Display) {
    if !crate::broker::app_container() {
        return;
    }
    use std::io::Write;
    let path = std::env::temp_dir().join("shanjie-tsf.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {} {message}", std::process::id(), std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis()));
    }
}
/// Version folder that holds this DLL's architecture folder, data and tools.
pub fn binary() -> Option<std::path::PathBuf> {
    let mut path = [0u16; 32768];
    let length =
        unsafe { windows::Win32::System::LibraryLoader::GetModuleFileNameW(Some(module()), &mut path) };
    if length == 0 {
        return None;
    }
    let file = std::path::PathBuf::from(String::from_utf16(&path[..length as usize]).ok()?);
    Some(file.parent()?.parent()?.to_owned())
}
#[implement(IClassFactory)]
struct Factory {
    _count: Count,
}
impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        out: *mut *mut std::ffi::c_void,
    ) -> Result<()> {
        unsafe {
            *out = std::ptr::null_mut();
            if outer.as_ref().is_some() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            let service = service::new();
            service.query(iid, out).ok()
        }
    }
    fn LockServer(&self, lock: BOOL) -> Result<()> {
        if lock.as_bool() {
            OBJECTS.fetch_add(1, Ordering::Relaxed);
        } else {
            OBJECTS.fetch_sub(1, Ordering::Relaxed);
        }
        Ok(())
    }
}
#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECTS.load(Ordering::Relaxed) == 0 && !resident::started() {
        S_OK
    } else {
        S_FALSE
    }
}
#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    id: *const GUID,
    iid: *const GUID,
    out: *mut *mut std::ffi::c_void,
) -> HRESULT {
    if *id != crate::registration::SERVICE {
        *out = std::ptr::null_mut();
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = Factory {
        _count: Count::new(),
    }
    .into();
    factory.query(iid, out)
}
#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    match crate::registration::register(module(), false) {
        Ok(()) => S_OK,
        Err(e) => {
            let _ = crate::registration::register(module(), true);
            e.code()
        }
    }
}
#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    match crate::registration::register(module(), true) {
        Ok(()) => S_OK,
        Err(e) => e.code(),
    }
}
