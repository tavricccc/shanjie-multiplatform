
/// The words of a set as one sorted string: about 390,000 short words as separate `String`s cost 18 MiB more.
struct WordSet {
    text: Box<str>,
    /// `text[ends[i]..ends[i + 1]]` is word `i`, in byte order.
    ends: Box<[u32]>,
}

impl WordSet {
    fn new(words: HashSet<String>) -> WordSet {
        let mut sorted: Vec<&str> = words.iter().map(String::as_str).collect();
        sorted.sort_unstable();
        let mut text = String::with_capacity(sorted.iter().map(|w| w.len()).sum());
        let mut ends = Vec::with_capacity(sorted.len() + 1);
        ends.push(0);
        for word in sorted {
            text.push_str(word);
            ends.push(text.len() as u32);
        }
        WordSet { text: text.into_boxed_str(), ends: ends.into_boxed_slice() }
    }

    fn contains(&self, word: &str) -> bool {
        let (mut low, mut high) = (0, self.ends.len() - 1);
        while low < high {
            let middle = (low + high) / 2;
            match self.text[self.ends[middle] as usize..self.ends[middle + 1] as usize].cmp(word) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return true,
            }
        }
        false
    }
}

#[cfg(test)]
mod word_set_tests {
    use super::*;

    #[test]
    fn holds_exactly_the_words_it_was_built_from() {
        let words = ["字", "字詞", "詞", "a", "ab", "", "\u{20000}"];
        let set = WordSet::new(words.iter().map(|w| w.to_string()).collect());
        for word in words {
            assert!(set.contains(word), "{word}");
        }
        for word in ["字詞語", "b", "詞字", "abc", "\u{20001}"] {
            assert!(!set.contains(word), "{word}");
        }
        assert!(!WordSet::new(HashSet::new()).contains("字"));
    }
}
