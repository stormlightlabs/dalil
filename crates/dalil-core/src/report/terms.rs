//! Split a plain search query into ranked terms.
//!
//! A query is lowercased and split on non-alphanumeric characters and on
//! identifier boundaries (`camelCase`, `HTTPServer`, `snake_case`). Terms under
//! two characters and common stopwords are dropped. A token that splits into
//! more than one part, such as `CacheStore` or `src/one.rs`, is also kept whole
//! as an extra term. Text inside double quotes stays one exact phrase.

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
        let mut unquoted = String::new();
        let mut rest = query;
        while let Some(start) = rest.find('"') {
            let (before, after) = rest.split_at(start);
            let after = &after[1..];
            let Some(end) = after.find('"') else {
                break;
            };
            unquoted.push_str(before);
            unquoted.push(' ');
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
        unquoted.push_str(rest);
        split_words(rest, &mut push);

        let has_quotes = query.contains('"');
        let words = terms.iter().filter(|term| term.kind == TermKind::Word).count();
        let phrase = (!has_quotes && words > 1)
            .then(|| unquoted.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase());
        Self { terms, phrase }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }
}

fn strip_commit_type(query: &str) -> &str {
    let Some((head, rest)) = query.split_once(':') else {
        return query;
    };
    let head = head.strip_suffix('!').unwrap_or(head);
    let kind = match head.split_once('(') {
        Some((kind, scope)) if scope.ends_with(')') => kind,
        Some(_) => return query,
        None => head,
    };
    if COMMIT_TYPES.contains(&kind.to_ascii_lowercase().as_str()) {
        // Keep the scope: it usually names the area the change touches.
        let scope = head
            .split_once('(')
            .map_or("", |(_, scope)| scope.trim_end_matches(')'));
        if scope.is_empty() {
            return rest.trim_start();
        }
        return query;
    }
    query
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
