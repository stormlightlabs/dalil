use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Stable identifier used by the lexical index for one repository file.
///
/// Dalil currently uses normalized repository-relative paths as file IDs. The
/// alias keeps the index independent from the report model while leaving room
/// for a compact numeric ID in a future persistent format.
pub type FileId = String;

/// Three consecutive bytes in normalized source text.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Trigram([u8; 3]);

impl Trigram {
    pub const fn new(bytes: [u8; 3]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 3] {
        self.0
    }

    pub const fn from_bytes(bytes: [u8; 3]) -> Self {
        Self::new(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 3] {
        &self.0
    }
}

/// One file and the byte offsets at which a trigram occurs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Posting {
    pub file: FileId,
    pub positions: Vec<u32>,
}

/// An inverted byte-trigram index with a reverse file-to-trigram index.
///
/// The forward index makes a query candidate lookup proportional to the
/// trigrams in the query rather than the number of indexed files. Positions
/// are retained so callers can verify that the query trigrams occur
/// consecutively instead of accepting a false match based on set overlap.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrigramIndex {
    /// trigram -> every file/position containing it
    postings: HashMap<Trigram, Vec<Posting>>,

    /// file -> trigrams contributed by that file
    ///
    /// Useful for incremental invalidation.
    trigrams_by_file: HashMap<FileId, Vec<Trigram>>,
}

#[derive(Deserialize, Serialize)]
struct StoredTrigramIndex {
    postings: Vec<(Trigram, Vec<Posting>)>,
    trigrams_by_file: Vec<(FileId, Vec<Trigram>)>,
}

impl Serialize for TrigramIndex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut postings = self
            .postings
            .iter()
            .map(|(trigram, entries)| {
                let mut entries = entries.clone();
                entries.sort_by(|left, right| left.file.cmp(&right.file));
                (*trigram, entries)
            })
            .collect::<Vec<_>>();
        postings.sort_by(|left, right| left.0.cmp(&right.0));
        let mut trigrams_by_file = self
            .trigrams_by_file
            .iter()
            .map(|(file, trigrams)| (file.clone(), trigrams.clone()))
            .collect::<Vec<_>>();
        trigrams_by_file.sort_by(|left, right| left.0.cmp(&right.0));
        StoredTrigramIndex { postings, trigrams_by_file }.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TrigramIndex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let stored = StoredTrigramIndex::deserialize(deserializer)?;
        let mut postings = HashMap::new();
        for (trigram, mut entries) in stored.postings {
            for entry in &mut entries {
                entry.positions.sort_unstable();
                entry.positions.dedup();
            }
            entries.sort_by(|left, right| left.file.cmp(&right.file));
            postings.insert(trigram, entries);
        }
        let trigrams_by_file = stored.trigrams_by_file.into_iter().collect();
        Ok(Self { postings, trigrams_by_file })
    }
}

impl TrigramIndex {
    /// Replace all indexed content for `file`.
    pub fn insert(&mut self, file: impl Into<FileId>, content: &[u8]) {
        let file = file.into();
        self.remove(&file);

        let mut trigrams = BTreeSet::new();
        for (position, window) in content.windows(3).enumerate() {
            let Some(position) = u32::try_from(position).ok() else {
                break;
            };
            let trigram = Trigram::new([
                normalize_byte(window[0]),
                normalize_byte(window[1]),
                normalize_byte(window[2]),
            ]);
            trigrams.insert(trigram);
            let postings = self.postings.entry(trigram).or_default();
            if let Some(posting) = postings.iter_mut().find(|posting| posting.file == file) {
                posting.positions.push(position);
            } else {
                postings.push(Posting { file: file.clone(), positions: vec![position] });
            }
        }

        for trigram in &trigrams {
            if let Some(postings) = self.postings.get_mut(trigram) {
                postings.sort_by(|left, right| left.file.cmp(&right.file));
            }
        }
        self.trigrams_by_file.insert(file, trigrams.into_iter().collect());
    }

    /// Alias for callers that describe indexing as adding a file.
    pub fn add_file(&mut self, file: impl Into<FileId>, content: &[u8]) {
        self.insert(file, content);
    }

    /// Alias for callers that name the operation after its file target.
    pub fn index_file(&mut self, file: impl Into<FileId>, content: &[u8]) {
        self.insert(file, content);
    }

    /// Remove all postings contributed by `file`.
    pub fn remove(&mut self, file: &str) -> bool {
        let Some(trigrams) = self.trigrams_by_file.remove(file) else {
            return false;
        };

        for trigram in trigrams {
            let mut remove_trigram = false;
            if let Some(postings) = self.postings.get_mut(&trigram) {
                postings.retain(|posting| posting.file != file);
                remove_trigram = postings.is_empty();
            }
            if remove_trigram {
                self.postings.remove(&trigram);
            }
        }
        true
    }

    /// Alias used by callers that name invalidation after its file target.
    pub fn remove_file(&mut self, file: &str) -> bool {
        self.remove(file)
    }

    /// Alias used by incremental index maintenance.
    pub fn invalidate(&mut self, file: &str) -> bool {
        self.remove(file)
    }

    /// Return postings for one trigram in deterministic file order.
    pub fn postings(&self, trigram: &Trigram) -> Option<&[Posting]> {
        self.postings.get(trigram).map(Vec::as_slice)
    }

    /// Return the trigrams contributed by one file.
    pub fn trigrams_for_file(&self, file: &str) -> Option<&[Trigram]> {
        self.trigrams_by_file.get(file).map(Vec::as_slice)
    }

