use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::utils::token_count;

const MAX_SYMBOLS_PER_TARGET: usize = 3;
const MAX_TESTS: usize = 4;
const MAX_OWNERSHIP_SIGNALS: usize = 4;
const MAX_HISTORY_EVIDENCE: usize = 8;
const MAX_UNCERTAINTIES: usize = 6;

#[derive(Default)]
struct TargetCandidate {
    symbols: Vec<ContextSymbol>,
    evidence: BTreeSet<ImpactEvidenceKind>,
    score: u64,
    confidence: ConfidenceTier,
    reasons: BTreeSet<String>,
    limitations: Vec<String>,
}

struct TargetInput<'a> {
    path: &'a str,
    source: &'a ReadingSourceEvidence,
    rank: Option<&'a FileRank>,
    evidence: ImpactEvidenceKind,
    priority: u64,
    confidence: ConfidenceTier,
    reason: &'a str,
    changed_symbols: Option<&'a BTreeSet<String>>,
}

/// Compile evidence around resolved changes into a review result. The
/// operation reports observed relationships and their limits, rather than
/// claiming semantic reachability or breakage.
pub fn compile(
    request: ContextRequest, map: &MapReport, history: &HistoryReport, change_resolution: ChangeResolution,
) -> ImpactReport {
    let request = normalized_request(request, map, &change_resolution);
    let budget = request.budget;
    let changed_paths = changed_paths(&change_resolution);
    let changed_symbols = changed_symbols(&change_resolution);
    let sources = source_evidence(map);
    let edges = graph_evidence(map);
    let ranks = rank_evidence(map);
    let source_by_path = sources
        .iter()
        .map(|source| (source.path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let rank_by_path = ranks
        .iter()
        .map(|rank| (rank.path.as_str(), rank))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = BTreeMap::<String, TargetCandidate>::new();
    let mut relationships = Vec::new();

    for path in &changed_paths {
        let Some(source) = source_by_path.get(path.as_str()) else {
            continue;
        };
        add_target(
            &mut candidates,
            TargetInput {
                path,
                source,
                rank: rank_by_path.get(path.as_str()).copied(),
                evidence: ImpactEvidenceKind::Structural,
                priority: 4_000_000_000,
                confidence: ConfidenceTier::High,
                reason: "the resolved change includes this current source path",
                changed_symbols: changed_symbols.get(path),
            },
        );
    }

    for edge in &edges {
        let related = changed_paths.contains(&edge.source)
            || changed_paths.contains(&edge.target)
            || changed_symbols
                .get(&edge.target)
                .is_some_and(|symbols| symbols.contains(&edge.symbol));
        if !related {
            continue;
        }
        relationships.push(lexical_relationship(edge));
        for path in [&edge.source, &edge.target] {
            if let Some(source) = source_by_path.get(path.as_str()) {
                add_target(
                    &mut candidates,
                    TargetInput {
                        path,
                        source,
                        rank: rank_by_path.get(path.as_str()).copied(),
                        evidence: ImpactEvidenceKind::Lexical,
                        priority: 3_000_000_000,
                        confidence: edge.confidence,
                        reason: "a retained lexical relationship connects this path to a changed path or changed symbol",
                        changed_symbols: changed_symbols.get(path),
                    },
                );
            }
        }
    }

    for root in &map.project_roots {
        for metadata in &root.manifest_metadata {
            for target in metadata.runtime_entry_points.iter().chain(&metadata.library_exports) {
                let Some(path) = &target.resolved_path else {
                    continue;
                };
                if !changed_paths.contains(path) {
                    continue;
                }
                relationships.push(ImpactRelationship {
                    source: metadata.path.clone(),
                    target: path.clone(),
                    evidence: ImpactEvidenceKind::Manifest,
                    confidence: ConfidenceTier::High,
                    reason: format!("the manifest declares `{}` as a target", target.declared),
                    symbol: target.name.clone(),
                    ambiguous: false,
                });
                if let Some(source) = source_by_path.get(path.as_str()) {
                    add_target(
                        &mut candidates,
                        TargetInput {
                            path,
                            source,
                            rank: rank_by_path.get(path.as_str()).copied(),
                            evidence: ImpactEvidenceKind::Manifest,
                            priority: 3_500_000_000,
                            confidence: ConfidenceTier::High,
                            reason: "a project manifest declares this changed path as a runtime or library target",
                            changed_symbols: changed_symbols.get(path),
                        },
                    );
                }
            }
        }
    }

    let tests = likely_tests(
        map,
        &changed_paths,
        &source_by_path,
        &rank_by_path,
        &mut candidates,
        &mut relationships,
    );
    let ownership = ownership_signals(map, &changed_paths);
    let history_complete = history.provenance.completeness.status == HistoryCompletenessStatus::Complete;
    let history = history_evidence(history, &changed_paths, &mut relationships);
    relationships.sort_by(|left, right| {
        left.evidence
            .cmp(&right.evidence)
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.symbol.cmp(&right.symbol))
    });
    relationships.dedup();
    let target_total = candidates.len();
    let relationship_total = relationships.len();
    let test_total = tests.len();
    let ownership_total = ownership.len();
    let history_total = history.len();

    let uncertainty = uncertainty(map, history_complete, &change_resolution);
    let mut report = ImpactReport {
        request,
        change_resolution,
        budget: ContextBudget { token_budget: budget, estimated_tokens: 0, truncated: false },
        ..ImpactReport::default()
    };

    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_by(|(left_path, left), (right_path, right)| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left_path.cmp(right_path))
    });
    for (path, candidate) in candidates {
        let target = ImpactTarget {
            path,
            symbols: candidate.symbols,
            evidence: candidate.evidence.into_iter().collect(),
            confidence: candidate.confidence,
            score: candidate.score,
            reason: candidate.reasons.into_iter().take(2).collect::<Vec<_>>().join("; "),
            limitations: candidate.limitations,
        };
        add_if_fits(&mut report, budget, |report| report.targets.push(target));
    }
    for relationship in relationships {
        add_if_fits(&mut report, budget, |report| report.relationships.push(relationship));
    }
    for test in tests {
        add_if_fits(&mut report, budget, |report| report.likely_tests.push(test));
    }
    for signal in ownership {
        add_if_fits(&mut report, budget, |report| report.ownership.push(signal));
    }
    for evidence in history {
        add_if_fits(&mut report, budget, |report| report.history.push(evidence));
    }
    for item in uncertainty {
        add_if_fits(&mut report, budget, |report| report.uncertainty.push(item));
    }

    report.budget.estimated_tokens = estimate_tokens(&report);
    report.budget.truncated = report.budget.estimated_tokens > budget
        || report.targets.len() < target_total
        || report.relationships.len() < relationship_total
        || report.likely_tests.len() < test_total
        || report.ownership.len() < ownership_total
        || report.history.len() < history_total;
    report
}

