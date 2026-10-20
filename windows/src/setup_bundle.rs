//! Single-file setup: 安裝善解.exe with the whole package appended. The package files are
//! written as one stream of `path, size, bytes` records, cut into chunks compressed with the
//! Windows LZMS compressor (cabinet.dll, no extra runtime), and closed by a fixed trailer.
//! The setup window (ui\ and the manager in tools\) comes first, so it can open while the
//! models and data are still being written. The extracted package is then verified again by
//! shanjie-manager against SHA256SUMS.txt.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};
use windows::Win32::Storage::Compression::*;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const MAGIC: &[u8; 8] = b"SJSETUP2";
/// magic, compressed length, extracted length, extracted length of the window's files,
/// SHA-256 of the compressed region.
const TRAILER: u64 = 8 + 8 + 8 + 8 + 32;
const CHUNK: usize = 16 << 20;
const MAX_PATH_BYTES: usize = 1024;

pub struct Bundle {
    start: u64,
    length: u64,
    /// Bytes of file records once extracted, for progress.
    pub extracted: u64,
    /// Bytes of records up to the last file the setup window needs.
    pub early: u64,
    hash: [u8; 32],
}

/// The setup window and the manager it runs; everything else can follow while it is open.
fn early(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|c| c.as_os_str() == "ui" || c.as_os_str() == "tools")
}

/// Appends `package` to `out` (which already holds the stub) and the trailer after it.
pub fn pack(package: &Path, out: &mut fs::File) -> Result<()> {
    let mut relative = Vec::new();
    collect(package, package, &mut relative)?;
    relative.sort_by_key(|path| (!early(path), path.clone()));
    let compressor = Compressor::new()?;
    let start = out.stream_position()?;
    let mut hash = Sha256::new();
    let mut extracted = 0u64;
    let mut early_bytes = 0u64;
    let mut pending = Vec::with_capacity(CHUNK);
    let mut flush = |pending: &mut Vec<u8>, out: &mut fs::File| -> Result<()> {
        let packed = compressor.compress(pending)?;
        let header = (packed.len() as u32).to_le_bytes();
        hash.update(header);
        hash.update(&packed);
        out.write_all(&header)?;
        out.write_all(&packed)?;
        pending.clear();
        Ok(())
    };
    for path in relative {
        let name = path.to_str().ok_or("檔名不是 UTF-8。")?.replace('\\', "/");
        let bytes = fs::read(package.join(&path))?;
        let mut record = Vec::with_capacity(12 + name.len());
        record.extend((name.len() as u32).to_le_bytes());
        record.extend(name.as_bytes());
        record.extend((bytes.len() as u64).to_le_bytes());
        for piece in [record.as_slice(), bytes.as_slice()] {
            extracted += piece.len() as u64;
            let mut rest = piece;
            while !rest.is_empty() {
                let take = rest.len().min(CHUNK - pending.len());
                pending.extend_from_slice(&rest[..take]);
                rest = &rest[take..];
                if pending.len() == CHUNK {
                    flush(&mut pending, out)?;
                }
            }
        }
        if early(&path) {
            early_bytes = extracted;
        }
    }
    if !pending.is_empty() {
        flush(&mut pending, out)?;
    }
    let length = out.stream_position()? - start;
    out.write_all(MAGIC)?;
    out.write_all(&length.to_le_bytes())?;
    out.write_all(&extracted.to_le_bytes())?;
    out.write_all(&early_bytes.to_le_bytes())?;
    out.write_all(&hash.finalize())?;
    Ok(())
}

fn collect(root: &Path, folder: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(format!("安裝包不可含有連結：{}", entry.path().display()).into());
        } else if kind.is_dir() {
            collect(root, &entry.path(), out)?;
        } else {
            out.push(entry.path().strip_prefix(root)?.to_owned());
        }
    }
    Ok(())
}

/// The trailer of `exe`, or `None` for a stub without a package (the zip's launcher).
pub fn find(exe: &mut fs::File) -> Result<Option<Bundle>> {
    let size = exe.seek(SeekFrom::End(0))?;
    if size < TRAILER {
        return Ok(None);
    }
    exe.seek(SeekFrom::End(-(TRAILER as i64)))?;
    let mut trailer = [0u8; TRAILER as usize];
    exe.read_exact(&mut trailer)?;
    if &trailer[..8] != MAGIC {
        return Ok(None);
    }
    let length = u64::from_le_bytes(trailer[8..16].try_into()?);
    let extracted = u64::from_le_bytes(trailer[16..24].try_into()?);
    let early = u64::from_le_bytes(trailer[24..32].try_into()?);
    if length > size - TRAILER || early > extracted {
        return Err("安裝程式不完整，請重新下載。".into());
    }
    Ok(Some(Bundle {
        start: size - TRAILER - length,
        length,
        extracted,
        early,
        hash: trailer[32..].try_into()?,
    }))
}

