// Included in learn.rs: undo only mutations made by one provisional Windows selection.
enum WindowsLearningUndo {
    Updated(usize, Record),
    Appended,
    Removed(usize, Record),
}

impl Learner {
    pub(crate) fn windows_preview_active(&self) -> bool { self.windows_undo.is_some() }

    pub(crate) fn windows_begin_preview(&mut self) {
        debug_assert!(self.windows_undo.is_none());
        self.windows_undo = Some(Vec::new());
    }

    fn windows_remember(&mut self, i: usize) {
        if let Some(undo) = &mut self.windows_undo {
            undo.push(WindowsLearningUndo::Updated(i, self.records[i].clone()));
        }
    }

    pub(crate) fn windows_rollback_preview(&mut self) -> bool {
        let Some(undo) = self.windows_undo.take() else { return false };
        for change in undo.into_iter().rev() {
            match change {
                WindowsLearningUndo::Updated(i, record) => self.records[i] = record,
                WindowsLearningUndo::Appended => self.remove_at(vec![self.records.len() - 1]),
                WindowsLearningUndo::Removed(i, record) => {
                    let last = self.records.len();
                    let key = reading_key(&record.reading);
                    self.new_count += (record.context == NEWWORD) as usize;
                    if i == last {
                        self.records.push(record);
                    } else {
                        let moved = std::mem::replace(&mut self.records[i], record);
                        let bucket = self.index.get_mut(&reading_key(&moved.reading)).expect("indexed");
                        *bucket.iter_mut().find(|j| **j == i).expect("indexed") = last;
                        self.records.push(moved);
                    }
                    self.index.entry(key).or_default().push(i);
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod windows_selection_journal_tests {
    use super::*;

    #[test]
    fn provisional_selection_updates_only_touched_records_in_a_full_store() {
        let records = (0..CAPACITY).map(|i| Record {
            context: if i == 0 { "字字".into() } else { format!("字{i}") }, reading: vec!["ㄋㄧˇ".into()],
            word: "妳".into(), weight: 1.0, day: 20_000,
        }).collect();
        let mut learner = Learner::from_records(records);
        let before = learner.records.clone();
        let bucket = learner.index["ㄋㄧˇ"].as_ptr();
        learner.windows_begin_preview();
        learner.teach("字字", &["ㄋㄧˇ".into()], "你", "妳", 20_000);
        assert!((1..=3).contains(&learner.windows_undo.as_ref().unwrap().len()));
        learner.windows_rollback_preview();
        assert!(learner.records == before);
        assert_eq!(learner.index["ㄋㄧˇ"].as_ptr(), bucket, "rollback must not rebuild the index");
        assert!(!learner.windows_rollback_preview());
    }

    #[test]
    fn rollback_restores_evicted_new_words_and_their_index_positions() {
        for initial_weight in [0.5, 1.0] {
            let records = (0..NEWWORD_CAP).map(|i| Record {
                context: NEWWORD.into(), reading: vec![format!("reading{i}")],
                word: format!("word{i}"), weight: initial_weight, day: 20_000,
            }).collect();
            let mut learner = Learner::from_records(records);
            let before = learner.records.clone();
            learner.windows_begin_preview();
            learner.learn_new(&["new".into()], "new", 20_000);
            learner.halve_new(&["reading0".into()], "word0", 20_000);
            learner.windows_rollback_preview();
            assert!(learner.records == before);
            assert_eq!(learner.new_count, NEWWORD_CAP);
            for (i, record) in learner.records.iter().enumerate() {
                assert_eq!(learner.find(&record.context, &record.reading, &record.word), Some(i));
            }
        }
    }
}
