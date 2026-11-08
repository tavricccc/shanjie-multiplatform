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
            if word.len() > prefix.len() { words.push((word[prefix.len()..].to_owned(), score)); }
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
        let mut personal_keys: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
        for line in base.lines() {
            let mut fields = line.split_whitespace();
            let (Some(reading), Some(word), Some(_), None) = (fields.next(), fields.next(), fields.next(), fields.next()) else { continue };
            if word.chars().count() == reading.split('-').count() {
                personal_keys.entry(reading.to_owned()).or_default().insert(word.to_owned());
            }
        }
        let replaced = |r: &str, w: &str| personal_keys.get(r).is_some_and(|words| words.contains(w));
        // Replace only the same reading/word pair. Other pronunciations of the word survive.
        let original: String = original.lines().filter(|line| {
            let mut fields = line.split_whitespace();
            !matches!((fields.next(), fields.next()), (Some(r), Some(w)) if replaced(r,w))
        }).map(|line| format!("{line}\n")).collect();
        base.reserve_exact(original.len()+1); base.push('\n'); base.push_str(&original); drop(original);
        let overlay = std::fs::read_to_string(data.join("overlay-add.tsv")).map_err(|_| EngineError::LoadFailed)?;
        let sandhi = std::fs::read_to_string(data.join("sandhi-add.tsv")).map_err(|_| EngineError::LoadFailed)?;
        // Same fixed order as upstream load_lexicon: overlay, sandhi, then the always-on names layer.
        let names = std::fs::read_to_string(data.join(NAMES_FILE)).map_err(|_| EngineError::LoadFailed)?;
        let mut overlay = join_overlays(join_overlays(overlay,&sandhi),&names);
        let (pack_text, pack_oov_len) = if packs != 0 { read_packs_split(&pack_dir,packs).map_err(|_| EngineError::LoadFailed)? } else { (String::new(), 0) };
        if !pack_text.is_empty() { overlay = join_overlays(overlay,&pack_text); }
        // Keep the original manual protection, including a manual row replaced by a personal score.
        // Keep pack_text unchanged too: upstream uses its tags to identify pack OOV words.
        let manual = crate::ManualSet::from_pairs(overlay.lines().filter_map(|line| {
            crate::overlay_row(line).and_then(|(r,w,_,tag)| (tag == "manual").then_some((w,r)))
        }));
        let filtered: String = overlay.lines().filter(|line| {
            !crate::overlay_row(line).is_some_and(|(r,w,_,_)| replaced(r,w))
        }).map(|line| format!("{line}\n")).collect();
        let mut lex = Lexicon::parse_with(&base,Some(&filtered)).map_err(|_| EngineError::LoadFailed)?;
        lex.manual = Arc::new(manual);
        let lex = Arc::new(lex);
        let demote = std::fs::read_to_string(data.join("demote.tsv")).map_err(|_| EngineError::LoadFailed)?;
        if !Demote::parse(&demote).is_some_and(|d| d.check(&lex)) { return Err(EngineError::LoadFailed); }
        let mut engine = Engine::with_lexicon(lex,layout); engine.data_dir = Some(data.to_path_buf());
        engine.pack_text = Some(pack_text).filter(|text| !text.is_empty());
        engine.pack_oov_len = pack_oov_len;
        engine.digit_table = windows_digit_table(data); Ok(engine)
    }
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
    /// Windows policy on a window upstream just opened: the reading-text filter, the typed-zhuyin and
    /// original-letters items. Windows built from a prediction row, English or digit windows keep upstream's list
    /// (their indexes carry meaning).
    fn decorate_windows_candidates(&mut self) {
        let Some(mut cands) = self.cands.take() else { return };
        let plain = cands.short.is_none() && cands.zh.is_none() && cands.dalt.is_none() && cands.rows.is_empty() && !cands.list.is_empty();
        let touching = self.cursor.saturating_sub(1);
        let digits = self.syls.get(touching).is_some_and(|reading| crate::learn::digit_of(reading).is_some());
        if !plain || digits {
            self.cands = (!cands.list.is_empty()).then_some(cands);
            return;
        }
        let list = std::mem::take(&mut cands.list);
        self.cands = self.make_candidates_with(list, std::mem::take(&mut cands.alt));
    }
    fn make_candidates(&self, list: Vec<(String,usize)>) -> Option<Cands> {
        self.make_candidates_with(list, HashMap::new())
    }
    /// `alt` is upstream's map from a list position to a tolerant item's lexicon reading; it follows its items.
    fn make_candidates_with(&self, list: Vec<(String,usize)>, alt: HashMap<usize,Vec<String>>) -> Option<Cands> {
        let (_,length) = list.first()?.clone();
        let keep: Vec<bool> = list.iter().map(|(word,_)| self.space_inputs_zhuyin || !windows_reading_text(word)).collect();
        let (start,end) = self.candidate_span(length);
        let mut cands = Cands::new(list, self.cand_vertical);
        cands.alt = alt;
        retain_with_alt(&mut cands, &keep);
        if end <= self.syls.len() && !(start..end).any(|index| self.is_punct(index)) {
            if !self.space_inputs_zhuyin {
                let text = self.syls[start..end].concat();
                let has_word = !cands.list.is_empty();
                insert_with_alt(&mut cands, 0, (text.clone(),length));
                cands.literals.insert(0,WindowsLiteralCandidate { text,start,end });
                cands.sel = usize::from(has_word);
            }
            if self.mixed_input {
                let original: Option<Vec<_>> = (start..end).map(|i| self.typed_literals.get(&i).cloned()).collect();
                if let Some(original) = original {
                    let text = original.concat();
                    let index = (cands.sel + 1).min(cands.list.len());
                    insert_with_alt(&mut cands, index, (text.clone(),length));
                    cands.literals.insert(index,WindowsLiteralCandidate { text,start,end });
                }
            }
        }
        (!cands.list.is_empty()).then_some(cands)
    }
}
fn retain_with_alt(cands: &mut Cands, keep: &[bool]) {
    let mut alt = HashMap::new();
    let mut next = 0;
    let list = std::mem::take(&mut cands.list);
    for (i, item) in list.into_iter().enumerate() {
        if !keep[i] { continue; }
        if let Some(reading) = cands.alt.remove(&i) { alt.insert(next, reading); }
        cands.list.push(item);
        next += 1;
    }
    cands.alt = alt;
}
fn insert_with_alt(cands: &mut Cands, index: usize, item: (String,usize)) {
    cands.list.insert(index, item);
    cands.alt = std::mem::take(&mut cands.alt).into_iter().map(|(i, reading)| (if i >= index { i + 1 } else { i }, reading)).collect();
}
/// The optional digit count table, read as upstream `new_with_packs` reads it (digits.md A.3.2):
/// missing or malformed leaves the digit layer off.
fn windows_digit_table(data: &Path) -> Option<Arc<DigitTable>> {
    std::fs::read_to_string(data.join("digits.tsv")).ok().and_then(|t| DigitTable::parse(&t)).map(Arc::new)
}
#[cfg(test)]
mod personal_lexicon_tests {
    use super::*;
    #[test]
    fn personal_duplicates_replace_only_the_same_pair_and_keep_layer_metadata() {
        let dir = std::env::temp_dir().join(format!("shanjie-personal-overlap-{}",std::process::id()));
        std::fs::create_dir_all(dir.join("packs")).unwrap();
        std::fs::write(dir.join("mcbpmf-data.txt"), "ㄨㄛˇ 我 -8\nㄋㄧˇ 你 -8\n").unwrap();
        std::fs::write(dir.join("overlay-add.tsv"), "ㄏㄠˇ\t好\t-8\tmanual\nㄓㄨˋ\t著\t-8\tmanual\nㄓㄜ˙\t著\t-9\tmanual\n").unwrap();
        std::fs::write(dir.join("sandhi-add.tsv"), "ㄧ\t一\t-8\tsandhi\n").unwrap();
        std::fs::write(dir.join("names-add.tsv"), "ㄌㄧˇ\t李\t-8\tnames\n").unwrap();
        std::fs::write(dir.join("packs/acg-add.tsv"), "ㄇㄧˇ-ㄎㄚ\t米卡\t-8\tacg\n").unwrap();
        std::fs::write(dir.join("packs/it-add.tsv"), "ㄉㄧㄢˋ-ㄋㄠˇ\t電腦\t-8\tit\n").unwrap();
        std::fs::write(dir.join("packs/quotes-add.tsv"), "ㄋㄧˇ-ㄏㄠˇ\t你好\t-8\tquotes\n").unwrap();
        std::fs::write(dir.join("demote.tsv"), "").unwrap();
        let personal = dir.join("dictionary.tsv");
        let rows = "ㄨㄛˇ 我 -1\nㄏㄠˇ 好 -1\nㄧ 一 -1\nㄌㄧˇ 李 -1\nㄇㄧˇ-ㄎㄚ 米卡 -1\nㄉㄧㄢˋ-ㄋㄠˇ 電腦 -1\nㄋㄧˇ-ㄏㄠˇ 你好 -1\nㄓㄨˋ 著 -1\n";
        std::fs::write(&personal, rows).unwrap();
        for packs in 0..=PACK_ALL {
            for _ in 0..2 {
                let e = Engine::new_with_personal(&dir,Layout::Standard,Some(&personal),packs).unwrap();
                for (r,w) in [("ㄨㄛˇ","我"),("ㄏㄠˇ","好"),("ㄧ","一"),("ㄌㄧˇ","李"),("ㄇㄧˇ-ㄎㄚ","米卡"),("ㄉㄧㄢˋ-ㄋㄠˇ","電腦"),("ㄋㄧˇ-ㄏㄠˇ","你好"),("ㄓㄨˋ","著")] {
                    let key: Vec<_> = r.split('-').map(str::to_owned).collect();
                    let entries: Vec<_> = e.lex.entries(&key).into_iter().filter(|item| item.0==w).collect();
                    assert_eq!(entries,[(w,-1.0)],"packs={packs} reading={r}");
                }
                assert!(e.lex.manual().iter().any(|(w,r)| w==['好'] && r==["ㄏㄠˇ"]));
                assert!(e.lex.manual().iter().any(|(w,r)| w==['著'] && r==["ㄓㄜ˙"]));
                if packs & PACK_ACG != 0 { assert!(e.pack_text.as_ref().unwrap().contains("\tacg")); }
            }
        }
        assert_eq!(std::fs::read_to_string(personal).unwrap(),rows);
    }
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
        assert!(Engine::new_with_personal(&dir, Layout::Standard, None, 8).is_err());
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