impl Bundle {
    /// Checks the whole compressed region before anything is written.
    pub fn verify(&self, exe: &mut fs::File) -> Result<()> {
        exe.seek(SeekFrom::Start(self.start))?;
        let mut hash = Sha256::new();
        std::io::copy(&mut exe.take(self.length), &mut hash)?;
        if hash.finalize().as_slice() != self.hash {
            return Err("安裝程式校驗失敗，檔案可能下載不完整，請重新下載。".into());
        }
        Ok(())
    }

    /// Writes every file under `destination`. `progress(done)` gets extracted bytes and
    /// returns false to stop; a stopped extraction returns `Ok(false)`. `window_ready` runs
    /// once, as soon as every file of the setup window is written and closed.
    pub fn extract(
        &self,
        exe: &mut fs::File,
        destination: &Path,
        mut progress: impl FnMut(u64) -> bool,
        window_ready: impl FnOnce(),
    ) -> Result<bool> {
        let mut window_ready = Some(window_ready);
        exe.seek(SeekFrom::Start(self.start))?;
        let mut records = Records {
            source: exe.take(self.length),
            decompressor: Decompressor::new()?,
            chunk: Vec::new(),
            at: 0,
            done: 0,
        };
        loop {
            // The window's files end on a record boundary.
            if records.done >= self.early {
                if let Some(ready) = window_ready.take() {
                    ready();
                }
            }
            if records.done >= self.extracted {
                break;
            }
            let length = u32::from_le_bytes(records.array()?) as usize;
            if length == 0 || length > MAX_PATH_BYTES {
                return Err("安裝程式內容格式有誤。".into());
            }
            let name = String::from_utf8(records.bytes(length)?)?;
            let path = destination.join(safe_relative(&name)?);
            let mut left = u64::from_le_bytes(records.array()?);
            fs::create_dir_all(path.parent().ok_or("安裝程式內容格式有誤。")?)?;
            let mut file = fs::File::create(&path)?;
            while left > 0 {
                let piece = records.piece(left)?;
                file.write_all(piece)?;
                left -= piece.len() as u64;
                if !progress(records.done) {
                    return Ok(false);
                }
            }
        }
        if records.done != self.extracted || records.at != records.chunk.len() {
            return Err("安裝程式內容格式有誤。".into());
        }
        Ok(true)
    }
}

/// Only plain relative names; never a drive, root, `..` or an alternate data stream.
fn safe_relative(name: &str) -> Result<PathBuf> {
    let path = PathBuf::from(name.replace('/', "\\"));
    let plain = path.components().all(|c| matches!(c, Component::Normal(_)));
    if name.is_empty() || !plain || name.contains(':') {
        return Err(format!("安裝程式含有不合法的路徑：{name}").into());
    }
    Ok(path)
}

struct Records<'a> {
    source: std::io::Take<&'a mut fs::File>,
    decompressor: Decompressor,
    chunk: Vec<u8>,
    at: usize,
    done: u64,
}
impl Records<'_> {
    fn fill(&mut self) -> Result<()> {
        if self.at < self.chunk.len() {
            return Ok(());
        }
        let mut header = [0u8; 4];
        self.source.read_exact(&mut header)?;
        let mut packed = vec![0u8; u32::from_le_bytes(header) as usize];
        self.source.read_exact(&mut packed)?;
        self.chunk = self.decompressor.decompress(&packed)?;
        self.at = 0;
        Ok(())
    }
    fn piece(&mut self, most: u64) -> Result<&[u8]> {
        self.fill()?;
        let take = (self.chunk.len() - self.at).min(most.min(usize::MAX as u64) as usize);
        let start = self.at;
        self.at += take;
        self.done += take as u64;
        Ok(&self.chunk[start..start + take])
    }
    fn bytes(&mut self, mut length: usize) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(length);
        while length > 0 {
            let piece = self.piece(length as u64)?;
            length -= piece.len();
            out.extend_from_slice(piece);
        }
        Ok(out)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.bytes(N)?.try_into().map_err(|_| "安裝程式內容格式有誤。")?)
    }
}

