use super::*;

const DEFAULT_QUERY_RESULT_LIMIT: usize = 20;
const MAX_QUERY_RESULT_LIMIT: usize = 256;
const MAX_QUERY_OMISSIONS: usize = 16;
const MAX_QUERY_REVISION_CHANGES: usize = 64;

#[derive(Clone)]
struct QuerySource {
    path: String,
    language: Option<SourceLanguage>,
    worktree_state: Option<WorktreeState>,
    status: Option<FileAnalysisStatus>,
    symbols: Vec<SourceSymbol>,
    limitations: Vec<String>,
    project_root: Option<String>,
}

#[derive(Clone)]
struct QueryCandidate {
    target: QueryTarget,
    path: String,
    project_root: Option<String>,
    language: Option<SourceLanguage>,
    symbol: Option<SourceSymbol>,
    reason: String,
    evidence: Vec<QueryEvidence>,
    confidence: ConfidenceTier,
    ambiguous: bool,
    partial: bool,
    score: u64,
    limitations: Vec<String>,
}

#[derive(Clone, Copy)]
struct MatchScore {
    score: u64,
    confidence: ConfidenceTier,
}

struct CandidateContext<'a> {
    ambiguous_paths: &'a BTreeSet<String>,
    resolution: &'a ChangeResolution,
}

struct CandidateInput {
    target: QueryTarget,
    symbol: Option<SourceSymbol>,
    evidence: Vec<QueryEvidence>,
    score: u64,
    reason: String,
    confidence: ConfidenceTier,
}

pub(crate) fn compile(request: QueryRequest, map: &MapReport, change_resolution: ChangeResolution) -> QueryResults {
    let request = normalize_request(request);
    let sources = query_sources(map);
    let change_paths = active_change_paths(&sources, &change_resolution);
    let ambiguous_paths = ambiguous_paths(map);
    let ranking = query_ranking(map);
    let candidate_context = CandidateContext { ambiguous_paths: &ambiguous_paths, resolution: &change_resolution };
    let mut candidates = Vec::new();

    for source in &sources {
        if !source_matches_file_filters(source, &request.filters, &change_paths) {
            continue;
        }
        let path_score = path_score(source, &request.filters);
        let text_path_score = request
            .filters
            .text
            .as_ref()
            .and_then(|query| match_text(query, &source.path));
        let symbol_constraints = request.filters.symbol.is_some() || request.filters.symbol_kind.is_some();
        let mut symbol_matches = Vec::new();

        for symbol in &source.symbols {
            let Some(symbol_score) = symbol_score(symbol, &request.filters) else {
                continue;
            };
            let Some(text_score) = text_symbol_score(&request.filters, symbol, text_path_score, symbol_constraints)
            else {
                continue;
            };
            let mut evidence = base_evidence(source, &request.filters, &change_paths);
            if let Some(match_score) = text_score {
                evidence.push(QueryEvidence {
                    kind: QueryEvidenceKind::Text,
                    detail: format!("text {} matched `{}`", match_mode(&request.filters), symbol.name),
                });
                symbol_matches.push((symbol.clone(), symbol_score, Some(match_score), evidence));
            } else if request.filters.text.is_none() {
                symbol_matches.push((symbol.clone(), symbol_score, None, evidence));
            }
        }

        if symbol_constraints || request.filters.text.is_some() {
            let no_symbol_match = symbol_matches.is_empty();
            let allow_path_fallback = no_symbol_match
                && text_path_score.is_some()
                && request.filters.symbol.is_none()
                && request.filters.symbol_kind.is_none();
            for (symbol, symbol_score, text_score, evidence) in symbol_matches {
                let score = ranking_score(&ranking, &source.path)
                    .saturating_add(symbol_score.score)
                    .saturating_add(text_score.map_or(0, |score| score.score));
                candidates.push(candidate_for_symbol(
                    source,
                    symbol,
                    evidence,
                    score,
                    symbol_score
                        .confidence
                        .min(text_score.map_or(ConfidenceTier::High, |score| score.confidence)),
                    &candidate_context,
                ));
            }
            if !symbol_matches_are_required(&request.filters) {
                // A path or text query can still return an omitted or source file
                // when the path matched but no retained symbol did.
                if allow_path_fallback {
                    candidates.push(candidate_for_file(
                        source,
                        base_evidence(source, &request.filters, &change_paths),
                        ranking_score(&ranking, &source.path)
                            .saturating_add(path_score.map_or(0, |score| score.score))
                            .saturating_add(text_path_score.map_or(0, |score| score.score)),
                        "the text query matched the retained source path",
                        &candidate_context,
                    ));
                }
            }
        } else {
            candidates.push(candidate_for_file(
                source,
                base_evidence(source, &request.filters, &change_paths),
                ranking_score(&ranking, &source.path).saturating_add(path_score.map_or(0, |score| score.score)),
                "the source satisfies the typed repository filters",
                &candidate_context,
            ));
        }
    }

    add_relevant_omissions(
        &mut candidates,
        map,
        &request.filters,
        &change_paths,
        &change_resolution,
        &ambiguous_paths,
        &sources,
    );
    add_missing_changed_paths(
        &mut candidates,
        &request.filters,
        &change_paths,
        &change_resolution,
        &sources,
    );

    candidates.sort_by(candidate_order);
    let total = candidates.len();
    let offset = request.offset.min(total);
    let page_limit = request.result_limit;
    let page = candidates.iter().skip(offset).take(page_limit);
    let mut matches = Vec::new();
    let mut budget_stopped = false;
    for candidate in page {
        let next = query_match(candidate);
        let mut proposed = matches.clone();
        proposed.push(next.clone());
        if estimate_matches(&proposed) > request.budget {
            budget_stopped = true;
            break;
        }
        matches.push(next);
    }

    let returned = matches.len();
    let omitted = total.saturating_sub(returned);
    let page_end = offset.saturating_add(page_limit).min(total);
    let continuation = (offset.saturating_add(returned) < total)
        .then_some(QueryCursor { offset: offset.saturating_add(returned), limit: page_limit });
    let mut omissions = query_omissions(
        map,
        &change_resolution,
        offset,
        total,
        returned,
        page_end,
        budget_stopped,
    );
    omissions.truncate(MAX_QUERY_OMISSIONS);
    let estimated_tokens = estimate_matches(&matches);
    let mut limitations = query_limitations(map, &change_resolution, &request, &sources);
    if budget_stopped {
        limitations
            .push("The query token budget stopped the page before every matching fact could be returned.".to_owned());
    }
    limitations.sort();
    limitations.dedup();

    let bounded_change_resolution = bound_change_resolution(change_resolution);
    let provenance = query_provenance(map, bounded_change_resolution, &limitations);
    let token_budget = request.budget;
    let result_limit = request.result_limit;
    QueryResults {
        request,
        matches,
        bounds: QueryBounds {
            token_budget,
            result_limit,
            offset,
            total,
            returned,
            omitted,
            estimated_tokens,
            truncated: omitted > 0,
            continuation,
        },
        omissions,
        provenance,
        limitations,
    }
}

