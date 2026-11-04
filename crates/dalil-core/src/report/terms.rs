//! Split a plain search query into ranked terms.
//!
//! A query is lowercased and split on non-alphanumeric characters and on
//! identifier boundaries (`camelCase`, `HTTPServer`, `snake_case`). Terms under
//! two characters and common stopwords are dropped. A token that splits into
//! more than one part, such as `CacheStore` or `src/one.rs`, is also kept whole
//! as an extra term. Text inside double quotes stays one exact phrase. Terms
//! under three bytes match paths and names only on word boundaries.

/// The most terms one query keeps, so matching stays bounded.
pub(crate) const MAX_QUERY_TERMS: usize = 32;

const MIN_TERM_CHARS: usize = 2;

const STOPWORDS: &[&str] = &[
    "a", "about", "after", "all", "also", "an", "and", "any", "are", "as", "at", "be", "been", "before", "but", "by",
    "can", "do", "does", "for", "from", "has", "have", "how", "if", "in", "into", "is", "it", "its", "of", "on", "or",
    "so", "than", "that", "the", "their", "them", "then", "there", "these", "this", "those", "to", "via", "was",
    "were", "what", "when", "where", "which", "while", "who", "why", "will", "with", "without",
];

/// Conventional Commits types, dropped when a query starts with `type:` or
/// `type(scope):` so a pasted commit subject searches for its words.
const COMMIT_TYPES: &[&str] = &[
    "build", "chore", "ci", "docs", "feat", "fix", "perf", "refactor", "revert", "style", "test",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TermKind {
    /// One lowercase word.
    Word,
    /// A whole token that also split into words, such as `cachestore`.
    Identifier,
    /// Quoted text, matched as one contiguous phrase.
    Phrase,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QueryTerm {
    pub(crate) text: String,
    pub(crate) kind: TermKind,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct QueryTerms {
    pub(crate) terms: Vec<QueryTerm>,
    /// The unquoted query as one phrase, when it holds more than one word.
    /// A result that contains it contiguously gets the exact-phrase bonus.
    pub(crate) phrase: Option<String>,
}

impl QueryTerms {
    pub(crate) fn parse(query: &str) -> Self {
        let query = strip_commit_type(query.trim());
        let mut terms = Vec::<QueryTerm>::new();
        let mut push = |text: String, kind: TermKind| {
            if terms.len() < MAX_QUERY_TERMS && !terms.iter().any(|term| term.text == text) {
                terms.push(QueryTerm { text, kind });
            }
        };
        let mut rest = query;
        while let Some(start) = rest.find('"') {
            let (before, after) = rest.split_at(start);
            let after = &after[1..];
            let Some(end) = after.find('"') else {
                break;
            };
            split_words(before, &mut push);
            let phrase = after[..end]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            if phrase.chars().count() >= MIN_TERM_CHARS {
                push(phrase, TermKind::Phrase);
            }
            rest = &after[end + 1..];
        }
        // An unbalanced quote is a separator like any other punctuation.
        split_words(rest, &mut push);

        let tokens = query.split_whitespace().collect::<Vec<_>>();
        let phrase = (!query.contains('"') && tokens.len() > 1).then(|| tokens.join(" ").to_lowercase());
        Self { terms, phrase }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }
}

/// Drop a leading Conventional Commits `type:`, `type!:` or `type(scope):`
/// prefix. The colon must end the query or be followed by whitespace, so a
/// path such as `test::helpers` keeps its first segment. A scoped prefix is
/// left for `strip_commit_scope`, which keeps the scope as a term.
fn strip_commit_type(query: &str) -> &str {
    let kind_len = query.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(query.len());
    let (kind, after) = query.split_at(kind_len);
    if !COMMIT_TYPES.contains(&kind.to_ascii_lowercase().as_str()) {
        return query;
    }
    let scoped = after.starts_with('(');
    let after = if scoped {
        match after.find(')') {
            Some(end) if !after[1..end].contains(['(', ':']) => &after[end + 1..],
            _ => return query,
        }
    } else {
        after
    };
    let after = after.strip_prefix('!').unwrap_or(after);
    let Some(rest) = after.strip_prefix(':') else {
        return query;
    };
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return query;
    }
    if scoped { query } else { rest.trim_start() }
}

/// Terms shorter than this many bytes have no trigrams, so they skip content
/// and match paths and names only as whole words or identifier parts.
pub(crate) const MIN_SUBSTRING_TERM_BYTES: usize = 3;

/// A path or symbol name prepared for term matching.
pub(crate) struct NameText {
    lower: String,
    parts: Vec<String>,
}

impl NameText {
    pub(crate) fn new(text: &str) -> Self {
        Self { lower: text.to_lowercase(), parts: identifier_parts(text) }
    }

    /// Whether `term` occurs in the text. A term of at least
    /// `MIN_SUBSTRING_TERM_BYTES` matches anywhere; a shorter one only as a
    /// whole word or identifier part, so `go` matches `go.mod`, `cmd/go/` and
    /// `GoParser` but not `Cargo.toml`.
    pub(crate) fn contains(&self, term: &str) -> bool {
        if term.len() < MIN_SUBSTRING_TERM_BYTES {
            self.parts.iter().any(|part| part == term)
        } else {
            self.lower.contains(term)
        }
    }

    pub(crate) fn lower(&self) -> &str {
        &self.lower
    }
}

fn split_words(text: &str, push: &mut impl FnMut(String, TermKind)) {
    for token in text.split_whitespace() {
        let token = strip_commit_scope(token);
        let parts = identifier_parts(token);
        let whole = token.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        for part in &parts {
            if part.chars().count() >= MIN_TERM_CHARS && !STOPWORDS.contains(&part.as_str()) {
                push(part.clone(), TermKind::Word);
            }
        }
        if parts.len() > 1 && whole.chars().count() >= MIN_TERM_CHARS {
            push(whole, TermKind::Identifier);
        }
    }
}

/// Turn a leading `type(scope):` token into its scope.
fn strip_commit_scope(token: &str) -> &str {
    let Some(head) = token
        .strip_suffix(':')
        .map(|head| head.strip_suffix('!').unwrap_or(head))
    else {
        return token;
    };
    match head.split_once('(') {
        Some((kind, scope)) if COMMIT_TYPES.contains(&kind.to_ascii_lowercase().as_str()) && scope.ends_with(')') => {
            scope.trim_end_matches(')')
        }
        _ => token,
    }
}

/// Lowercase parts of one token, split on non-alphanumerics and on case
/// boundaries: `parseHTTPResponse` gives `parse`, `http`, `response`.
fn identifier_parts(token: &str) -> Vec<String> {
    let mut parts = Vec::new();
    for run in token.split(|c: char| !c.is_alphanumeric()) {
        let chars = run.chars().collect::<Vec<_>>();
        let mut current = String::new();
        for (index, &c) in chars.iter().enumerate() {
            let previous = index.checked_sub(1).map(|index| chars[index]);
            let next = chars.get(index + 1).copied();
            let boundary = c.is_uppercase()
                && previous.is_some_and(|previous| {
                    previous.is_lowercase()
                        || previous.is_numeric()
                        || (previous.is_uppercase() && next.is_some_and(char::is_lowercase))
                });
            if boundary && !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
            current.extend(c.to_lowercase());
        }
        if !current.is_empty() {
            parts.push(current);
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(query: &str) -> Vec<String> {
        QueryTerms::parse(query)
            .terms
            .into_iter()
            .map(|term| term.text)
            .collect()
    }

    #[test]
    fn splits_words_and_drops_short_terms_and_stopwords() {
        assert_eq!(
            texts("zig language support for the parser"),
            ["zig", "language", "support", "parser"]
        );
        assert_eq!(texts("a b of go"), ["go"]);
        assert_eq!(texts("Cache cache CACHE"), ["cache"]);
    }

    #[test]
    fn splits_identifiers_and_keeps_them_whole() {
        assert_eq!(texts("CacheStore"), ["cache", "store", "cachestore"]);
        assert_eq!(
            texts("parseHTTPResponse"),
            ["parse", "http", "response", "parsehttpresponse"]
        );
        assert_eq!(texts("max_file_bytes"), ["max", "file", "bytes", "max_file_bytes"]);
        assert_eq!(texts("src/one.rs"), ["src", "one", "rs", "src/one.rs"]);
        assert_eq!(texts("utf8Decode"), ["utf8", "decode", "utf8decode"]);
    }

    #[test]
    fn quoted_text_is_one_phrase() {
        let terms = QueryTerms::parse("\"cache invalidation\" parser");
        assert_eq!(
            terms.terms,
            [
                QueryTerm { text: "cache invalidation".to_owned(), kind: TermKind::Phrase },
                QueryTerm { text: "parser".to_owned(), kind: TermKind::Word },
            ]
        );
        assert_eq!(terms.phrase, None);
        assert_eq!(texts("\"unbalanced quote"), ["unbalanced", "quote"]);
    }

    #[test]
    fn multi_word_queries_carry_the_whole_phrase() {
        assert_eq!(
            QueryTerms::parse("Cache  Invalidation").phrase.as_deref(),
            Some("cache invalidation")
        );
        assert_eq!(QueryTerms::parse("invalidation").phrase, None);
    }

    #[test]
    fn commit_types_are_dropped_and_scopes_kept() {
        assert_eq!(texts("feat: split search queries"), ["split", "search", "queries"]);
        assert_eq!(texts("fix(cli)!: read path"), ["cli", "read", "path"]);
        assert_eq!(texts("Note: keep"), ["note", "keep"]);
        assert_eq!(texts("refactor!: drop"), ["drop"]);
        assert_eq!(texts("fix(cli):"), ["cli"]);
        assert_eq!(texts("docs:"), Vec::<String>::new());
    }

    #[test]
    fn rust_paths_starting_with_a_commit_type_keep_every_segment() {
        assert_eq!(texts("test::helpers"), ["test", "helpers", "test::helpers"]);
        assert_eq!(texts("build::Config"), ["build", "config", "build::config"]);
        assert_eq!(texts("fix:value"), ["fix", "value", "fix:value"]);
        assert_eq!(texts("fix(cli)::run"), ["fix", "cli", "run", "fix(cli)::run"]);
    }

    #[test]
    fn the_phrase_needs_more_than_one_token() {
        assert_eq!(QueryTerms::parse("HTTPClient").phrase, None);
        assert_eq!(QueryTerms::parse("src/one.rs").phrase, None);
        assert_eq!(
            QueryTerms::parse("HTTPClient retry").phrase.as_deref(),
            Some("httpclient retry")
        );
    }

    #[test]
    fn short_terms_match_names_only_on_word_boundaries() {
        for name in ["go.mod", "cmd/go/main.rs", "GoParser", "go_parser"] {
            assert!(NameText::new(name).contains("go"), "{name}");
        }
        for name in ["Cargo.toml", "src/ergonomic.rs", "Gopher"] {
            assert!(!NameText::new(name).contains("go"), "{name}");
        }
        assert!(NameText::new("Cargo.toml").contains("arg"));
    }

    #[test]
    fn term_count_is_bounded() {
        let query = (0..100)
            .map(|index| format!("word{index}x"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(texts(&query).len(), MAX_QUERY_TERMS);
    }

    #[test]
    fn empty_and_punctuation_queries_have_no_terms() {
        assert!(QueryTerms::parse("").is_empty());
        assert!(QueryTerms::parse("-- / !!").is_empty());
    }
}
