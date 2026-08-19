use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::utils::token_count;

const DEFAULT_RESULT_LIMIT: usize = 5;
const MAX_RESULT_LIMIT: usize = 12;

#[derive(Clone)]
struct Candidate {
    target: SearchTarget,
    path: String,
    symbol: Option<SourceSymbol>,
    purpose: ReadingPurpose,
    reason: String,
    evidence_kinds: BTreeSet<ReadingEvidenceKind>,
    confidence: ConfidenceTier,
    limitations: BTreeSet<String>,
    score: u64,
    anchor: bool,
}

/// Compile a small, deterministic set of source-reading anchors from a typed
/// query. Search consumes map evidence but does not return its graph or expose
/// graph traversal as a public mode.
pub fn compile(request: SearchRequest, map: &MapReport) -> SearchResults {
    let request = normalized_request(request, map);
    let mut candidates = BTreeMap::<String, Candidate>::new();

    if request.query.is_empty() {
        return SearchResults {
            budget: SearchBudget {
                token_budget: request.budget,
                result_limit: request.result_limit,
                ..SearchBudget::default()
            },
            limitations: vec!["Search requires a non-empty query.".to_owned()],
            request,
            ..SearchResults::default()
        };
    }

    for landmark in &map.landmarks {
        if request.mode == SearchQueryMode::Plain
            && let Some(score) = text_match_score(&request.query, &landmark.path)
        {
            add_candidate(
                &mut candidates,
                Candidate {
                    target: SearchTarget::Path,
                    path: landmark.path.clone(),
                    symbol: None,
                    purpose: ReadingPurpose::StartHere,
                    reason: format!("the query matches this {} path", landmark.kind.label()),
                    evidence_kinds: [ReadingEvidenceKind::Landmark].into_iter().collect(),
                    confidence: ConfidenceTier::High,
                    limitations: [
                        "Path-name evidence does not establish source behavior or symbol ownership.".to_owned(),
                    ]
                    .into_iter()
                    .collect(),
                    score: score + u64::from(landmark.priority) * 1_000_000,
                    anchor: false,
                },
            );
        }
    }

    let fallback_sources;
    let sources = if map.reading_evidence.sources.is_empty() {
        fallback_sources = map
            .files
            .iter()
            .map(|file| ReadingSourceEvidence {
                path: file.path.clone(),
                language: file.language,
                worktree_state: file.worktree_state,
                status: file.status,
                symbols: file.symbols.clone(),
                limitations: file.limitations.clone(),
            })
            .collect::<Vec<_>>();
        &fallback_sources
    } else {
        &map.reading_evidence.sources
    };
    let ranking = if map.reading_evidence.ranking.is_empty() { &map.ranking } else { &map.reading_evidence.ranking };
    let rank_by_path = ranking
        .iter()
        .map(|rank| (rank.path.as_str(), rank))
        .collect::<BTreeMap<_, _>>();
    for source in sources {
        let ranking_bonus = rank_by_path
            .get(source.path.as_str())
            .map_or(0, |rank| rank.score / 1_000);
        if request.mode == SearchQueryMode::Plain
            && let Some(score) = text_match_score(&request.query, &source.path)
        {
            add_candidate(
                &mut candidates,
                source_candidate(
                    source,
                    SearchTarget::Path,
                    None,
                    score + ranking_bonus,
                    format!("the query matches source path `{}`", source.path),
                    ConfidenceTier::High,
                ),
            );
        }
        for symbol in &source.symbols {
            let Some(score) = symbol_match_score(&request, symbol) else {
                continue;
            };
            let confidence =
                if symbol.role == SymbolRole::Definition { ConfidenceTier::High } else { ConfidenceTier::Medium };
            let match_kind = match request.mode {
                SearchQueryMode::Plain => "the query matches",
                SearchQueryMode::Symbol => "the explicit symbol query matches",
            };
            add_candidate(
                &mut candidates,
                source_candidate(
                    source,
                    SearchTarget::Symbol,
                    Some(symbol.clone()),
                    score + ranking_bonus + symbol_role_bonus(symbol),
                    format!("{match_kind} {} `{}`", symbol.kind.label(), symbol.name),
                    confidence,
                ),
            );
        }
    }

    let mut total_candidates = candidates.len();
    let mut results = SearchResults {
        request,
        budget: SearchBudget {
            token_budget: 0,
            result_limit: 0,
            total_candidates,
            returned: 0,
            estimated_tokens: 0,
            truncated: false,
        },
        ..SearchResults::default()
    };
    results.budget.token_budget = results.request.budget;
    results.budget.result_limit = results.request.result_limit;

    let candidates = sorted_candidates(candidates.into_values());
    for candidate in candidates {
        if results.matches.len() == results.request.result_limit {
            break;
        }
        add_if_fits(&mut results, candidate);
    }

    if results.matches.len() < results.request.result_limit {
        if let Some(anchor) = related_anchor(&results.matches, sources, map) {
            total_candidates += 1;
            add_if_fits(&mut results, anchor);
        }
    }

    results.budget.total_candidates = total_candidates;
    results.budget.returned = results.matches.len();
    results.budget.estimated_tokens = estimate_tokens(&results);
    results.budget.truncated =
        results.matches.len() < total_candidates || results.budget.estimated_tokens > results.request.budget;
    results.shortfall = search_shortfall(&results, total_candidates);
    results.limitations = limitations(map, &results);
    results
}