fn normalized_request(
    request: ContextRequest, map: &MapReport, change_resolution: &ChangeResolution,
) -> ContextRequest {
    ContextRequest {
        repository: map.repository_root.clone(),
        task: map.task_seeds.task.clone(),
        symbols: map.task_seeds.symbols.clone(),
        paths: map.task_seeds.paths.clone(),
        projects: map.task_seeds.projects.clone(),
        changes: map.task_seeds.changes.clone(),
        revision_context: request.revision_context,
        change_resolution: change_resolution.clone(),
        budget: request.budget,
        profile: request.profile,
        teaching: false,
    }
}

fn changed_paths(resolution: &ChangeResolution) -> BTreeSet<String> {
    resolution
        .changes
        .iter()
        .flat_map(|change| std::iter::once(change.path.clone()).chain(change.previous_path.clone()))
        .collect()
}

fn changed_symbols(resolution: &ChangeResolution) -> BTreeMap<String, BTreeSet<String>> {
    resolution
        .changes
        .iter()
        .map(|change| {
            (
                change.path.clone(),
                change
                    .symbols
                    .iter()
                    .map(|symbol| symbol.name.clone())
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect()
}

fn source_evidence(map: &MapReport) -> Vec<ReadingSourceEvidence> {
    if map.reading_evidence.sources.is_empty() {
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
            .collect()
    } else {
        map.reading_evidence.sources.clone()
    }
}

fn graph_evidence(map: &MapReport) -> Vec<LexicalEdge> {
    if map.reading_evidence.graph.is_empty() {
        map.edges.clone()
    } else {
        map.reading_evidence
            .graph
            .iter()
            .map(|relationship| relationship.relationship.clone())
            .collect()
    }
}

fn rank_evidence(map: &MapReport) -> &[FileRank] {
    if map.reading_evidence.ranking.is_empty() { &map.ranking } else { &map.reading_evidence.ranking }
}

fn add_target(candidates: &mut BTreeMap<String, TargetCandidate>, input: TargetInput<'_>) {
    let candidate = candidates.entry(input.path.to_owned()).or_default();
    candidate.evidence.insert(input.evidence);
    candidate.score = candidate
        .score
        .max(input.priority.saturating_add(input.rank.map_or(0, |rank| rank.score)));
    candidate.confidence = candidate.confidence.max(input.confidence);
    candidate.reasons.insert(input.reason.to_owned());
    if candidate.limitations.is_empty() {
        candidate.limitations = input.source.limitations.clone();
    }
    if candidate.symbols.is_empty() {
        let mut symbols = input.source.symbols.iter().collect::<Vec<_>>();
        symbols.sort_by(|left, right| {
            let left_changed = input.changed_symbols.is_some_and(|names| names.contains(&left.name));
            let right_changed = input.changed_symbols.is_some_and(|names| names.contains(&right.name));
            right_changed
                .cmp(&left_changed)
                .then_with(|| (right.role == SymbolRole::Definition).cmp(&(left.role == SymbolRole::Definition)))
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.location.start.line.cmp(&right.location.start.line))
        });
        candidate.symbols = symbols
            .into_iter()
            .take(MAX_SYMBOLS_PER_TARGET)
            .map(|symbol| ContextSymbol {
                path: input.path.to_owned(),
                symbol: symbol.clone(),
                score: input.rank.map_or(0, |rank| rank.score),
            })
            .collect();
    }
}

fn lexical_relationship(edge: &LexicalEdge) -> ImpactRelationship {
    ImpactRelationship {
        source: edge.source.clone(),
        target: edge.target.clone(),
        evidence: ImpactEvidenceKind::Lexical,
        confidence: edge.confidence,
        reason: format!(
            "a {} lexical reference resolved by {}",
            edge.symbol,
            edge.resolution_reason.label()
        ),
        symbol: Some(edge.symbol.clone()),
        ambiguous: edge.ambiguous,
    }
}

fn likely_tests(
    map: &MapReport, changed_paths: &BTreeSet<String>, source_by_path: &BTreeMap<&str, &ReadingSourceEvidence>,
    rank_by_path: &BTreeMap<&str, &FileRank>, candidates: &mut BTreeMap<String, TargetCandidate>,
    relationships: &mut Vec<ImpactRelationship>,
) -> Vec<ContextTest> {
    let mut tests = Vec::new();
    for source in source_by_path.values() {
        let Some(root) = map
            .landmarks
            .iter()
            .find(|landmark| landmark.kind == LandmarkKind::TestRoot && path_under(&source.path, &landmark.path))
        else {
            continue;
        };
        let shares_root = changed_paths
            .iter()
            .any(|path| same_project_root(map, path, &source.path));
        if !shares_root {
            continue;
        }
        let confidence = ConfidenceTier::Low;
        let reason = format!(
            "the path is under recognized test root `{}` in the changed project",
            root.path
        );
        add_target(
            candidates,
            TargetInput {
                path: &source.path,
                source,
                rank: rank_by_path.get(source.path.as_str()).copied(),
                evidence: ImpactEvidenceKind::Structural,
                priority: 1_000_000_000,
                confidence,
                reason: &reason,
                changed_symbols: None,
            },
        );
        if let Some(changed) = changed_paths
            .iter()
            .find(|path| same_project_root(map, path, &source.path))
        {
            relationships.push(ImpactRelationship {
                source: changed.clone(),
                target: source.path.clone(),
                evidence: ImpactEvidenceKind::Structural,
                confidence,
                reason: "both paths are in the same project root, and the target is under a recognized test root"
                    .to_owned(),
                symbol: None,
                ambiguous: false,
            });
        }
        tests.push(ContextTest { path: source.path.clone(), reason, confidence });
        if tests.len() == MAX_TESTS {
            break;
        }
    }
    tests
}

fn ownership_signals(map: &MapReport, changed_paths: &BTreeSet<String>) -> Vec<ImpactOwnershipSignal> {
    map.landmarks
        .iter()
        .filter(|landmark| landmark.kind == LandmarkKind::Ownership)
        .filter(|landmark| changed_paths.iter().any(|path| same_project_root(map, path, &landmark.path)))
        .take(MAX_OWNERSHIP_SIGNALS)
        .map(|landmark| ImpactOwnershipSignal {
            path: landmark.path.clone(),
            confidence: ConfidenceTier::Medium,
            reason: "recognized ownership configuration in the changed project; its rules were not semantically matched to individual paths".to_owned(),
        })
        .collect()
}

fn history_evidence(
    history: &HistoryReport, changed_paths: &BTreeSet<String>, relationships: &mut Vec<ImpactRelationship>,
) -> Vec<ImpactHistoryEvidence> {
    let mut evidence = Vec::new();
    for path in changed_paths {
        let churn = history
            .churn
            .as_ref()
            .and_then(|report| report.paths.iter().find(|candidate| candidate.path == *path));
        let bugs = history
            .bugs
            .as_ref()
            .and_then(|report| report.overlap_paths.iter().find(|candidate| candidate.path == *path));
        let commits = history
            .firefighting
            .as_ref()
            .map(|report| {
                report
                    .commits
                    .iter()
                    .filter(|commit| commit.paths.iter().any(|candidate| candidate == path))
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let Some(reason) = churn
            .map(|count| {
                format!(
                    "bounded churn history records {} commit(s) touching this changed path",
                    count.commits
                )
            })
            .or_else(|| {
                bugs.map(|count| {
                    format!(
                        "bounded bug-history overlap records {} commit(s) touching this changed path",
                        count.commits
                    )
                })
            })
            .or_else(|| {
                (!commits.is_empty())
                    .then(|| "bounded firefighting-language commits touched this changed path".to_owned())
            })
        else {
            continue;
        };
        relationships.push(ImpactRelationship {
            source: "history".to_owned(),
            target: path.clone(),
            evidence: ImpactEvidenceKind::History,
            confidence: ConfidenceTier::Medium,
            reason: reason.clone(),
            symbol: None,
            ambiguous: false,
        });
        evidence.push(ImpactHistoryEvidence {
            path: path.clone(),
            evidence: ImpactEvidenceKind::History,
            confidence: ConfidenceTier::Medium,
            reason,
            commits,
        });
        if evidence.len() == MAX_HISTORY_EVIDENCE {
            break;
        }
    }
    evidence
}

fn uncertainty(map: &MapReport, history_complete: bool, resolution: &ChangeResolution) -> Vec<ContextUncertainty> {
    let mut uncertainty = resolution
        .uncertainty
        .iter()
        .map(|item| ContextUncertainty { kind: format!("change_{}", item.kind), detail: item.detail.clone() })
        .collect::<Vec<_>>();
    uncertainty.push(ContextUncertainty {
        kind: "evidence_scope".to_owned(),
        detail: "Impact relationships are bounded lexical, structural, manifest, and history evidence. They do not establish semantic callers, callees, ownership matches, or breakage.".to_owned(),
    });
    if map.collections.edges.truncated || map.collections.ranking.truncated {
        uncertainty.push(ContextUncertainty {
            kind: "bounded_evidence".to_owned(),
            detail: "Some graph or ranking candidates were omitted by the active profile or resource limits."
                .to_owned(),
        });
    }
    if !history_complete {
        uncertainty.push(ContextUncertainty {
            kind: "incomplete_history".to_owned(),
            detail: "History evidence is incomplete for the selected repository state.".to_owned(),
        });
    }
    uncertainty.into_iter().take(MAX_UNCERTAINTIES).collect()
}

fn same_project_root(map: &MapReport, left: &str, right: &str) -> bool {
    let root_for = |path: &str| {
        map.project_roots
            .iter()
            .filter(|root| root.path == "." || path_under(path, &root.path))
            .max_by_key(|root| root.path.len())
            .map(|root| root.path.as_str())
    };
    root_for(left) == root_for(right)
}

fn path_under(path: &str, root: &str) -> bool {
    root == "." || path == root || path.strip_prefix(root).is_some_and(|suffix| suffix.starts_with('/'))
}

fn add_if_fits(report: &mut ImpactReport, budget: usize, add: impl FnOnce(&mut ImpactReport)) -> bool {
    let before = report.clone();
    add(report);
    if estimate_tokens(report) > budget {
        *report = before;
        return false;
    }
    true
}

fn estimate_tokens(report: &ImpactReport) -> usize {
    let mut selected = report.clone();
    selected.budget.estimated_tokens = 0;
    serde_json::to_string(&selected).map_or(usize::MAX, |json| token_count(&json))
}
