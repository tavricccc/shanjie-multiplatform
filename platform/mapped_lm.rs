// Prepared from the upstream parser: score calculation and decode remain upstream.
impl Lm {
    pub fn windows_map_matches(&self, other: &Self) -> bool {
        self.n == other.n && self.d.to_bits() == other.d.to_bits() && self.p_eos.to_bits() == other.p_eos.to_bits()
            && *self.pool == *other.pool && *self.voff == *other.voff && *self.uni == *other.uni
            && *self.ctx_id == *other.ctx_id && *self.ctx_total == *other.ctx_total
            && *self.ctx_back == *other.ctx_back && *self.off == *other.off
            && *self.nxt == *other.nxt && *self.cnt == *other.cnt
    }
    pub fn write_windows_map(&self, path: &Path) -> Result<(), LmError> {
        use std::io::Write;
        let mut file = std::fs::File::create(path).map_err(|_| LmError::Io)?;
        let save = (|| -> std::io::Result<()> {
            file.write_all(b"SJWM0001")?;
            file.write_all(&self.n.to_le_bytes())?;
            file.write_all(&self.d.to_le_bytes())?;
            file.write_all(&self.p_eos.to_le_bytes())?;
            crate::mapped_array::write(&mut file,&self.pool)?;
            crate::mapped_array::write(&mut file,&self.voff)?;
            crate::mapped_array::write(&mut file,&self.uni)?;
            crate::mapped_array::write(&mut file,&self.ctx_id)?;
            crate::mapped_array::write(&mut file,&self.ctx_total)?;
            crate::mapped_array::write(&mut file,&self.ctx_back)?;
            crate::mapped_array::write(&mut file,&self.off)?;
            crate::mapped_array::write(&mut file,&self.nxt)?;
            crate::mapped_array::write(&mut file,&self.cnt)?;
            file.sync_all()
        })();
        save.map_err(|_| LmError::Io)
    }
    fn load_windows_map(path: &Path) -> Result<Self, LmError> {
        use crate::mapped_array::{Mapping, read};
        let owner = Mapping::open(path).map_err(|_| LmError::Io)?;
        let bytes = owner.bytes();
        if bytes.get(..8) != Some(b"SJWM0001") || bytes.len() < 32 { return Err(LmError::Format); }
        let n = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let d = f64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let p_eos = f64::from_le_bytes(bytes[24..32].try_into().unwrap());
        let mut cursor = 32;
        let model = Self {
            n,d,p_eos,
            pool: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
            voff: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
            uni: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
            ctx_id: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
            ctx_total: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
            ctx_back: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
            off: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
            nxt: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
            cnt: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
        };
        if cursor != bytes.len() || n == 0 || !d.is_finite() || !p_eos.is_finite() { return Err(LmError::Format); }
        let v = model.uni.len(); let c = model.ctx_id.len(); let e = model.nxt.len();
        if v < 2 || model.voff.len() != v + 1 || model.voff[0] != 0
            || model.voff[v] as usize != model.pool.len()
            || model.voff.windows(2).any(|w| w[0] > w[1])
            || model.ctx_total.len() != c || model.ctx_back.len() != c
            || model.off.len() != c + 1 || model.off[0] != 0 || model.off[c] as usize != e
            || model.off.windows(2).any(|w| w[0] > w[1]) || model.cnt.len() != e
            || model.ctx_id.iter().any(|&i| i as usize >= v)
            || model.ctx_id.windows(2).any(|w| w[0] >= w[1])
            || model.nxt.iter().any(|&i| i as usize >= v)
            || model.ctx_total.iter().any(|t| !t.is_finite() || *t <= 0.0)
            || model.ctx_back.iter().any(|t| !t.is_finite()) { return Err(LmError::Format); }
        for i in 0..c {
            if model.nxt[model.off[i] as usize..model.off[i+1] as usize].windows(2).any(|w| w[0] >= w[1]) { return Err(LmError::Format); }
        }
        if model.vocab_word(0) != b"<s>" || model.vocab_word(1) != b"</s>"
            || (0..v).any(|i| std::str::from_utf8(model.vocab_word(i)).is_err())
            || (3..v).any(|i| model.vocab_word(i-1) >= model.vocab_word(i)) { return Err(LmError::Format); }
        Ok(model)
    }
}