fn normalized_request(mut request: SearchRequest, map: &MapReport) -> SearchRequest {
    request.repository = map.repository_root.clone();
    request.query = request.query.trim().to_owned();
    if request.result_limit == 0 {
        request.result_limit = DEFAULT_RESULT_LIMIT;
    }
    request.result_limit = request.result_limit.min(MAX_RESULT_LIMIT);
    if request.budget == 0 {
        request.budget = map.selection.token_budget.max(1);
    }
    request.profile = map.profile;
    request
}

fn source_candidate(
    source: &ReadingSourceEvidence, target: SearchTarget, symbol: Option<SourceSymbol>, score: u64, reason: String,
    confidence: ConfidenceTier,
) -> Candidate {
    let mut limitations = source.limitations.iter().cloned().collect::<BTreeSet<_>>();
    limitations
        .insert("Search matches are lexical source evidence; inspect the source before relying on it.".to_owned());
    Candidate {
        target,
        path: source.path.clone(),
        symbol,
        purpose: if is_test_path(&source.path) { ReadingPurpose::Tests } else { ReadingPurpose::Architecture },
        reason,
        evidence_kinds: [ReadingEvidenceKind::SourceMap].into_iter().collect(),
        confidence,
        limitations,
        score,
        anchor: false,
    }
}

fn add_candidate(candidates: &mut BTreeMap<String, Candidate>, candidate: Candidate) {
    let key = candidate.path.clone();
    let replace = candidates.get(&key).is_none_or(|current| {
        candidate.score > current.score
            || (candidate.score == current.score
                && candidate.target == SearchTarget::Symbol
                && current.target == SearchTarget::Path)
    });
    if replace {
        candidates.insert(key, candidate);
    }
}

fn sorted_candidates(candidates: impl IntoIterator<Item = Candidate>) -> Vec<Candidate> {
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.target.label().cmp(right.target.label()))
    });
    candidates
}

fn symbol_match_score(request: &SearchRequest, symbol: &SourceSymbol) -> Option<u64> {
    let name = symbol.name.to_ascii_lowercase();
    let query = request.query.to_ascii_lowercase();
    match request.mode {
        SearchQueryMode::Symbol => (name == query).then_some(5_000_000_000),
        SearchQueryMode::Plain => text_match_score(&request.query, &symbol.name)
            .or_else(|| text_match_score(&request.query, &symbol.context).map(|score| score / 2)),
    }
}

fn text_match_score(query: &str, subject: &str) -> Option<u64> {
    let query = query.trim().to_ascii_lowercase();
    let subject = subject.to_ascii_lowercase();
    if query.is_empty() || subject.is_empty() {
        return None;
    }
    if subject == query {
        return Some(4_000_000_000);
    }
    if subject.contains(&query) {
        return Some(3_000_000_000);
    }
    let terms = query_terms(&query);
    (!terms.is_empty() && terms.iter().all(|term| subject.contains(term)))
        .then_some(1_000_000_000 + terms.len() as u64 * 10_000_000)
}

fn query_terms(query: &str) -> Vec<String> {
    let mut terms = query
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| term.len() > 1)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    terms.sort();
    terms.dedup();
    terms
}

fn symbol_role_bonus(symbol: &SourceSymbol) -> u64 {
    match symbol.role {
        SymbolRole::Definition => 500_000_000,
        SymbolRole::Reference => 0,
    }
}

fn add_if_fits(results: &mut SearchResults, candidate: Candidate) {
    let ordinal = results.matches.len() + 1;
    let candidate = SearchMatch {
        recommendation: ReadingRecommendation {
            ordinal,
            purpose: candidate.purpose,
            path: candidate.path,
            project_root: None,
            reason: candidate.reason,
            evidence_kinds: candidate.evidence_kinds.into_iter().collect(),
            confidence: candidate.confidence,
            limitations: candidate.limitations.into_iter().collect(),
        },
        target: candidate.target,
        symbol: candidate.symbol,
        score: candidate.score,
        anchor: candidate.anchor,
    };
    let before = results.clone();
    results.matches.push(candidate);
    if estimate_tokens(results) > results.request.budget {
        *results = before;
    }
}

