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
            selected: (!candidates.is_empty()).then_some(0), columns: 0, first: 0, total: candidates.len() as u32, vertical: 0, candidates }
    }
    pub fn new_with_personal(data: &Path, layout: Layout, personal: Option<&Path>) -> Result<Engine,EngineError> {
        let Some(personal) = personal else { return Engine::new(data,layout) };
        let original = std::fs::read_to_string(data.join("mcbpmf-data.txt")).map_err(|_| EngineError::LoadFailed)?;
        // Personal rows use the base reading/word/score format, so existing words may be reprioritized.
        let mut base = std::fs::read_to_string(personal).map_err(|_| EngineError::LoadFailed)?;
        base.reserve_exact(original.len()+1); base.push('\n'); base.push_str(&original); drop(original);
        let overlay = std::fs::read_to_string(data.join("overlay-add.tsv")).map_err(|_| EngineError::LoadFailed)?;
        let sandhi = std::fs::read_to_string(data.join("sandhi-add.tsv")).map_err(|_| EngineError::LoadFailed)?;
        for line in join_overlays(overlay,&sandhi).lines() {
            let (reading,word,score) = crate::overlay_row(line).ok_or(EngineError::LoadFailed)?;
            base.push_str(&format!("\n{reading} {word} {score}"));
        }
        let lex = Arc::new(Lexicon::parse(&base).map_err(|_| EngineError::LoadFailed)?);
        let demote = std::fs::read_to_string(data.join("demote.tsv")).map_err(|_| EngineError::LoadFailed)?;
        if !Demote::parse(&demote).is_some_and(|d| d.check(&lex)) { return Err(EngineError::LoadFailed); }
        let mut engine = Engine::with_lexicon(lex,layout); engine.data_dir = Some(data.to_path_buf()); Ok(engine)
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