fn normalize_request(mut request: QueryRequest) -> QueryRequest {
    request.repository = request.repository.trim().to_owned();
    request.result_limit = if request.result_limit == 0 {
        DEFAULT_QUERY_RESULT_LIMIT
    } else {
        request.result_limit.min(MAX_QUERY_RESULT_LIMIT)
    };
    request.budget = request.budget.max(1);
    normalize_text_query(request.filters.text.as_mut());
    normalize_path_query(request.filters.path.as_mut());
    normalize_symbol_query(request.filters.symbol.as_mut());
    normalize_project_query(request.filters.project.as_mut());
    normalize_changed_path_query(request.filters.changed_path.as_mut());
    request
}

fn normalize_text_query(query: Option<&mut TextQuery>) {
    if let Some(query) = query {
        query.value = query.value.trim().to_owned();
    }
}

fn normalize_path_query(query: Option<&mut PathQuery>) {
    if let Some(query) = query {
        query.value = normalize_query_path(&query.value);
    }
}

fn normalize_symbol_query(query: Option<&mut SymbolQuery>) {
    if let Some(query) = query {
        query.name = query.name.trim().to_owned();
    }
}

fn normalize_project_query(query: Option<&mut ProjectQuery>) {
    if let Some(query) = query {
        query.path = normalize_query_path(&query.path);
    }
}

fn normalize_changed_path_query(query: Option<&mut ChangedPathQuery>) {
    if let Some(query) = query {
        query.path = normalize_query_path(&query.path);
    }
}

fn normalize_query_path(path: &str) -> String {
    path.trim().replace('\\', "/").trim_start_matches("./").to_owned()
}

fn query_sources(map: &MapReport) -> Vec<QuerySource> {
    let roots = if map.reading_evidence.project_roots.is_empty() {
        &map.project_roots
    } else {
        &map.reading_evidence.project_roots
    };
    let source_evidence = if map.reading_evidence.sources.is_empty() {
        map.files
            .iter()
            .map(|file| ReadingSourceEvidence {
                path: file.path.clone(),
                language: file.language,
                worktree_state: file.worktree_state,
                status: file.status,
                symbols: file.symbols.clone(),
                limitations: file.limitations.clone(),
            })
            .collect::<Vec<_>>()
    } else {
        map.reading_evidence.sources.clone()
    };
    let mut sources = BTreeMap::<String, QuerySource>::new();
    for evidence in source_evidence {
        let language = Some(evidence.language);
        let worktree_state = Some(evidence.worktree_state);
        let status = Some(evidence.status);
        let mut limitations = evidence.limitations.clone();
        limitations.sort();
        limitations.dedup();
        sources.insert(
            evidence.path.clone(),
            QuerySource {
                project_root: crate::landmarks::project_root_for_path(&evidence.path, roots),
                path: evidence.path,
                language,
                worktree_state,
                status,
                symbols: evidence.symbols,
                limitations,
            },
        );
    }
    for file in &map.files {
        sources.entry(file.path.clone()).or_insert_with(|| QuerySource {
            project_root: crate::landmarks::project_root_for_path(&file.path, roots),
            path: file.path.clone(),
            language: Some(file.language),
            worktree_state: Some(file.worktree_state),
            status: Some(file.status),
            symbols: file.symbols.clone(),
            limitations: file.limitations.clone(),
        });
    }
    sources.into_values().collect()
}

fn active_change_paths(sources: &[QuerySource], resolution: &ChangeResolution) -> BTreeSet<String> {
    if resolution.status != ChangeResolutionStatus::NotRequested {
        return resolution
            .changes
            .iter()
            .flat_map(|change| std::iter::once(change.path.clone()).chain(change.previous_path.clone()))
            .collect();
    }
    sources
        .iter()
        .filter(|source| {
            source
                .worktree_state
                .is_some_and(|state| matches!(state, WorktreeState::Modified | WorktreeState::Untracked))
        })
        .map(|source| source.path.clone())
        .collect()
}

