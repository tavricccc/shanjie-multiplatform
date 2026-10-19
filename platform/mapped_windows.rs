//! `trigram::Mapped` on Windows: the same read-only view of a whole file, through the mapping the model arrays use
//! (it denies write/delete sharing while mapped, so the file cannot change underneath it).
use std::{ops::Deref, path::Path, sync::Arc};

pub(crate) struct Mapped(Arc<crate::mapped_array::Mapping>);

impl Mapped {
    /// # Safety
    /// Same contract as the Unix mapping; Windows additionally refuses writers while the view exists.
    pub(crate) unsafe fn open(path: &Path) -> Result<Mapped, ()> {
        crate::mapped_array::Mapping::open(path).map(Mapped).map_err(|_| ())
    }
}

impl Deref for Mapped {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.0.bytes()
    }
}
