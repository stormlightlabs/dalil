use std::collections::BTreeSet;

use super::*;

/// Convert the legacy search adapter request into the shared typed query
/// request. Adapters may add filters, but matching semantics stay in the
/// repository-query compiler.
pub(crate) fn query_request(request: &SearchRequest, repository: &str, cache_mode: CacheMode) -> QueryRequest {
    let mut filters = request.filters.clone();
    if request.mode == SearchQueryMode::Symbol {
        if filters.symbol.is_none() {
            filters.symbol = Some(SymbolQuery { name: request.query.clone(), mode: QueryMatchMode::Exact, role: None });
        }
    } else if filters.text.is_none() {
        filters.text = Some(TextQuery { value: request.query.clone(), mode: QueryMatchMode::Substring });
    }
    QueryRequest {
        repository: repository.to_owned(),
        filters,
        result_limit: request.result_limit,
        offset: 0,
        budget: request.budget,
        profile: request.profile,
        cache_mode,
        revision: request.revision.clone(),
    }
}

/// Project the shared typed query result into the concise, human-oriented
/// search shape retained for compatibility with existing report consumers.
pub(crate) fn from_query(query: QueryResults) -> SearchResults {
    let request = search_request_from_query(&query.request);
    let matches = query
        .matches
        .iter()
        .enumerate()
        .map(|(index, result)| SearchMatch {
            recommendation: ReadingRecommendation {
                ordinal: index + 1,
                purpose: if is_test_path(&result.path) { ReadingPurpose::Tests } else { ReadingPurpose::Architecture },
                path: result.path.clone(),
                project_root: result.project_root.clone(),
                reason: result.reason.clone(),
                evidence_kinds: query_evidence_kinds(&result.evidence),
                confidence: result.confidence,
                limitations: if result.limitations.is_empty() {
                    vec![
                        "Search matches are lexical source evidence; inspect the source before relying on it."
                            .to_owned(),
                    ]
                } else {
                    result.limitations.clone()
                },
            },
            target: match result.target {
                QueryTarget::File => SearchTarget::Path,
                QueryTarget::Symbol => SearchTarget::Symbol,
            },
            symbol: result.symbol.clone(),
            score: result.score,
            anchor: false,
        })
        .collect::<Vec<_>>();
    let shortfall = if query.bounds.total == 0 {
        Some(SearchShortfall {
            requested: request.result_limit,
            returned: 0,
            reason: format!(
                "No strong {} matches were found for `{}`.",
                request.mode.label(),
                request.query
            ),
        })
    } else if query.bounds.returned < request.result_limit {
        let reason = if query
            .omissions
            .iter()
            .any(|omission| omission.reason == QueryOmissionReason::TokenBudget)
        {
            "The shared token budget stopped before every strong match could be returned.".to_owned()
        } else {
            "Fewer strong matches exist than the requested result limit.".to_owned()
        };
        Some(SearchShortfall { requested: request.result_limit, returned: query.bounds.returned, reason })
    } else {
        None
    };
    SearchResults {
        request,
        matches,
        budget: SearchBudget {
            token_budget: query.bounds.token_budget,
            result_limit: query.bounds.result_limit,
            total_candidates: query.bounds.total,
            returned: query.bounds.returned,
            estimated_tokens: query.bounds.estimated_tokens,
            truncated: !query.is_complete(),
        },
        shortfall,
        limitations: query.limitations.clone(),
        query: Some(query),
    }
}

fn search_request_from_query(query: &QueryRequest) -> SearchRequest {
    let mode = query
        .filters
        .symbol
        .as_ref()
        .map_or(SearchQueryMode::Plain, |_| SearchQueryMode::Symbol);
    let value = match mode {
        SearchQueryMode::Plain => query
            .filters
            .text
            .as_ref()
            .map(|text| text.value.clone())
            .unwrap_or_default(),
        SearchQueryMode::Symbol => query
            .filters
            .symbol
            .as_ref()
            .map(|symbol| symbol.name.clone())
            .unwrap_or_default(),
    };
    SearchRequest {
        repository: query.repository.clone(),
        query: value,
        mode,
        result_limit: query.result_limit,
        budget: query.budget,
        profile: query.profile,
        filters: query.filters.clone(),
        revision: query.revision.clone(),
    }
}

fn query_evidence_kinds(evidence: &[QueryEvidence]) -> Vec<ReadingEvidenceKind> {
    let mut kinds = BTreeSet::new();
    for evidence in evidence {
        kinds.insert(match evidence.kind {
            QueryEvidenceKind::Revision | QueryEvidenceKind::ChangedPath | QueryEvidenceKind::Worktree => {
                ReadingEvidenceKind::Focus
            }
            _ => ReadingEvidenceKind::SourceMap,
        });
    }
    kinds.into_iter().collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_search_defaults_to_a_substring_text_query() {
        let request = SearchRequest { query: "invalidation".to_owned(), ..SearchRequest::default() };
        let query = query_request(&request, "/fixture", CacheMode::Disabled);

        assert_eq!(
            query.filters.text.as_ref().map(|text| text.mode),
            Some(QueryMatchMode::Substring)
        );
        assert_eq!(
            query.filters.text.as_ref().map(|text| text.value.as_str()),
            Some("invalidation")
        );
    }

    #[test]
    fn typed_results_remain_available_alongside_the_concise_projection() {
        let query = QueryResults {
            request: QueryRequest {
                repository: "/fixture".to_owned(),
                filters: QueryFilters {
                    text: Some(TextQuery { value: "cache".to_owned(), mode: QueryMatchMode::Substring }),
                    ..QueryFilters::default()
                },
                ..QueryRequest::default()
            },
            bounds: QueryBounds { total: 1, returned: 1, ..QueryBounds::default() },
            ..QueryResults::default()
        };
        let projected = from_query(query);

        assert_eq!(projected.matches.len(), 0);
        assert!(projected.query.is_some());
    }
}