fn ambiguous_paths(map: &MapReport) -> BTreeSet<String> {
    let edges = if map.reading_evidence.graph.is_empty() {
        &map.edges
    } else {
        &map.reading_evidence
            .graph
            .iter()
            .map(|edge| edge.relationship.clone())
            .collect::<Vec<_>>()
    };
    edges
        .iter()
        .filter(|edge| edge.ambiguous)
        .flat_map(|edge| [edge.source.clone(), edge.target.clone()])
        .chain(
            map.findings
                .iter()
                .filter(|finding| finding.kind == MapFindingKind::AmbiguousReference)
                .map(|finding| finding.path.clone()),
        )
        .collect()
}

fn query_ranking(map: &MapReport) -> BTreeMap<String, u64> {
    let ranking = if map.reading_evidence.ranking.is_empty() { &map.ranking } else { &map.reading_evidence.ranking };
    ranking.iter().map(|rank| (rank.path.clone(), rank.score)).collect()
}

fn source_matches_file_filters(source: &QuerySource, filters: &QueryFilters, changed_paths: &BTreeSet<String>) -> bool {
    if filters
        .path
        .as_ref()
        .is_some_and(|query| match_path(&query.value, query.mode, &source.path).is_none())
    {
        return false;
    }
    if filters
        .project
        .as_ref()
        .is_some_and(|query| !project_matches(query, source))
    {
        return false;
    }
    if filters
        .language
        .is_some_and(|language| source.language != Some(language))
    {
        return false;
    }
    let is_test = is_test_path(&source.path);
    if matches!(filters.test, QueryTestFilter::Only) && !is_test {
        return false;
    }
    if matches!(filters.test, QueryTestFilter::Exclude) && is_test {
        return false;
    }
    if let Some(query) = &filters.changed_path {
        if changed_paths.is_empty()
            || !changed_paths.iter().any(|changed| {
                match_path(&query.path, query.mode, changed).is_some_and(|_| path_related(changed, &source.path))
            })
        {
            return false;
        }
    }
    true
}

fn path_related(left: &str, right: &str) -> bool {
    left == right || left.starts_with(&format!("{right}/")) || right.starts_with(&format!("{left}/"))
}

fn path_score(source: &QuerySource, filters: &QueryFilters) -> Option<MatchScore> {
    filters
        .path
        .as_ref()
        .and_then(|query| match_path(&query.value, query.mode, &source.path))
}

fn symbol_score(symbol: &SourceSymbol, filters: &QueryFilters) -> Option<MatchScore> {
    if filters.symbol_kind.is_some_and(|kind| kind != symbol.kind)
        || filters.symbol.as_ref().is_some_and(|query| {
            query.role.is_some_and(|role| role != symbol.role)
                || match_text_value(&query.name, query.mode, &symbol.name).is_none()
        })
    {
        return None;
    }
    let mut score = MatchScore { score: 0, confidence: ConfidenceTier::High };
    if let Some(query) = &filters.symbol {
        let matched = match_text_value(&query.name, query.mode, &symbol.name)?;
        score = matched;
    }
    if filters.symbol_kind.is_some() {
        score.score = score.score.saturating_add(800_000_000);
    }
    Some(score)
}

fn text_symbol_score(
    filters: &QueryFilters, symbol: &SourceSymbol, path_score: Option<MatchScore>, allow_path_match: bool,
) -> Option<Option<MatchScore>> {
    let Some(query) = &filters.text else {
        return Some(None);
    };
    match_text_value(&query.value, query.mode, &symbol.name)
        .or_else(|| match_text_value(&query.value, query.mode, &symbol.context))
        .or_else(|| allow_path_match.then_some(path_score).flatten())
        .map(Some)
}

fn match_mode(filters: &QueryFilters) -> &'static str {
    filters.text.as_ref().map_or("source", |query| query.mode.label())
}

fn match_path(value: &str, mode: QueryMatchMode, subject: &str) -> Option<MatchScore> {
    let value = normalize_query_path(value);
    let subject = normalize_query_path(subject);
    if value.is_empty() {
        return None;
    }
    match mode {
        QueryMatchMode::Exact if subject == value => {
            Some(MatchScore { score: 4_000_000_000, confidence: ConfidenceTier::High })
        }
        QueryMatchMode::Prefix if subject.starts_with(&value) => {
            Some(MatchScore { score: 3_000_000_000, confidence: ConfidenceTier::High })
        }
        QueryMatchMode::Substring if subject.contains(&value) => {
            Some(MatchScore { score: 2_000_000_000, confidence: ConfidenceTier::Medium })
        }
        _ => None,
    }
}

fn match_text_value(value: &str, mode: QueryMatchMode, subject: &str) -> Option<MatchScore> {
    let value = value.trim().to_ascii_lowercase();
    let subject = subject.to_ascii_lowercase();
    if value.is_empty() {
        return None;
    }
    match mode {
        QueryMatchMode::Exact if subject == value => {
            Some(MatchScore { score: 4_000_000_000, confidence: ConfidenceTier::High })
        }
        QueryMatchMode::Prefix if subject.starts_with(&value) => {
            Some(MatchScore { score: 3_000_000_000, confidence: ConfidenceTier::High })
        }
        QueryMatchMode::Substring if subject.contains(&value) => {
            Some(MatchScore { score: 2_000_000_000, confidence: ConfidenceTier::Medium })
        }
        _ => None,
    }
}

fn match_text(query: &TextQuery, subject: &str) -> Option<MatchScore> {
    match_text_value(&query.value, query.mode, subject)
}

