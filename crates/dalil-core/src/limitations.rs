//! Method limitations: caveats that hold for every run of an analysis or
//! operation, as opposed to limitations a particular result reached.
//!
//! Reports still carry these texts in full. Human renderers may print one
//! pointer in their place; [`is_method_limitation`] tells them apart.

/// Lexical extraction across the supported languages.
pub const LEXICAL_EXTRACTION: &str = "Definitions and references are extracted lexically with language-specific Tree-sitter queries; only explicit import/module evidence contributes cross-file edges, and types, macros, and runtime behavior are not semantically resolved.";
/// Lexical extraction in a repository whose sources are all Rust.
pub const RUST_LEXICAL_EXTRACTION: &str = "Rust definitions and references are extracted lexically; only explicit call or import evidence contributes file relationships, and types, macros, and runtime behavior are not semantically resolved.";
/// Reference names may match several definitions.
pub const AMBIGUOUS_REFERENCES: &str = "Reference names can have multiple lexical definition candidates; ambiguity is reported rather than treated as a semantic call edge.";
/// Grammar variants and query-pack provenance.
pub const GRAMMAR_VARIANTS: &str = "JavaScript/JSX, TypeScript/TSX, Python, Ruby, Java, C#, Go, Lua, and Zig use explicit grammar variants; query-pack provenance is reported per language.";
/// Tracked files that ignore rules match.
pub const TRACKED_FILE_ELIGIBILITY: &str = "Tracked files are eligible even when ignore rules match them, except deterministic generated/vendor/minified classifications; exact focus paths can opt in within the safety limits.";
/// File ranking.
pub const LEXICAL_RANKING: &str = "Ranking uses deterministic lexical centrality; generic and underscore-prefixed names are downweighted only for ranking and remain available in the full symbol evidence.";
/// Every relationship operation.
pub const RELATIONSHIP_TARGETS: &str = "Relationship targets are derived from retained Tree-sitter symbols and lexical file edges; compiler, type, and runtime resolution is not performed.";
/// Symbol, definition and reference operations.
pub const SYMBOL_EVIDENCE: &str =
    "Definitions and references preserve syntax evidence, including duplicate names and bare references.";
/// Import operations.
pub const IMPORT_EVIDENCE: &str =
    "Imports are returned only when literal import or module evidence contributed a lexical edge.";
/// Dependency and reverse-dependency operations.
pub const DEPENDENCY_EVIDENCE: &str =
    "Dependencies are file-level lexical relationships and can have multiple candidate targets.";
/// Related-test operations.
pub const TEST_EVIDENCE: &str =
    "Related tests are test paths with retained lexical relationships to the requested file or symbol.";
/// Caller and callee operations.
pub const CALL_EVIDENCE: &str = "Call relationships require syntax evidence for a call expression; dynamic dispatch and unresolved names remain uncertain.";
/// Caller and callee candidates.
pub const CALL_CANDIDATES: &str = "Callers and callees use syntax tagged as calls; an ambiguous lexical candidate is not a resolved semantic call relationship.";

/// Every method limitation, in no particular order.
pub const METHOD_LIMITATIONS: &[&str] = &[
    LEXICAL_EXTRACTION,
    RUST_LEXICAL_EXTRACTION,
    AMBIGUOUS_REFERENCES,
    GRAMMAR_VARIANTS,
    TRACKED_FILE_ELIGIBILITY,
    LEXICAL_RANKING,
    RELATIONSHIP_TARGETS,
    SYMBOL_EVIDENCE,
    IMPORT_EVIDENCE,
    DEPENDENCY_EVIDENCE,
    TEST_EVIDENCE,
    CALL_EVIDENCE,
    CALL_CANDIDATES,
];

/// Whether `text` is a method limitation rather than one a result reached.
pub fn is_method_limitation(text: &str) -> bool {
    METHOD_LIMITATIONS.contains(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_limitations_are_recognized_and_result_limitations_are_not() {
        for limitation in METHOD_LIMITATIONS {
            assert!(is_method_limitation(limitation), "{limitation}");
        }
        assert!(!is_method_limitation(
            "The per-file symbol limit (128) was reached; additional unique symbols were omitted."
        ));
        assert!(!is_method_limitation(""));
    }
}