    /// Return the number of indexed files.
    pub fn file_count(&self) -> usize {
        self.trigrams_by_file.len()
    }

    /// Return the number of distinct indexed trigrams.
    pub fn trigram_count(&self) -> usize {
        self.postings.len()
    }

    /// Find files containing `query` as a contiguous, ASCII-case-insensitive
    /// byte substring.
    ///
    /// Queries shorter than three bytes cannot be answered by a trigram index
    /// without a second shorter-token index, so they return no candidates.
    pub fn search(&self, query: &[u8]) -> Vec<FileId> {
        if query.len() < 3 {
            return Vec::new();
        }

        let query_trigrams = query
            .windows(3)
            .map(|window| {
                Trigram::new([
                    normalize_byte(window[0]),
                    normalize_byte(window[1]),
                    normalize_byte(window[2]),
                ])
            })
            .collect::<Vec<_>>();
        let Some(anchor) = query_trigrams
            .iter()
            .min_by_key(|trigram| self.postings.get(trigram).map_or(0, Vec::len))
        else {
            return Vec::new();
        };
        let Some(anchor_postings) = self.postings.get(anchor) else {
            return Vec::new();
        };

        let mut matches = BTreeSet::new();
        for posting in anchor_postings {
            if posting.positions.iter().any(|&position| {
                query_trigrams.iter().enumerate().all(|(offset, trigram)| {
                    let Some(offset) = u32::try_from(offset).ok() else {
                        return false;
                    };
                    let Some(position) = position.checked_add(offset) else {
                        return false;
                    };
                    self.postings.get(trigram).is_some_and(|postings| {
                        postings
                            .iter()
                            .find(|candidate| candidate.file == posting.file)
                            .is_some_and(|candidate| candidate.positions.binary_search(&position).is_ok())
                    })
                })
            }) {
                matches.insert(posting.file.clone());
            }
        }
        matches.into_iter().collect()
    }

    /// String convenience wrapper around [`Self::search`].
    pub fn search_str(&self, query: &str) -> Vec<FileId> {
        self.search(query.as_bytes())
    }

    /// Alias used by callers that describe a lookup as finding files.
    pub fn files_containing(&self, query: &str) -> Vec<FileId> {
        self.search_str(query)
    }

    /// Remove all indexed files and postings.
    pub fn clear(&mut self) {
        self.postings.clear();
        self.trigrams_by_file.clear();
    }
}

fn normalize_byte(byte: u8) -> u8 {
    byte.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trigram(value: &str) -> Trigram {
        let bytes = value.as_bytes();
        Trigram::new([bytes[0], bytes[1], bytes[2]])
    }

    #[test]
    fn indexes_overlapping_occurrences_and_keeps_positions_sorted() {
        let mut index = TrigramIndex::default();
        index.insert("src/a.rs", b"aaaa");

        assert_eq!(index.trigrams_for_file("src/a.rs"), Some([trigram("aaa")].as_slice()));
        assert_eq!(index.postings(&trigram("aaa")).unwrap()[0].positions, [0, 1]);
        assert_eq!(index.search_str("aaaa"), [String::from("src/a.rs")]);
    }

    #[test]
    fn indexes_case_insensitively_but_reports_original_file_ids() {
        let mut index = TrigramIndex::default();
        index.insert("src/cache.rs", b"CacheStore invalidation");

        assert_eq!(index.search_str("cache"), [String::from("src/cache.rs")]);
        assert_eq!(index.search_str("CACHE"), [String::from("src/cache.rs")]);
    }

    #[test]
    fn requires_consecutive_positions_instead_of_trigram_set_overlap() {
        let mut index = TrigramIndex::default();
        index.insert("src/a.rs", b"abc---abc");

        assert_eq!(index.search_str("abc---"), [String::from("src/a.rs")]);
        assert!(index.search_str("abcabc").is_empty());
    }

    #[test]
    fn replacing_and_invalidating_a_file_removes_old_postings() {
        let mut index = TrigramIndex::default();
        index.insert("src/a.rs", b"old value");
        index.insert("src/a.rs", b"new value");

        assert!(index.search_str("old").is_empty());
        assert_eq!(index.search_str("new"), [String::from("src/a.rs")]);
        assert!(index.invalidate("src/a.rs"));
        assert!(index.search_str("new").is_empty());
        assert_eq!(index.file_count(), 0);
    }

    #[test]
    fn keeps_empty_files_in_the_reverse_index_without_creating_postings() {
        let mut index = TrigramIndex::default();
        index.insert("empty.rs", b"");

        assert_eq!(index.file_count(), 1);
        assert_eq!(index.trigram_count(), 0);
        assert!(index.trigrams_for_file("empty.rs").is_some_and(<[Trigram]>::is_empty));
    }

    #[test]
    fn short_queries_have_no_false_positive_candidates() {
        let mut index = TrigramIndex::default();
        index.insert("src/a.rs", b"ab");

        assert!(index.search_str("ab").is_empty());
    }

    #[test]
    fn serializes_hash_map_keys_as_stable_arrays() {
        let mut index = TrigramIndex::default();
        index.insert("src/a.rs", b"cache");

        let encoded = serde_json::to_value(&index).expect("trigram index serializes");
        let decoded: TrigramIndex = serde_json::from_value(encoded).expect("trigram index deserializes");
        assert_eq!(decoded, index);
    }
}