fn project_root(source: &QuerySource) -> &str {
    source.project_root.as_deref().unwrap_or(".")
}

fn project_matches(query: &ProjectQuery, source: &QuerySource) -> bool {
    match_path(&query.path, query.mode, project_root(source)).is_some()
}

fn base_evidence(source: &QuerySource, filters: &QueryFilters, changed_paths: &BTreeSet<String>) -> Vec<QueryEvidence> {
    let mut evidence = vec![QueryEvidence {
        kind: QueryEvidenceKind::SourceMap,
        detail: "the path and symbols came from the retained repository source map".to_owned(),
    }];
    if let Some(query) = &filters.path {
        evidence.push(QueryEvidence {
            kind: QueryEvidenceKind::Path,
            detail: format!("{} path match for `{}`", query.mode.label(), query.value),
        });
    }
    if let Some(query) = &filters.project {
        evidence.push(QueryEvidence {
            kind: QueryEvidenceKind::Project,
            detail: format!("{} project-root match for `{}`", query.mode.label(), query.path),
        });
    }
    if let Some(language) = filters.language {
        evidence.push(QueryEvidence {
            kind: QueryEvidenceKind::Language,
            detail: format!("language matched `{}`", language.label()),
        });
    }
    if let Some(kind) = filters.symbol_kind {
        evidence.push(QueryEvidence {
            kind: QueryEvidenceKind::SymbolKind,
            detail: format!("symbol kind matched `{}`", kind.label()),
        });
    }
    if filters.test != QueryTestFilter::Any {
        evidence.push(QueryEvidence {
            kind: QueryEvidenceKind::Test,
            detail: format!("test filter `{}` matched the path", filters.test.label()),
        });
    }
    if let Some(query) = &filters.changed_path {
        let change = changed_paths
            .iter()
            .find(|path| match_path(&query.path, query.mode, path).is_some_and(|_| path_related(path, &source.path)));
        if let Some(change) = change {
            evidence.push(QueryEvidence {
                kind: QueryEvidenceKind::ChangedPath,
                detail: format!("changed path `{change}` is related to `{}`", query.path),
            });
        }
    }
    if let Some(state) = source.worktree_state.filter(|state| *state != WorktreeState::Tracked) {
        evidence.push(QueryEvidence {
            kind: QueryEvidenceKind::Worktree,
            detail: format!("current worktree state is `{}`", state.label()),
        });
    }
    evidence
}

fn symbol_matches_are_required(filters: &QueryFilters) -> bool {
    filters.symbol.is_some() || filters.symbol_kind.is_some()
}

fn candidate_for_symbol(
    source: &QuerySource, symbol: SourceSymbol, mut evidence: Vec<QueryEvidence>, score: u64,
    confidence: ConfidenceTier, context: &CandidateContext<'_>,
) -> QueryCandidate {
    evidence.push(QueryEvidence {
        kind: QueryEvidenceKind::Symbol,
        detail: format!("retained {} {} evidence", symbol.role.label(), symbol.kind.label()),
    });
    candidate(
        source,
        CandidateInput {
            target: QueryTarget::Symbol,
            reason: format!("the typed query matched {} `{}`", symbol.kind.label(), symbol.name),
            symbol: Some(symbol),
            evidence,
            score,
            confidence,
        },
        context,
    )
}

fn candidate_for_file(
    source: &QuerySource, evidence: Vec<QueryEvidence>, score: u64, reason: &str, context: &CandidateContext<'_>,
) -> QueryCandidate {
    candidate(
        source,
        CandidateInput {
            target: QueryTarget::File,
            symbol: None,
            evidence,
            score,
            reason: reason.to_owned(),
            confidence: ConfidenceTier::High,
        },
        context,
    )
}

fn candidate(source: &QuerySource, input: CandidateInput, context: &CandidateContext<'_>) -> QueryCandidate {
    let CandidateInput { target, symbol, mut evidence, score, reason, mut confidence } = input;
    let ambiguous = context.ambiguous_paths.contains(&source.path);
    let partial = source.status == Some(FileAnalysisStatus::Partial);
    let mut limitations = source.limitations.clone();
    if partial {
        confidence = confidence.min(ConfidenceTier::Medium);
        limitations.push("source analysis for this path was partial".to_owned());
    }
    if ambiguous {
        confidence = confidence.min(ConfidenceTier::Medium);
        evidence.push(QueryEvidence {
            kind: QueryEvidenceKind::Omission,
            detail: "retained relationship evidence has multiple lexical candidates".to_owned(),
        });
        limitations.push("ambiguous lexical evidence does not establish one semantic target".to_owned());
    }
    if context.resolution.status != ChangeResolutionStatus::NotRequested {
        evidence.push(QueryEvidence {
            kind: QueryEvidenceKind::Revision,
            detail: format!("revision evidence status is `{}`", context.resolution.status.label()),
        });
    }
    limitations.sort();
    limitations.dedup();
    evidence.sort();
    evidence.dedup();
    QueryCandidate {
        target,
        path: source.path.clone(),
        project_root: source.project_root.clone(),
        language: source.language,
        symbol,
        reason,
        evidence,
        confidence,
        ambiguous,
        partial,
        score,
        limitations,
    }
}

fn ranking_score(ranking: &BTreeMap<String, u64>, path: &str) -> u64 {
    ranking.get(path).copied().unwrap_or_default() / 1_000
}