struct Compressor(COMPRESSOR_HANDLE);
impl Compressor {
    fn new() -> Result<Self> {
        let mut handle = COMPRESSOR_HANDLE::default();
        unsafe { CreateCompressor(COMPRESS_ALGORITHM_LZMS, None, &mut handle)? };
        Ok(Self(handle))
    }
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        // LZMS output never needs more than the input plus a small header for chunk sizes.
        let mut out = vec![0u8; data.len() + data.len() / 8 + 4096];
        let mut written = 0;
        unsafe {
            Compress(
                self.0,
                Some(data.as_ptr().cast()),
                data.len(),
                Some(out.as_mut_ptr().cast()),
                out.len(),
                &mut written,
            )?
        };
        out.truncate(written);
        Ok(out)
    }
}
impl Drop for Compressor {
    fn drop(&mut self) {
        let _ = unsafe { CloseCompressor(self.0) };
    }
}
struct Decompressor(DECOMPRESSOR_HANDLE);
impl Decompressor {
    fn new() -> Result<Self> {
        let mut handle = DECOMPRESSOR_HANDLE::default();
        unsafe { CreateDecompressor(COMPRESS_ALGORITHM_LZMS, None, &mut handle)? };
        Ok(Self(handle))
    }
    fn decompress(&self, packed: &[u8]) -> Result<Vec<u8>> {
        // Buffer mode records the original size in its header; a chunk is at most CHUNK bytes.
        let mut out = vec![0u8; CHUNK];
        let mut written = 0;
        unsafe {
            Decompress(
                self.0,
                Some(packed.as_ptr().cast()),
                packed.len(),
                Some(out.as_mut_ptr().cast()),
                out.len(),
                Some(&mut written),
            )?
        };
        out.truncate(written);
        Ok(out)
    }
}
impl Drop for Decompressor {
    fn drop(&mut self) {
        let _ = unsafe { CloseDecompressor(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("shanjie-bundle-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn package_round_trips_through_the_stub() {
        let root = scratch("round");
        let package = root.join("package");
        fs::create_dir_all(package.join("ui/Assets")).unwrap();
        fs::write(package.join("安裝說明.txt"), "說明".as_bytes()).unwrap();
        fs::write(package.join("ui/Assets/empty.bin"), b"").unwrap();
        fs::create_dir_all(package.join("tools")).unwrap();
        fs::write(package.join("tools/m.exe"), b"manager").unwrap();
        // Sorts before ui by name, but is not needed to open the window.
        fs::create_dir_all(package.join("data")).unwrap();
        fs::write(package.join("data/model.bin"), b"model").unwrap();
        // Larger than one chunk, so records cross chunk boundaries.
        let big: Vec<u8> = (0..CHUNK + 12345).map(|i| (i * 7 % 251) as u8).collect();
        fs::write(package.join("ui/big.bin"), &big).unwrap();
        let exe = root.join("setup.exe");
        let mut out = fs::File::create(&exe).unwrap();
        out.write_all(b"MZ stub bytes").unwrap();
        pack(&package, &mut out).unwrap();
        drop(out);

        let mut file = fs::File::open(&exe).unwrap();
        let bundle = find(&mut file).unwrap().unwrap();
        bundle.verify(&mut file).unwrap();
        let destination = root.join("out");
        let last = std::cell::Cell::new(0);
        let ready_at = std::cell::Cell::new(None);
        let early_file = destination.join("ui/Assets/empty.bin");
        assert!(bundle
            .extract(&mut file, &destination, |done| { last.set(done); true }, || {
                // Every window file exists; nothing outside ui and tools yet.
                ready_at.set(Some(last.get()));
                assert!(early_file.exists() && destination.join("tools/m.exe").exists());
                assert!(!destination.join("data/model.bin").exists());
            })
            .unwrap());
        assert_eq!(last.get(), bundle.extracted);
        assert_eq!(ready_at.get(), Some(bundle.early));
        assert!(0 < bundle.early && bundle.early < bundle.extracted);
        assert_eq!(fs::read(destination.join("安裝說明.txt")).unwrap(), "說明".as_bytes());
        assert_eq!(fs::read(destination.join("ui/Assets/empty.bin")).unwrap(), b"");
        assert_eq!(fs::read(destination.join("ui/big.bin")).unwrap(), big);
        assert!(!bundle.extract(&mut file, &root.join("stopped"), |_| false, || {}).unwrap());
        fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn stub_without_package_and_damaged_payload_are_told_apart() {
        let root = scratch("damaged");
        fs::write(root.join("a.txt"), b"hello hello hello").unwrap();
        let plain = root.join("plain.exe");
        fs::write(&plain, b"MZ just the launcher, no package appended to it at all").unwrap();
        assert!(find(&mut fs::File::open(&plain).unwrap()).unwrap().is_none());

        let exe = root.join("setup.exe");
        let mut out = fs::File::create(&exe).unwrap();
        out.write_all(b"MZ").unwrap();
        let package = root.join("pkg");
        fs::create_dir_all(&package).unwrap();
        fs::copy(root.join("a.txt"), package.join("a.txt")).unwrap();
        pack(&package, &mut out).unwrap();
        drop(out);
        let mut bytes = fs::read(&exe).unwrap();
        bytes[6] ^= 0xff;
        fs::write(&exe, &bytes).unwrap();
        let mut file = fs::File::open(&exe).unwrap();
        let bundle = find(&mut file).unwrap().unwrap();
        assert!(bundle.verify(&mut file).is_err());
        fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn only_plain_relative_names_are_extracted() {
        assert!(safe_relative("ui/Shanjie.UI.exe").is_ok());
        for name in ["", "../x", "ui/../../x", "C:/Windows/x", "/x", "a.txt:stream"] {
            assert!(safe_relative(name).is_err(), "{name}");
        }
    }
}
