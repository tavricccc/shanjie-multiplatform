// Prepared from the upstream parser: score calculation and decode remain upstream.
impl Lm {
    pub fn windows_map_matches(&self, other: &Self) -> bool {
        self.sha == other.sha && self.n == other.n && self.d.to_bits() == other.d.to_bits() && self.p_eos.to_bits() == other.p_eos.to_bits()
            && *self.pool == *other.pool && *self.voff == *other.voff && *self.uni == *other.uni
            && *self.ctx_id == *other.ctx_id && *self.ctx_total == *other.ctx_total
            && *self.ctx_back == *other.ctx_back && *self.off == *other.off
            && *self.nxt == *other.nxt && *self.cnt == *other.cnt
            && match (&self.classes,&other.classes) {
                (None,None) => true,
                (Some(a),Some(b)) => a.k3 == b.k3 && a.mu.to_bits() == b.mu.to_bits()
                    && *a.cls == *b.cls && *a.emit == *b.emit && *a.pc == *b.pc,
                _ => false,
            }
    }
    pub fn write_windows_map(&self, path: &Path, source: &[u8], classes: Option<&[u8]>) -> Result<(), LmError> {
        use std::io::Write;
        let mut file = std::fs::File::create(path).map_err(|_| LmError::Io)?;
        let save = (|| -> std::io::Result<()> {
            file.write_all(b"SJWM0002")?;
            file.write_all(&self.n.to_le_bytes())?;
            file.write_all(&self.d.to_le_bytes())?;
            file.write_all(&self.p_eos.to_le_bytes())?;
            file.write_all(&windows_digest(source))?;
            file.write_all(&classes.map(windows_digest).unwrap_or([0;32]))?;
            file.write_all(&(self.classes.is_some() as u64).to_le_bytes())?;
            crate::mapped_array::write(&mut file,&self.pool)?;
            crate::mapped_array::write(&mut file,&self.voff)?;
            crate::mapped_array::write(&mut file,&self.uni)?;
            crate::mapped_array::write(&mut file,&self.ctx_id)?;
            crate::mapped_array::write(&mut file,&self.ctx_total)?;
            crate::mapped_array::write(&mut file,&self.ctx_back)?;
            crate::mapped_array::write(&mut file,&self.off)?;
            crate::mapped_array::write(&mut file,&self.nxt)?;
            crate::mapped_array::write(&mut file,&self.cnt)?;
            if let Some(c) = &self.classes {
                file.write_all(&(c.k3 as u64).to_le_bytes())?;
                file.write_all(&c.mu.to_le_bytes())?;
                crate::mapped_array::write(&mut file,&c.cls)?;
                crate::mapped_array::write(&mut file,&c.emit)?;
                crate::mapped_array::write(&mut file,&c.pc)?;
            }
            file.sync_all()
        })();
        save.map_err(|_| LmError::Io)
    }
    fn load_windows_map(path: &Path) -> Result<Self, LmError> {
        use crate::mapped_array::{Mapping, read};
        let owner = Mapping::open(path).map_err(|_| LmError::Io)?;
        let bytes = owner.bytes();
        if bytes.get(..8) != Some(b"SJWM0002") || bytes.len() < 104 { return Err(LmError::Format); }
        let n = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let d = f64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let p_eos = f64::from_le_bytes(bytes[24..32].try_into().unwrap());
        let classed = u64::from_le_bytes(bytes[96..104].try_into().unwrap());
        if classed > 1 { return Err(LmError::Format); }
        let mut cursor = 104;
        let mut model = Self {
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
            classes: None,
            sha: bytes[32..64].iter().map(|b| format!("{b:02x}")).collect(),
            kn: None,
            tri: None,
        };
        if classed == 1 {
            // Keep the released class table beside the map. Missing/wrong pairs fail,
            // while scoring uses the aligned, shared arrays from the verified map.
            let table = std::fs::read(path.with_file_name("classes.sjc")).map_err(|_| LmError::Io)?;
            if table.get(..8) != Some(CLASS_MAGIC) || table.get(8..40) != Some(&bytes[32..64])
                || windows_digest(&table) != bytes[64..96] { return Err(LmError::Format); }
            let end = cursor.checked_add(16).ok_or(LmError::Format)?;
            let header = bytes.get(cursor..end).ok_or(LmError::Format)?;
            let k3 = usize::try_from(u64::from_le_bytes(header[..8].try_into().unwrap())).map_err(|_| LmError::Format)?;
            let mu = f64::from_le_bytes(header[8..16].try_into().unwrap());
            cursor = end;
            let c = Classes { k3,mu,
                cls: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
                emit: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
                pc: read(&owner,&mut cursor).map_err(|_| LmError::Format)?,
            };
            let prob_ok = |x: &f64| (0.0..=1.0).contains(x);
            if !(0.0..=1.0).contains(&mu) || c.cls.len() != model.uni.len() || c.emit.len() != c.cls.len()
                || k3.checked_mul(k3) != Some(c.pc.len()) || c.cls.iter().any(|&v| v != NO_CLASS && v as usize >= k3)
                || !c.emit.iter().all(prob_ok) || !c.pc.iter().all(prob_ok) { return Err(LmError::Format); }
            model.classes = Some(c);
        }
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
        let side = path.with_file_name("kn.sjkn");
        match std::fs::symlink_metadata(&side) {
            Ok(_) => model.add_kn(&std::fs::read(&side).map_err(|_| LmError::KnSide)?)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(LmError::KnSide),
        }
        // Same rule as upstream `load_opts`: a `trigram.sjt3` beside the model is mapped and must match it;
        // none is off (the package ships none, like upstream's app).
        let tri = path.with_file_name("trigram.sjt3");
        match std::fs::symlink_metadata(&tri) {
            Ok(_) => {
                // SAFETY: the Windows mapping denies writers and deleters while it is open.
                let map = unsafe { Mapped::open(&tri) }.map_err(|()| LmError::TriTable)?;
                model.add_tri_bytes(TriBytes::Map(map))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(LmError::TriTable),
        }
        Ok(model)
    }
}

fn windows_digest(bytes: &[u8]) -> [u8;32] {
    let hex = crate::eval::sha256_hex(bytes);
    std::array::from_fn(|i| u8::from_str_radix(&hex[2*i..2*i+2],16).unwrap())
}

#[cfg(test)]
mod windows_map_trigram {
    use super::*;
    #[test]
    fn a_trigram_table_beside_the_map_is_used_like_upstream() {
        let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../eval/golden");
        let model = std::fs::read(golden.join("kn-tiny/bigram.sjlm")).unwrap();
        let classes = std::fs::read(golden.join("kn-tiny/classes.sjc")).unwrap();
        let dir = std::env::temp_dir().join(format!("shanjie-winmap-tri-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("classes.sjc"), &classes).unwrap();
        let map = dir.join("bigram.winmap");
        Lm::parse_with_classes(&model, &classes).unwrap().write_windows_map(&map, &model, Some(&classes)).unwrap();
        assert!(Lm::load(&map).unwrap().tri.is_none(), "no table: off");
        std::fs::copy(golden.join("tri-tiny/trigram.sjt3"), dir.join("trigram.sjt3")).unwrap();
        {
            let loaded = Lm::load(&map).unwrap();
            assert!(loaded.tri.as_ref().is_some_and(Tri::is_mapped), "the table beside the map is mapped");
        }
        std::fs::write(dir.join("trigram.sjt3"), b"garbage").unwrap();
        assert_eq!(Lm::load(&map).err(), Some(LmError::TriTable), "a bad table is refused, not ignored");
        std::fs::remove_dir_all(&dir).ok();
    }
}