fn candidate_order(left: &QueryCandidate, right: &QueryCandidate) -> std::cmp::Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| right.confidence.cmp(&left.confidence))
        .then_with(|| left.path.cmp(&right.path))
        .then_with(|| left.target.cmp(&right.target))
        .then_with(|| {
            left.symbol
                .as_ref()
                .map(|symbol| symbol.name.as_str())
                .cmp(&right.symbol.as_ref().map(|symbol| symbol.name.as_str()))
        })
        .then_with(|| {
            left.symbol
                .as_ref()
                .map(|symbol| (&symbol.location.start.line, &symbol.location.start.column))
                .cmp(
                    &right
                        .symbol
                        .as_ref()
                        .map(|symbol| (&symbol.location.start.line, &symbol.location.start.column)),
                )
        })
}

fn query_match(candidate: &QueryCandidate) -> QueryMatch {
    let id = match &candidate.symbol {
        Some(symbol) => format!(
            "symbol:{}:{}:{}:{}",
            candidate.path, symbol.location.start.line, symbol.location.start.column, symbol.name
        ),
        None => format!("file:{}", candidate.path),
    };
    QueryMatch {
        id,
        target: candidate.target,
        path: candidate.path.clone(),
        project_root: candidate.project_root.clone(),
        language: candidate.language,
        symbol: candidate.symbol.clone(),
        reason: candidate.reason.clone(),
        evidence: candidate.evidence.clone(),
        confidence: candidate.confidence,
        ambiguous: candidate.ambiguous,
        partial: candidate.partial,
        score: candidate.score,
        limitations: candidate.limitations.clone(),
    }
}

fn estimate_matches(matches: &[QueryMatch]) -> usize {
    serde_json::to_string(matches).map_or(usize::MAX, |json| token_count(&json))
}

fn add_relevant_omissions(
    candidates: &mut Vec<QueryCandidate>, map: &MapReport, filters: &QueryFilters, changed_paths: &BTreeSet<String>,
    resolution: &ChangeResolution, ambiguous_paths: &BTreeSet<String>, sources: &[QuerySource],
) {
    let source_paths = sources
        .iter()
        .map(|source| source.path.as_str())
        .collect::<BTreeSet<_>>();
    let context = CandidateContext { ambiguous_paths, resolution };
    for omission in &map.omissions {
        if source_paths.contains(omission.path.as_str())
            || !omitted_path_matches(omission, filters, changed_paths, &map.project_roots)
        {
            continue;
        }
        let source = QuerySource {
            path: omission.path.clone(),
            language: None,
            worktree_state: None,
            status: Some(FileAnalysisStatus::Partial),
            symbols: Vec::new(),
            limitations: vec![omission.detail.clone()],
            project_root: crate::landmarks::project_root_for_path(&omission.path, &map.project_roots),
        };
        let mut evidence = base_evidence(&source, filters, changed_paths);
        evidence.push(QueryEvidence {
            kind: QueryEvidenceKind::Omission,
            detail: format!("source path was omitted as `{}`", omission.reason.label()),
        });
        candidates.push(candidate(
            &source,
            CandidateInput {
                target: QueryTarget::File,
                symbol: None,
                evidence,
                score: 500_000_000,
                reason: "the path matched, but source analysis retained an omission".to_owned(),
                confidence: ConfidenceTier::Low,
            },
            &context,
        ));
    }
}

fn omitted_path_matches(
    omission: &SourceOmission, filters: &QueryFilters, changed_paths: &BTreeSet<String>, project_roots: &[ProjectRoot],
) -> bool {
    let path_match = filters
        .path
        .as_ref()
        .is_some_and(|query| match_path(&query.value, query.mode, &omission.path).is_some());
    let text_match = filters
        .text
        .as_ref()
        .is_some_and(|query| match_text(query, &omission.path).is_some());
    let changed_match = filters.changed_path.as_ref().is_some_and(|query| {
        changed_paths
            .iter()
            .any(|path| match_path(&query.path, query.mode, path).is_some_and(|_| path_related(path, &omission.path)))
    });
    let project =
        crate::landmarks::project_root_for_path(&omission.path, project_roots).unwrap_or_else(|| ".".to_owned());
    let project_match = filters
        .project
        .as_ref()
        .is_some_and(|query| match_path(&query.path, query.mode, &project).is_some());
    let test_match = match filters.test {
        QueryTestFilter::Any => true,
        QueryTestFilter::Only => is_test_path(&omission.path),
        QueryTestFilter::Exclude => !is_test_path(&omission.path),
    };
    (path_match || text_match || changed_match || project_match) && test_match
}

