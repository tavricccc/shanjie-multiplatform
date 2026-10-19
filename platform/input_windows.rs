// Appended to the pinned engine.rs in the Windows build; no changes to its scorer.
impl Engine {
    pub fn associated_words(&self, prefix: &str) -> Output {
        let word_at = |position: usize| {
            let ent = self.lex.ents[self.lex.by_word[position] as usize];
            (&self.lex.words[ent.off as usize..(ent.off + ent.len) as usize], ent.score)
        };
        let first = self.lex.by_word.partition_point(|&index| {
            let ent = self.lex.ents[index as usize];
            &self.lex.words[ent.off as usize..(ent.off + ent.len) as usize] < prefix
        });
        let mut words = Vec::new();
        for index in first..self.lex.by_word.len() {
            let (word, score) = word_at(index);
            if !word.starts_with(prefix) { break; }
            if word.len() > prefix.len() && (!self.candidate_big5 || fits_big5(word)) { words.push((word[prefix.len()..].to_owned(), score)); }
        }
        words.sort_by(|a,b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        words.truncate(81);
        let candidates: Vec<_> = words.into_iter().map(|(word,_)| word).collect();
        Output { handled: true, commit: String::new(), preedit: String::new(), cursor_utf16: 0,
            selected: (!candidates.is_empty()).then_some(0), columns: 0, first: 0, total: candidates.len() as u32, vertical: 0, candidates, english_run_utf16_start: 0, english_run_utf16_len: 0 }
    }
    pub fn new_with_personal(data: &Path, layout: Layout, personal: Option<&Path>, packs: u32) -> Result<Engine,EngineError> {
        if packs & !PACK_ALL != 0 { return Err(EngineError::LoadFailed); }
        let pack_dir = data.join("packs");
        let Some(personal) = personal else { return Engine::new_with_packs(data,layout,(packs != 0).then_some((pack_dir.as_path(),packs))) };
        let original = std::fs::read_to_string(data.join("mcbpmf-data.txt")).map_err(|_| EngineError::LoadFailed)?;
        // Personal rows use the base reading/word/score format, so existing words may be reprioritized.
        let mut base = std::fs::read_to_string(personal).map_err(|_| EngineError::LoadFailed)?;
        base.reserve_exact(original.len()+1); base.push('\n'); base.push_str(&original); drop(original);
        let overlay = std::fs::read_to_string(data.join("overlay-add.tsv")).map_err(|_| EngineError::LoadFailed)?;
        let sandhi = std::fs::read_to_string(data.join("sandhi-add.tsv")).map_err(|_| EngineError::LoadFailed)?;
        // Same fixed order as upstream load_lexicon: overlay, sandhi, then the always-on names layer.
        let names = std::fs::read_to_string(data.join(NAMES_FILE)).map_err(|_| EngineError::LoadFailed)?;
        let mut overlay = join_overlays(join_overlays(overlay,&sandhi),&names);
        let pack_text = if packs != 0 { read_packs_detailed(&pack_dir,packs).map_err(|_| EngineError::LoadFailed)? } else { String::new() };
        if !pack_text.is_empty() { overlay = join_overlays(overlay,&pack_text); }
        for line in overlay.lines() {
            let (reading,word,score) = crate::overlay_row(line).ok_or(EngineError::LoadFailed)?;
            base.push_str(&format!("\n{reading} {word} {score}"));
        }
        let lex = Arc::new(Lexicon::parse(&base).map_err(|_| EngineError::LoadFailed)?);
        let demote = std::fs::read_to_string(data.join("demote.tsv")).map_err(|_| EngineError::LoadFailed)?;
        if !Demote::parse(&demote).is_some_and(|d| d.check(&lex)) { return Err(EngineError::LoadFailed); }
        let mut engine = Engine::with_lexicon(lex,layout); engine.data_dir = Some(data.to_path_buf());
        engine.pack_text = Some(pack_text).filter(|text| !text.is_empty());
        engine.digit_table = windows_digit_table(data); Ok(engine)
    }
    pub fn set_candidate_charset(&mut self, big5: bool) { self.candidate_big5 = big5; self.cands = None; self.clear_pred(); }
    pub fn is_prediction_row(&self) -> bool { self.cands.is_none() && !self.pred.is_empty() }
    pub fn candidate_reading(&self) -> String {
        let Some(cands) = &self.cands else { return String::new() };
        let Some((_,length)) = cands.list.get(cands.sel) else { return String::new() };
        let (start,end) = self.candidate_span(*length);
        if (start..end).any(|index| self.is_punct(index)) { return String::new(); }
        self.syls[start..end].concat()
    }
    pub fn commit_candidate_reading(&mut self) -> Output {
        let reading = self.candidate_reading();
        if reading.is_empty() { return self.view(false,String::new()); }
        let length = self.cands.as_ref().unwrap().list[self.cands.as_ref().unwrap().sel].1;
        let (start,end) = self.candidate_span(length);
        let before = (0..start).map(|i| self.token_width(i)).sum::<usize>();
        let after = (0..end).map(|i| self.token_width(i)).sum::<usize>();
        let commit = self.display.chars().take(before).collect::<String>() + &reading + &self.display.chars().skip(after).collect::<String>();
        self.clear_all(); self.view(true,commit)
    }
    /// Windows policy on a window upstream just opened: the Big5 and reading-text filters, the typed-zhuyin and
    /// original-letters items. Windows built from a prediction row, English or digit windows keep upstream's list
    /// (their indexes carry meaning); only the lexicon items behind a row are filtered.
    fn decorate_windows_candidates(&mut self) {
        let Some(mut cands) = self.cands.take() else { return };
        let plain = !cands.english && cands.dalt.is_none() && cands.rows.is_empty() && !cands.list.is_empty();
        let touching = self.cursor.saturating_sub(1);
        let digits = self.syls.get(touching).is_some_and(|reading| crate::learn::digit_of(reading).is_some());
        if !plain || digits {
            let kept = cands.rows.len();
            if self.candidate_big5 && !cands.english && cands.dalt.is_none() {
                let tail = cands.list.split_off(kept.min(cands.list.len()));
                cands.list.extend(tail.into_iter().filter(|(word,_)| fits_big5(word)));
            }
            self.cands = (!cands.list.is_empty()).then_some(cands);
            return;
        }
        let list = std::mem::take(&mut cands.list);
        self.cands = self.make_candidates(list);
    }
    fn make_candidates(&self, mut list: Vec<(String,usize)>) -> Option<Cands> {
        let (_,length) = list.first()?.clone();
        if !self.space_inputs_zhuyin { list.retain(|(word,_)| !windows_reading_text(word)); }
        if self.candidate_big5 { list.retain(|(word,_)| fits_big5(word)); }
        let (start,end) = self.candidate_span(length);
        let mut cands = Cands::new(list, self.cand_vertical);
        if end <= self.syls.len() && !(start..end).any(|index| self.is_punct(index)) {
            if !self.space_inputs_zhuyin {
                let text = self.syls[start..end].concat();
                let has_word = !cands.list.is_empty();
                cands.list.insert(0,(text.clone(),length));
                cands.literals.insert(0,WindowsLiteralCandidate { text,start,end });
                cands.sel = usize::from(has_word);
            }
            if self.mixed_input {
                let original: Option<Vec<_>> = (start..end).map(|i| self.typed_literals.get(&i).cloned()).collect();
                if let Some(original) = original {
                    let text = original.concat();
                    let index = (cands.sel + 1).min(cands.list.len());
                    cands.list.insert(index,(text.clone(),length));
                    cands.literals.insert(index,WindowsLiteralCandidate { text,start,end });
                }
            }
        }
        (!cands.list.is_empty()).then_some(cands)
    }
}
/// The optional digit count table, read as upstream `new_with_packs` reads it (digits.md A.3.2):
/// missing or malformed leaves the digit layer off.
fn windows_digit_table(data: &Path) -> Option<Arc<DigitTable>> {
    std::fs::read_to_string(data.join("digits.tsv")).ok().and_then(|t| DigitTable::parse(&t)).map(Arc::new)
}
// glibc's CP950 is the same code page; without //TRANSLIT it never substitutes, matching
// WC_NO_BEST_FIT_CHARS. The descriptor is kept per thread; without one nothing is filtered.
#[cfg(unix)]
fn fits_big5(word: &str) -> bool {
    use std::{cell::Cell, ffi::{c_char, c_void}, ptr::null_mut};
    extern "C" {
        fn iconv_open(to: *const c_char, from: *const c_char) -> *mut c_void;
        fn iconv(cd: *mut c_void, input: *mut *mut c_char, input_left: *mut usize, output: *mut *mut c_char, output_left: *mut usize) -> usize;
    }
    thread_local! { static CD: Cell<*mut c_void> = const { Cell::new(null_mut()) }; }
    let cd = CD.with(|cd| {
        if cd.get().is_null() { cd.set(unsafe { iconv_open(c"CP950".as_ptr(), c"UTF-8".as_ptr()) }); }
        cd.get()
    });
    if cd as isize == -1 { return true; }
    let mut input = word.as_bytes().to_vec();
    let mut output = vec![0u8; input.len() * 2 + 8];
    let (mut input_ptr, mut input_left) = (input.as_mut_ptr().cast::<c_char>(), input.len());
    let (mut output_ptr, mut output_left) = (output.as_mut_ptr().cast::<c_char>(), output.len());
    unsafe {
        iconv(cd, null_mut(), null_mut(), null_mut(), null_mut());
        iconv(cd, &mut input_ptr, &mut input_left, &mut output_ptr, &mut output_left) == 0 && input_left == 0
    }
}
#[cfg(windows)]
fn fits_big5(word: &str) -> bool {
    #[link(name="kernel32")]
    unsafe extern "system" { fn WideCharToMultiByte(page: u32, flags: u32, text: *const u16, length: i32, output: *mut u8, capacity: i32, default: *const u8, used: *mut i32) -> i32; }
    let wide: Vec<u16> = word.encode_utf16().collect(); let mut bytes = vec![0u8; wide.len()*2]; let mut used = 0;
    let length = unsafe { WideCharToMultiByte(950,0x400,wide.as_ptr(),wide.len() as i32,bytes.as_mut_ptr(),bytes.len() as i32,std::ptr::null(),&mut used) };
    length > 0 && used == 0
}

#[cfg(test)]
mod windows_input_tests {
    use super::*;
    #[test]
    fn big5_candidate_filter_preserves_unicode_default() {
        let lex = Lexicon::parse("ㄐㄧˊ 𠮷 -0.1\nㄐㄧˊ 吉 -0.2\n").unwrap();
        let mut engine = Engine::with_lexicon(Arc::new(lex),Layout::Standard);
        for ch in "ru6".chars() { engine.key(Key::ch(ch,0)).unwrap(); }
        assert!(engine.key(Key::new(KeyKind::Down)).unwrap().candidates.contains(&"𠮷".to_owned()));
        engine.set_candidate_charset(true);
        assert_eq!(engine.key(Key::new(KeyKind::Down)).unwrap().candidates,vec!["吉"]);
    }
}
#[cfg(test)]
mod personal_lexicon_tests {
    use super::*;
    #[test]
    fn personal_lexicon_keeps_the_names_layer() {
        let dir = std::env::temp_dir().join(format!("shanjie-names-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mcbpmf-data.txt"), "ㄨㄛˇ 我 -1.0\n").unwrap();
        std::fs::write(dir.join("overlay-add.tsv"), "").unwrap();
        std::fs::write(dir.join("sandhi-add.tsv"), "").unwrap();
        std::fs::write(dir.join("names-add.tsv"), "ㄊㄞˊ-ㄨㄢ-ㄧˋ-ㄉㄚˋ\t臺灣藝大\t-4.6\tnames\n").unwrap();
        std::fs::write(dir.join("demote.tsv"), "").unwrap();
        let personal = dir.join("dictionary.tsv");
        std::fs::write(&personal, "ㄋㄧˇ 你 -1.0\n").unwrap();
        let plain = Engine::new(&dir, Layout::Standard).unwrap();
        let with_personal = Engine::new_with_personal(&dir, Layout::Standard, Some(&personal), 0).unwrap();
        assert_eq!(plain.associated_words("臺灣藝").candidates, ["大"]);
        assert_eq!(with_personal.associated_words("臺灣藝").candidates, ["大"]);
        std::fs::remove_dir_all(&dir).ok();
    }
    #[test]
    fn acg_pack_loads_with_and_without_a_personal_dictionary() {
        let dir = std::env::temp_dir().join(format!("shanjie-pack-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("packs")).unwrap();
        std::fs::write(dir.join("mcbpmf-data.txt"), "ㄨㄛˇ 我 -1.0\n").unwrap();
        for name in ["overlay-add.tsv", "sandhi-add.tsv", "names-add.tsv", "demote.tsv"] { std::fs::write(dir.join(name), "").unwrap(); }
        std::fs::write(dir.join("packs/acg-add.tsv"), "ㄇㄧˇ-ㄎㄚ-ㄕㄚ\t米卡莎\t-4.6\tacg\n").unwrap();
        let personal = dir.join("dictionary.tsv");
        std::fs::write(&personal, "ㄋㄧˇ 你 -1.0\n").unwrap();
        for source in [None, Some(personal.as_path())] {
            // The second iteration of each pair is served from the resident cache.
            for _ in 0..2 {
                let on = Engine::new_with_personal(&dir, Layout::Standard, source, PACK_ACG).unwrap();
                assert_eq!(on.associated_words("米卡").candidates, ["莎"]);
                assert!(on.pack_text.is_some());
            }
            let off = Engine::new_with_personal(&dir, Layout::Standard, source, 0).unwrap();
            assert!(off.associated_words("米卡").candidates.is_empty());
            assert!(off.pack_text.is_none());
        }
        assert!(Engine::new_with_personal(&dir, Layout::Standard, None, 2).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
    #[test]
    fn digit_table_loads_on_every_lexicon_path() {
        let dir = std::env::temp_dir().join(format!("shanjie-digits-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mcbpmf-data.txt"), "ㄨㄛˇ 我 -1.0\n").unwrap();
        for name in ["overlay-add.tsv", "sandhi-add.tsv", "names-add.tsv", "demote.tsv"] { std::fs::write(dir.join(name), "").unwrap(); }
        let personal = dir.join("dictionary.tsv");
        std::fs::write(&personal, "ㄋㄧˇ 你 -1.0\n").unwrap();
        for source in [None, Some(personal.as_path())] {
            assert!(Engine::new_with_personal(&dir, Layout::Standard, source, 0).unwrap().digit_table.is_none(), "no table: digits stay off");
        }
        let mut table = String::from("T\t1000\nN\t100\nS\t10\nD\t<s>\t5\t100\n");
        for k in 1..=9 { table += &format!("L\t{k}\t10\n"); }
        table += "L\t10+\t10\n";
        for c in "0123456789./-".chars() { table += &format!("C\t{c}\t10\n"); }
        std::fs::write(dir.join("digits.tsv"), table).unwrap();
        for source in [None, Some(personal.as_path())] {
            // The second engine of each pair comes from the resident cache.
            for _ in 0..2 {
                assert!(Engine::new_with_personal(&dir, Layout::Standard, source, 0).unwrap().digit_table.is_some(), "personal={}", source.is_some());
            }
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
