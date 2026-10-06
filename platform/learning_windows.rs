// Windows filesystem backend. Serialization and validation above this implementation
// are extracted from the pinned upstream module by prepare-core.ps1.
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
const REPARSE: u32 = 0x400;
const OPEN_REPARSE: u32 = 0x00200000;

fn checked_file(path: &Path, append: bool) -> Result<File, StoreError> {
    let mut options = OpenOptions::new();
    options.read(!append).append(append).custom_flags(OPEN_REPARSE);
    let file = io(options.open(path))?;
    let metadata = io(file.metadata())?;
    if !metadata.is_file() || metadata.file_attributes() & REPARSE != 0 {
        return Err(StoreError::Io);
    }
    Ok(file)
}

impl LearnStore {
    pub fn open(dir: &Path) -> Result<(Self, Vec<Record>, Opened), StoreError> {
        io(fs::create_dir_all(dir))?;
        let metadata = io(fs::symlink_metadata(dir))?;
        if !metadata.is_dir() || metadata.file_attributes() & REPARSE != 0 {
            return Err(StoreError::Io);
        }
        let store = Self { dir: dir.to_path_buf() };
        let (records, opened) = store.load()?;
        Ok((store, records, opened))
    }
    fn load(&self) -> Result<(Vec<Record>, Opened), StoreError> {
        let path = self.dir.join(FILE);
        if !path.try_exists().map_err(|_| StoreError::Io)? {
            return Ok((Vec::new(), Opened::Fresh));
        }
        let file = checked_file(&path, false)?;
        let mut bytes = Vec::new();
        io((&file).take(MAX_BYTES + 1).read_to_end(&mut bytes))?;
        drop(file);
        let mut rows = bytes.split(|&b| b == b'\n');
        if bytes.len() as u64 > MAX_BYTES || rows.next() != Some(HEADER.as_bytes()) {
            io(fs::rename(path, self.dir.join(CORRUPT)))?;
            return Ok((Vec::new(), Opened::Corrupt));
        }
        let mut records = Vec::<Record>::new();
        let mut positions = std::collections::HashMap::new();
        let mut skipped = 0;
        for row in rows.filter(|row| !row.is_empty()) {
            let Some(record) = std::str::from_utf8(row).ok().and_then(parse) else {
                skipped += 1;
                continue;
            };
            let key = (record.context.clone(), record.reading.clone(), record.word.clone());
            if let Some(&index) = positions.get(&key) {
                records[index] = record;
            } else {
                positions.insert(key, records.len());
                records.push(record);
            }
        }
        Ok((records, Opened::Loaded { skipped }))
    }
    pub fn save(&self, records: &[Record]) -> Result<(), StoreError> { self.rewrite(records) }
    pub fn save_forgetting(&self, records: &[Record]) -> Result<(), StoreError> { self.rewrite(records) }
    fn rewrite(&self, records: &[Record]) -> Result<(), StoreError> {
        let mut buffer = String::from(HEADER);
        buffer.push('\n');
        lines(records, &mut buffer);
        let temporary = self.dir.join(TMP);
        remove(&temporary)?;
        let mut file = io(OpenOptions::new().write(true).create_new(true).open(&temporary))?;
        io(file.write_all(buffer.as_bytes()))?;
        io(file.sync_all())?;
        drop(file);
        // Rust uses MoveFileExW(REPLACE_EXISTING) on Windows: no delete-then-rename gap.
        io(fs::rename(temporary, self.dir.join(FILE)))
    }
    pub fn append(&self, records: &[Record]) -> Result<(), StoreError> {
        let mut buffer = String::new();
        lines(records, &mut buffer);
        let mut file = checked_file(&self.dir.join(FILE), true)?;
        let size = io(file.metadata())?.len();
        if size <= HEADER.len() as u64 || size + buffer.len() as u64 > MAX_BYTES {
            return Err(StoreError::Io);
        }
        io(file.write_all(buffer.as_bytes()))
    }
    pub fn clear(&self) -> Result<(), StoreError> {
        let mut result = Ok(());
        for name in [FILE, TMP, CORRUPT] {
            if let Err(error) = remove(&self.dir.join(name)) { result = Err(error); }
        }
        result
    }
    pub fn dir(&self) -> &Path { &self.dir }
}