fn add_missing_changed_paths(
    candidates: &mut Vec<QueryCandidate>, filters: &QueryFilters, changed_paths: &BTreeSet<String>,
    resolution: &ChangeResolution, sources: &[QuerySource],
) {
    let Some(query) = &filters.changed_path else {
        return;
    };
    let known = sources
        .iter()
        .map(|source| source.path.as_str())
        .collect::<BTreeSet<_>>();
    let empty_ambiguity = BTreeSet::new();
    let context = CandidateContext { ambiguous_paths: &empty_ambiguity, resolution };
    for path in changed_paths {
        if known.contains(path.as_str())
            || match_path(&query.path, query.mode, path).is_none()
            || !matches!(filters.test, QueryTestFilter::Any)
                && (matches!(filters.test, QueryTestFilter::Only) != is_test_path(path))
        {
            continue;
        }
        let source = QuerySource {
            path: path.clone(),
            language: None,
            worktree_state: None,
            status: Some(FileAnalysisStatus::Partial),
            symbols: Vec::new(),
            limitations: vec!["the changed path has no retained current source evidence".to_owned()],
            project_root: None,
        };
        let evidence = vec![
            QueryEvidence {
                kind: QueryEvidenceKind::ChangedPath,
                detail: format!("changed path `{path}` matched the filter"),
            },
            QueryEvidence {
                kind: QueryEvidenceKind::Revision,
                detail: format!("change resolution status is `{}`", resolution.status.label()),
            },
        ];
        candidates.push(candidate(
            &source,
            CandidateInput {
                target: QueryTarget::File,
                symbol: None,
                evidence,
                score: 1_000_000_000,
                reason: "the changed path has no current source evidence".to_owned(),
                confidence: ConfidenceTier::Low,
            },
            &context,
        ));
    }
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

fn query_omissions(
    map: &MapReport, resolution: &ChangeResolution, offset: usize, total: usize, returned: usize, page_end: usize,
    budget_stopped: bool,
) -> Vec<QueryOmission> {
    let mut omissions = Vec::new();
    if offset > 0 {
        omissions.push(QueryOmission {
            reason: QueryOmissionReason::Pagination,
            count: offset.min(total),
            paths: Vec::new(),
            detail: "earlier matching facts are available through the request offset".to_owned(),
        });
    }
    if page_end < total && !budget_stopped {
        omissions.push(QueryOmission {
            reason: QueryOmissionReason::ResultLimit,
            count: total - page_end,
            paths: Vec::new(),
            detail: "the result limit ended this bounded page".to_owned(),
        });
    }
    if budget_stopped {
        omissions.push(QueryOmission {
            reason: QueryOmissionReason::TokenBudget,
            count: total.saturating_sub(offset.saturating_add(returned)),
            paths: Vec::new(),
            detail: "the token budget ended this bounded page".to_owned(),
        });
    }
    let source_paths = map
        .omissions
        .iter()
        .take(MAX_QUERY_OMISSIONS)
        .map(|omission| omission.path.clone())
        .collect::<Vec<_>>();
    if !source_paths.is_empty() {
        omissions.push(QueryOmission {
            reason: QueryOmissionReason::SourceEvidence,
            count: map.omissions.len(),
            paths: source_paths,
            detail: "some repository paths were omitted before query selection".to_owned(),
        });
    }
    if (resolution.status == ChangeResolutionStatus::Unresolved || resolution.status == ChangeResolutionStatus::Partial)
        && !resolution.uncertainty.is_empty()
    {
        omissions.push(QueryOmission {
            reason: QueryOmissionReason::Revision,
            count: resolution.uncertainty.len(),
            paths: Vec::new(),
            detail: "revision evidence is partial or unresolved; inspect provenance uncertainty".to_owned(),
        });
    }
    omissions
}

fn query_limitations(
    map: &MapReport, resolution: &ChangeResolution, request: &QueryRequest, sources: &[QuerySource],
) -> Vec<String> {
    let mut limitations = map.limitations.clone();
    if sources.len() < map.inventory.analyzed {
        limitations.push("the query source index did not retain every analyzed source path".to_owned());
    }
    if request.filters.text.is_some() {
        limitations.push(
            "text matching uses retained paths, symbol names, and syntax contexts; raw source bodies are not rescanned"
                .to_owned(),
        );
    }
    if request.filters.symbol.is_some() || request.filters.symbol_kind.is_some() {
        limitations.push(
            "symbol matching uses retained syntax evidence and does not establish compiler-level resolution".to_owned(),
        );
    }
    if request.filters.changed_path.is_some() && resolution.status == ChangeResolutionStatus::NotRequested {
        limitations
            .push("changed-path filtering uses current worktree state because no revision was requested".to_owned());
    }
    if resolution.status == ChangeResolutionStatus::Partial || resolution.status == ChangeResolutionStatus::Unresolved {
        limitations.push(
            "revision evidence is incomplete; paths without resolved change evidence are not treated as changed"
                .to_owned(),
        );
    }
    if map.availability.unsupported_paths > 0 {
        limitations.push(format!(
            "{} unsupported source path(s) were omitted before syntax analysis",
            map.availability.unsupported_paths
        ));
    }
    limitations
}

fn query_provenance(map: &MapReport, change_resolution: ChangeResolution, limitations: &[String]) -> QueryProvenance {
    let mut provenance_limitations = limitations.to_vec();
    provenance_limitations.sort();
    provenance_limitations.dedup();
    QueryProvenance {
        repository: map.repository_root.clone(),
        scope_path: map.scope_path.clone(),
        profile: map.profile,
        head: map.head.clone(),
        worktree: map.worktree.clone(),
        cache: CacheProvenance {
            mode: map.cache.mode,
            status: map.cache.status,
            index_status: map.cache.index_status,
            index_detail: map.cache.index_detail.clone(),
            available: map.cache.mode != CacheMode::Disabled,
            reused: map.cache.reused.len(),
            invalidated: map.cache.invalidated.len(),
            hits: map.cache.hits,
            misses: map.cache.misses,
            refreshed: map.cache.refreshed.len(),
            stale: map.cache.stale.len(),
        },
        query_packs: map.query_packs.clone(),
        source_files: map.collections.files.clone(),
        symbols: map.collections.symbols.clone(),
        relationships: map.collections.edges.clone(),
        partial: map.availability.resource_limited
            || map.availability.partial_files > 0
            || map.availability.unsupported_paths > 0
            || change_resolution.status == ChangeResolutionStatus::Partial
            || change_resolution.status == ChangeResolutionStatus::Unresolved,
        change_resolution,
        limitations: provenance_limitations,
    }
}

fn bound_change_resolution(mut resolution: ChangeResolution) -> ChangeResolution {
    if resolution.changes.len() > MAX_QUERY_REVISION_CHANGES {
        resolution.changes.truncate(MAX_QUERY_REVISION_CHANGES);
        resolution.uncertainty.push(ChangeUncertainty {
            kind: "query_projection".to_owned(),
            detail: format!("query provenance retained the first {MAX_QUERY_REVISION_CHANGES} resolved changes"),
        });
        if resolution.status == ChangeResolutionStatus::Resolved {
            resolution.status = ChangeResolutionStatus::Partial;
        }
    }
    resolution
}

impl From<&QueryRevision> for ContextRevisionContext {
    fn from(revision: &QueryRevision) -> Self {
        Self {
            base: revision.base.clone(),
            head: revision.head.clone(),
            range: revision.range.clone(),
            dirty_worktree: revision.dirty_worktree,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol(name: &str, kind: SymbolKind) -> SourceSymbol {
        SourceSymbol {
            name: name.to_owned(),
            kind,
            role: SymbolRole::Definition,
            scope: Vec::new(),
            location: SourceLocation { start: Position { line: 1, column: 1 }, end: Position { line: 1, column: 10 } },
            context: format!("pub fn {name}() {{}}"),
            visibility: SymbolVisibility::Public,
            evidence: SymbolEvidence::Declaration,
        }
    }

    fn source(path: &str, language: SourceLanguage, symbols: Vec<SourceSymbol>) -> SourceFile {
        SourceFile {
            path: path.to_owned(),
            language,
            extension: path.rsplit('.').next().unwrap_or_default().to_owned(),
            worktree_state: WorktreeState::Tracked,
            status: FileAnalysisStatus::Complete,
            symbols,
            limitations: Vec::new(),
            classifications: Vec::new(),
            classification_overridden: false,
        }
    }

    fn fixture_map() -> MapReport {
        let files = vec![
            source(
                "packages/api/src/build.rs",
                SourceLanguage::Rust,
                vec![symbol("build", SymbolKind::Function)],
            ),
            source(
                "packages/api/tests/build_test.rs",
                SourceLanguage::Rust,
                vec![symbol("build_test", SymbolKind::Function)],
            ),
            source(
                "packages/web/src/view.ts",
                SourceLanguage::TypeScript,
                vec![symbol("View", SymbolKind::Class)],
            ),
        ];
        MapReport {
            profile: AnalysisProfile::Evidence,
            repository_root: "/fixture".to_owned(),
            scope_path: ".".to_owned(),
            head: HeadSnapshot::default(),
            worktree: WorktreeSnapshot::default(),
            query_pack: "mixed".to_owned(),
            query_packs: BTreeMap::new(),
            exclusions: Vec::new(),
            task_seeds: TaskSeeds::default(),
            inventory: MapInventory { tracked: 3, modified: 0, untracked: 0, analyzed: 3, omitted: 0 },
            classifications: MapClassificationSummary::default(),
            files: files.clone(),
            omissions: Vec::new(),
            findings: Vec::new(),
            limitations: Vec::new(),
            edges: Vec::new(),
            ranking: Vec::new(),
            selection: MapSelection {
                token_budget: 1_000,
                estimated_tokens: 0,
                snippets: Vec::new(),
                primary_languages: Vec::new(),
                omitted_relevant_paths: Vec::new(),
                shortfall: None,
            },
            cache: MapCacheReport {
                mode: CacheMode::Disabled,
                status: CacheStatus::Disabled,
                index_status: PersistentIndexStatus::Bypassed,
                index_detail: None,
                matched: 0,
                unmatched: 0,
                unavailable: 0,
                reused: Vec::new(),
                invalidated: Vec::new(),
                hits: 0,
                misses: 0,
                refreshed: Vec::new(),
                stale: Vec::new(),
            },
            landmarks: Vec::new(),
            project_roots: vec![ProjectRoot {
                path: "packages/api".to_owned(),
                kind: ProjectRootKind::Package,
                reason: "fixture".to_owned(),
                manifests: Vec::new(),
                manifest_metadata: Vec::new(),
                landmark_total: 0,
                recommendation_total: 0,
                recommended_paths: Vec::new(),
            }],
            collections: MapCollections {
                files: CollectionSummary::complete(3),
                symbols: CollectionSummary::complete(3),
                omissions: CollectionSummary::complete(0),
                findings: CollectionSummary::complete(0),
                edges: CollectionSummary::complete(0),
                ranking: CollectionSummary::complete(0),
                snippets: CollectionSummary::complete(0),
                landmarks: CollectionSummary::complete(0),
                project_roots: CollectionSummary::complete(1),
            },
            availability: MapAvailability::default(),
            reading_evidence: ReadingPlanEvidence {
                sources: files
                    .iter()
                    .map(|file| ReadingSourceEvidence {
                        path: file.path.clone(),
                        language: file.language,
                        worktree_state: file.worktree_state,
                        status: file.status,
                        symbols: file.symbols.clone(),
                        limitations: Vec::new(),
                    })
                    .collect(),
                ranking: Vec::new(),
                graph: Vec::new(),
                omissions: Vec::new(),
                landmarks: Vec::new(),
                project_roots: vec![ProjectRoot {
                    path: "packages/api".to_owned(),
                    kind: ProjectRootKind::Package,
                    reason: "fixture".to_owned(),
                    manifests: Vec::new(),
                    manifest_metadata: Vec::new(),
                    landmark_total: 0,
                    recommendation_total: 0,
                    recommended_paths: Vec::new(),
                }],
            },
        }
    }

    #[test]
    fn exact_prefix_and_substring_symbol_queries_are_typed_and_deterministic() {
        let map = fixture_map();
        for (mode, expected_total) in [
            (QueryMatchMode::Exact, 1),
            (QueryMatchMode::Prefix, 2),
            (QueryMatchMode::Substring, 2),
        ] {
            let request = QueryRequest {
                filters: QueryFilters {
                    symbol: Some(SymbolQuery { name: "build".to_owned(), mode, role: None }),
                    ..QueryFilters::default()
                },
                cache_mode: CacheMode::Disabled,
                ..QueryRequest::new("/fixture")
            };
            let first = compile(request.clone(), &map, ChangeResolution::default());
            let second = compile(request, &map, ChangeResolution::default());
            assert_eq!(first, second);
            assert_eq!(first.bounds.total, expected_total);
            assert_eq!(
                first.matches[0].symbol.as_ref().map(|symbol| symbol.name.as_str()),
                Some("build")
            );
        }
        let path_text = compile(
            QueryRequest {
                filters: QueryFilters {
                    text: Some(TextQuery { value: "build.rs".to_owned(), mode: QueryMatchMode::Substring }),
                    ..QueryFilters::default()
                },
                cache_mode: CacheMode::Disabled,
                ..QueryRequest::new("/fixture")
            },
            &map,
            ChangeResolution::default(),
        );
        assert_eq!(path_text.bounds.total, 1);
        assert_eq!(path_text.matches[0].target, QueryTarget::File);
    }

    #[test]
    fn typed_query_request_fixture_round_trips_without_semantic_loss() {
        let request: QueryRequest =
            serde_json::from_str(include_str!("../../../../schema/v1/golden/query_request.json"))
                .expect("query fixture");
        let encoded = serde_json::to_value(request).expect("query request serializes");
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../schema/v1/golden/query_request.json"))
                .expect("fixture JSON");
        assert_eq!(encoded, fixture);
    }

    #[test]
    fn repository_filters_compose_and_report_bounded_ambiguity_state() {
        let mut map = fixture_map();
        map.files[0].worktree_state = WorktreeState::Modified;
        map.reading_evidence.sources[0].worktree_state = WorktreeState::Modified;
        map.reading_evidence.sources[0].status = FileAnalysisStatus::Partial;
        map.reading_evidence.sources[0]
            .limitations
            .push("partial fixture evidence".to_owned());
        map.availability.partial_files = 1;
        map.edges.push(LexicalEdge {
            source: "packages/api/src/build.rs".to_owned(),
            target: "packages/api/src/other.rs".to_owned(),
            symbol: "build".to_owned(),
            ambiguous: true,
            candidates: vec!["packages/api/src/other.rs".to_owned()],
            candidate_group: "fixture-group".to_owned(),
            resolution_reason: LexicalResolutionReason::SameModule,
            confidence: ConfidenceTier::Low,
            target_visibility: SymbolVisibility::Unknown,
        });
        let request = QueryRequest {
            filters: QueryFilters {
                project: Some(ProjectQuery { path: "packages/api".to_owned(), mode: QueryMatchMode::Exact }),
                language: Some(SourceLanguage::Rust),
                symbol_kind: Some(SymbolKind::Function),
                test: QueryTestFilter::Exclude,
                changed_path: Some(ChangedPathQuery { path: "packages/api".to_owned(), mode: QueryMatchMode::Prefix }),
                ..QueryFilters::default()
            },
            cache_mode: CacheMode::Disabled,
            ..QueryRequest::new("/fixture")
        };
        let results = compile(request, &map, ChangeResolution::default());
        assert_eq!(results.bounds.total, 1);
        assert_eq!(results.matches[0].path, "packages/api/src/build.rs");
        assert!(
            results.matches[0]
                .evidence
                .iter()
                .any(|evidence| evidence.kind == QueryEvidenceKind::ChangedPath)
        );
        assert!(
            results
                .provenance
                .limitations
                .iter()
                .any(|limitation| limitation.contains("changed-path"))
        );
        assert!(results.matches[0].ambiguous);
        assert!(results.matches[0].partial);
        assert!(results.provenance.partial);
    }

    #[test]
    fn high_cardinality_results_expose_totals_and_a_continuation_cursor() {
        let map = fixture_map();
        let request = QueryRequest {
            result_limit: 1,
            budget: 100_000,
            cache_mode: CacheMode::Disabled,
            ..QueryRequest::new("/fixture")
        };
        let first = compile(request, &map, ChangeResolution::default());
        assert_eq!(first.bounds.total, 3);
        assert_eq!(first.bounds.returned, 1);
        assert_eq!(first.bounds.omitted, 2);
        assert_eq!(first.bounds.continuation, Some(QueryCursor { offset: 1, limit: 1 }));
        assert!(!first.is_complete());

        let next_request = QueryRequest {
            offset: first.bounds.continuation.expect("continuation").offset,
            result_limit: 1,
            budget: 100_000,
            cache_mode: CacheMode::Disabled,
            ..QueryRequest::new("/fixture")
        };
        let next = compile(next_request, &map, ChangeResolution::default());
        assert_eq!(next.bounds.offset, 1);
        assert_eq!(next.bounds.returned, 1);
        assert_ne!(first.matches[0].id, next.matches[0].id);
    }
}
