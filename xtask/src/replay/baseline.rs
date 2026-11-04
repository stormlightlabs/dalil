//! Keyword search plus path ranking, without Dalil.
//!
//! The subject loses any Conventional Commits prefix (`fix(scope):`), is lowercased and split on
//! non-alphanumeric characters. Words shorter than three characters and stopwords are dropped. Each file
//! scores `PATH_WEIGHT` for every word its lowercased path contains, plus the number of times each word
//! occurs in its lowercased content, capped at `CONTENT_CAP` per word. Binary files and files over
//! `MAX_CONTENT_BYTES` score on their path only. Files that score zero are left out; ties are ordered
//! by path.

pub const PATH_WEIGHT: u64 = 10;
pub const CONTENT_CAP: u64 = 10;
pub const MAX_CONTENT_BYTES: usize = 1 << 20;

const STOPWORDS: &[&str] = &[
    "about", "add", "added", "adds", "after", "all", "also", "and", "any", "are", "back", "before", "but", "can",
    "change", "changes", "does", "don", "each", "for", "from", "has", "have", "into", "its", "make", "more", "new",
    "not", "now", "one", "only", "out", "remove", "removed", "same", "set", "should", "some", "than", "that", "the",
    "their", "them", "then", "there", "these", "this", "use", "used", "uses", "using", "via", "was", "were", "what",
    "when", "which", "while", "will", "with", "without", "you", "update", "updated", "fix", "fixed", "fixes",
];

/// The search words in `subject`, in first-seen order.
pub fn tokens(subject: &str) -> Vec<String> {
    let mut words = Vec::new();
    for word in strip_conventional_prefix(subject)
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
    {
        if word.chars().count() >= 3 && !STOPWORDS.contains(&word) && !words.iter().any(|seen| seen == word) {
            words.push(word.to_owned());
        }
    }
    words
}

fn strip_conventional_prefix(subject: &str) -> &str {
    let Some((head, rest)) = subject.split_once(':') else {
        return subject;
    };
    let head = head.strip_suffix('!').unwrap_or(head);
    let kind = match head.split_once('(') {
        Some((kind, scope)) if scope.ends_with(')') => kind,
        Some(_) => return subject,
        None => head,
    };
    if !kind.is_empty() && kind.chars().all(|c| c.is_ascii_alphabetic()) {
        rest.trim_start()
    } else {
        subject
    }
}

/// Score one file. `content` is `None` when the file could not be read.
pub fn score(path: &str, content: Option<&[u8]>, words: &[String]) -> u64 {
    let path = path.to_lowercase();
    let content = content
        .filter(|bytes| bytes.len() <= MAX_CONTENT_BYTES && !bytes.iter().take(8000).any(|byte| *byte == 0))
        .map(|bytes| bytes.to_ascii_lowercase());
    words
        .iter()
        .map(|word| {
            let in_path = if path.contains(word.as_str()) { PATH_WEIGHT } else { 0 };
            let in_content = content
                .as_deref()
                .map_or(0, |bytes| occurrences(bytes, word.as_bytes()));
            in_path + in_content.min(CONTENT_CAP)
        })
        .sum()
}

fn occurrences(haystack: &[u8], needle: &[u8]) -> u64 {
    if needle.is_empty() || haystack.len() < needle.len() {
        return 0;
    }
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count() as u64
}

/// Rank scored files, highest first, dropping zero scores.
pub fn rank(mut scored: Vec<(String, u64)>) -> Vec<String> {
    scored.retain(|(_, score)| *score > 0);
    scored.sort_by(|(left_path, left), (right_path, right)| right.cmp(left).then_with(|| left_path.cmp(right_path)));
    scored.into_iter().map(|(path, _)| path).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_drop_prefix_short_words_stopwords_and_repeats() {
        assert_eq!(
            tokens("fix(cli): Read the path qualifiers outside Rust path"),
            ["read", "path", "qualifiers", "outside", "rust"]
        );
        assert_eq!(tokens("feat!: tag callee positions"), ["tag", "callee", "positions"]);
        assert_eq!(
            tokens("Move method limitations out of human output"),
            ["move", "method", "limitations", "human", "output"]
        );
        assert_eq!(tokens("Note: keep this"), ["keep"]);
        assert_eq!(tokens("see http://x.io: a"), ["see", "http"]);
    }

    #[test]
    fn path_hits_outweigh_content_hits() {
        let words = tokens("parser cache");
        assert_eq!(score("src/parser.rs", Some(b"fn parse() {}"), &words), PATH_WEIGHT);
        assert_eq!(score("src/lib.rs", Some(b"cache cache Cache"), &words), 3);
        let many = "cache ".repeat(50);
        assert_eq!(score("src/lib.rs", Some(many.as_bytes()), &words), CONTENT_CAP);
        assert_eq!(score("src/cache.bin", Some(b"cache\0cache"), &words), PATH_WEIGHT);
        assert_eq!(score("README.md", None, &words), 0);
    }

    #[test]
    fn rank_orders_by_score_then_path_and_drops_zero() {
        let ranked = rank(vec![
            ("b.rs".to_owned(), 3),
            ("z.rs".to_owned(), 0),
            ("a.rs".to_owned(), 3),
            ("c.rs".to_owned(), 12),
        ]);
        assert_eq!(ranked, ["c.rs", "a.rs", "b.rs"]);
    }
}