fn related_anchor(matches: &[SearchMatch], sources: &[ReadingSourceEvidence], map: &MapReport) -> Option<Candidate> {
    let selected = matches
        .iter()
        .map(|result| result.recommendation.path.as_str())
        .collect::<BTreeSet<_>>();
    let sources = sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let edges = if map.reading_evidence.graph.is_empty() {
        map.edges.iter().collect::<Vec<_>>()
    } else {
        map.reading_evidence
            .graph
            .iter()
            .map(|edge| &edge.relationship)
            .collect()
    };
    for edge in edges {
        let other = if selected.contains(edge.source.as_str()) && !selected.contains(edge.target.as_str()) {
            edge.target.as_str()
        } else if selected.contains(edge.target.as_str()) && !selected.contains(edge.source.as_str()) {
            edge.source.as_str()
        } else {
            continue;
        };
        let Some(source) = sources.get(other) else {
            continue;
        };
        let mut candidate = source_candidate(
            source,
            SearchTarget::Path,
            None,
            500_000_000,
            format!(
                "a retained lexical relationship via `{}` connects this file to a direct search match",
                edge.symbol
            ),
            edge.confidence,
        );
        candidate.purpose = if is_test_path(other) { ReadingPurpose::Tests } else { ReadingPurpose::SupportingContext };
        candidate.evidence_kinds.insert(ReadingEvidenceKind::Graph);
        candidate
            .limitations
            .insert("This is lexical relationship evidence, not a proven caller or callee.".to_owned());
        if edge.ambiguous {
            candidate
                .limitations
                .insert("The retained lexical relationship has multiple candidate targets.".to_owned());
        }
        candidate.anchor = true;
        return Some(candidate);
    }
    None
}

fn is_test_path(path: &str) -> bool {
    path.starts_with("test/")
        || path.starts_with("tests/")
        || path.contains("/test/")
        || path.contains("/tests/")
        || path.ends_with("_test.go")
        || path.ends_with("_test.rs")
        || path.ends_with(".test.js")
        || path.ends_with(".test.ts")
        || path.ends_with(".spec.js")
        || path.ends_with(".spec.ts")
}

fn estimate_tokens(results: &SearchResults) -> usize {
    serde_json::to_string(&results.matches).map_or(usize::MAX, |json| token_count(&json))
}

fn search_shortfall(results: &SearchResults, total_candidates: usize) -> Option<SearchShortfall> {
    if total_candidates == 0 {
        return Some(SearchShortfall {
            requested: results.request.result_limit,
            returned: 0,
            reason: format!(
                "No strong {} matches were found for `{}`.",
                results.request.mode.label(),
                results.request.query
            ),
        });
    }
    (results.matches.len() < results.request.result_limit).then(|| SearchShortfall {
        requested: results.request.result_limit,
        returned: results.matches.len(),
        reason: if results.matches.len() < total_candidates {
            "The shared token budget stopped before every strong match could be returned.".to_owned()
        } else {
            "Fewer strong matches exist than the requested result limit.".to_owned()
        },
    })
}

fn limitations(map: &MapReport, results: &SearchResults) -> Vec<String> {
    let mut limitations = Vec::new();
    if results.request.mode == SearchQueryMode::Symbol {
        limitations.push(
            "Symbol matches come from retained syntax evidence and do not prove compiler-level resolution.".to_owned(),
        );
    }
    if map.collections.symbols.truncated {
        limitations.push("The source-symbol collection was truncated before search selection.".to_owned());
    }
    if map.collections.files.truncated {
        limitations.push("The source-file collection was truncated before search selection.".to_owned());
    }
    if map.availability.unsupported_paths > 0 {
        limitations.push(format!(
            "{} unsupported source path(s) were outside the retained syntax evidence.",
            map.availability.unsupported_paths
        ));
    }
    limitations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_terms_ignore_one_character_fragments_and_are_deterministic() {
        assert_eq!(query_terms("cache, parser cache a"), ["cache", "parser"]);
    }

    #[test]
    fn explicit_symbol_queries_match_only_exact_names() {
        let request =
            SearchRequest { query: "CacheStore".to_owned(), mode: SearchQueryMode::Symbol, ..SearchRequest::default() };
        let symbol = SourceSymbol {
            name: "CacheStore".to_owned(),
            kind: SymbolKind::Struct,
            role: SymbolRole::Definition,
            scope: Vec::new(),
            location: SourceLocation { start: Position { line: 1, column: 1 }, end: Position { line: 1, column: 11 } },
            context: "pub struct CacheStore;".to_owned(),
            visibility: SymbolVisibility::Public,
            evidence: SymbolEvidence::Declaration,
        };
        assert!(symbol_match_score(&request, &symbol).is_some());
        let other = SourceSymbol { name: "CacheStoreBuilder".to_owned(), ..symbol };
        assert!(symbol_match_score(&request, &other).is_none());
    }
}
