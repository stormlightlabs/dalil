use std::collections::BTreeSet;

use super::*;
use crate::utils::token_count;

const MAX_CONTEXT_SYMBOLS_PER_FILE: usize = 3;
const MAX_CONTEXT_LANDMARKS: usize = 4;
const MAX_CONTEXT_PROJECT_ROOTS: usize = 4;
const MAX_CONTEXT_RISKS: usize = 4;
const MAX_CONTEXT_UNCERTAINTIES: usize = 4;
const MAX_CONTEXT_OMISSIONS: usize = 8;

/// Compose selected repository evidence into the task-level result used by the
/// `context` command. The map and history reports remain implementation inputs
/// and are not attached to the returned report.
pub fn compile(request: ContextRequest, map: &MapReport, history: &HistoryReport) -> ContextBundle {
    let plan = super::analysis::build_reading_plan(history, map);
    let task_seeds = map.task_seeds.clone();
    let request = ContextRequest {
        repository: map.repository_root.clone(),
        task: task_seeds.task.clone(),
        symbols: task_seeds.symbols.clone(),
        paths: task_seeds.paths.clone(),
        projects: task_seeds.projects.clone(),
        changes: task_seeds.changes.clone(),
        revision_context: request.revision_context,
        budget: request.budget,
        profile: request.profile,
    };
    let orientation = ContextOrientation {
        repository_root: map.repository_root.clone(),
        scope_path: map.scope_path.clone(),
        worktree: map.worktree.state,
        primary_languages: if plan.primary_languages.is_empty() {
            map.selection.primary_languages.clone()
        } else {
            plan.primary_languages.clone()
        },
        project_roots: map
            .project_roots
            .iter()
            .take(MAX_CONTEXT_PROJECT_ROOTS)
            .map(|root| root.path.clone())
            .collect(),
        landmarks: map
            .landmarks
            .iter()
            .filter(|landmark| {
                matches!(
                    landmark.kind,
                    LandmarkKind::AgentInstructions
                        | LandmarkKind::ContributorInstructions
                        | LandmarkKind::Readme
                        | LandmarkKind::Manifest
                        | LandmarkKind::WorkspaceRoot
                        | LandmarkKind::PackageRoot
                )
            })
            .take(MAX_CONTEXT_LANDMARKS)
            .cloned()
            .collect(),
    };
    let mut bundle = ContextBundle {
        request,
        orientation,
        provenance: ContextProvenance {
            head: map.head.clone(),
            cache: CacheProvenance {
                mode: map.cache.mode,
                status: map.cache.status,
                available: map.cache.mode != CacheMode::Disabled,
                hits: map.cache.hits,
                misses: map.cache.misses,
                refreshed: map.cache.refreshed.len(),
                stale: map.cache.stale.len(),
            },
            task_seeds,
            history_complete: history.provenance.completeness.status == HistoryCompletenessStatus::Complete,
        },
        budget: ContextBudget { token_budget: request_budget(&map.selection), estimated_tokens: 0, truncated: false },
        ..ContextBundle::default()
    };
    bundle.budget.token_budget = bundle.request.budget;

    let budget = bundle.request.budget;
    for uncertainty in uncertainty_candidates(map, history)
        .into_iter()
        .take(MAX_CONTEXT_UNCERTAINTIES)
    {
        add_if_fits(&mut bundle, budget, |bundle| bundle.uncertainty.push(uncertainty));
    }
    for risk in risk_candidates(map).into_iter().take(MAX_CONTEXT_RISKS) {
        add_if_fits(&mut bundle, budget, |bundle| bundle.risks.push(risk));
    }

    for recommendation in &plan.recommendations {
        let file = context_file(recommendation, map);
        add_if_fits(&mut bundle, budget, |bundle| bundle.files.push(file));
    }

    let selected_paths = bundle
        .files
        .iter()
        .map(|file| file.recommendation.path.clone())
        .collect::<BTreeSet<_>>();
    for recommendation in plan
        .recommendations
        .iter()
        .filter(|recommendation| recommendation.purpose == ReadingPurpose::Tests)
        .filter(|recommendation| selected_paths.contains(&recommendation.path))
    {
        let test = ContextTest {
            path: recommendation.path.clone(),
            reason: recommendation.reason.clone(),
            confidence: recommendation.confidence,
        };
        add_if_fits(&mut bundle, budget, |bundle| bundle.relevant_tests.push(test));
    }

    for relationship in map
        .reading_evidence
        .graph
        .iter()
        .map(|edge| edge.relationship.clone())
        .filter(|edge| selected_paths.contains(&edge.source) || selected_paths.contains(&edge.target))
    {
        add_if_fits(&mut bundle, budget, |bundle| bundle.relationships.push(relationship));
    }

    for observation in &history.observations {
        let observation = observation.clone();
        add_if_fits(&mut bundle, budget, |bundle| bundle.history.push(observation));
    }

    for omission in omission_candidates(map) {
        add_if_fits(&mut bundle, budget, |bundle| bundle.omissions.push(omission));
    }

    let selected_paths = bundle
        .files
        .iter()
        .map(|file| file.recommendation.path.clone())
        .collect::<BTreeSet<_>>();
    for next_read in plan
        .recommendations
        .iter()
        .filter(|recommendation| !selected_paths.contains(&recommendation.path))
    {
        let next_read = next_read.clone();
        add_if_fits(&mut bundle, budget, |bundle| bundle.next_reads.push(next_read));
    }

    let estimated_tokens = estimate_tokens(&bundle);
    bundle.budget.estimated_tokens = estimated_tokens;
    bundle.budget.truncated = estimated_tokens > budget
        || bundle.files.len() < plan.recommendations.len()
        || bundle.relationships.len()
            < map
                .reading_evidence
                .graph
                .iter()
                .filter(|edge| selected_paths.contains(&edge.source) || selected_paths.contains(&edge.target))
                .count()
        || bundle.next_reads.len()
            < plan
                .recommendations
                .iter()
                .filter(|recommendation| !selected_paths.contains(&recommendation.path))
                .count()
        || map.collections.snippets.truncated
        || map.collections.edges.truncated
        || history.collections.commits.truncated;
    bundle
}

fn request_budget(selection: &MapSelection) -> usize {
    selection.token_budget
}

fn context_file(recommendation: &ReadingRecommendation, map: &MapReport) -> ContextFile {
    let ranking = map
        .reading_evidence
        .ranking
        .iter()
        .find(|rank| rank.path == recommendation.path)
        .or_else(|| map.ranking.iter().find(|rank| rank.path == recommendation.path))
        .cloned();
    let snippets = map
        .selection
        .snippets
        .iter()
        .filter(|snippet| snippet.path == recommendation.path)
        .cloned()
        .collect::<Vec<_>>();
    let mut symbols = snippets
        .iter()
        .map(|snippet| ContextSymbol {
            path: snippet.path.clone(),
            symbol: snippet.symbol.clone(),
            score: snippet.score,
        })
        .collect::<Vec<_>>();
    let source_symbols = map
        .reading_evidence
        .sources
        .iter()
        .find(|source| source.path == recommendation.path)
        .map(|source| source.symbols.as_slice())
        .or_else(|| {
            map.files
                .iter()
                .find(|file| file.path == recommendation.path)
                .map(|file| file.symbols.as_slice())
        })
        .unwrap_or_default();
    let fallback_score = ranking.as_ref().map_or(0, |rank| rank.score);
    for symbol in source_symbols {
        if symbols
            .iter()
            .any(|candidate| candidate.symbol.name == symbol.name && candidate.symbol.location == symbol.location)
        {
            continue;
        }
        symbols.push(ContextSymbol {
            path: recommendation.path.clone(),
            symbol: symbol.clone(),
            score: fallback_score,
        });
        if symbols.len() >= MAX_CONTEXT_SYMBOLS_PER_FILE {
            break;
        }
    }
    symbols.truncate(MAX_CONTEXT_SYMBOLS_PER_FILE);
    ContextFile { recommendation: recommendation.clone(), ranking, symbols, snippets }
}

fn uncertainty_candidates(map: &MapReport, history: &HistoryReport) -> Vec<ContextUncertainty> {
    let mut uncertainty = map
        .limitations
        .iter()
        .map(|detail| ContextUncertainty { kind: "map_limitation".to_owned(), detail: detail.clone() })
        .collect::<Vec<_>>();
    if map.collections.snippets.truncated || map.collections.edges.truncated || map.collections.ranking.truncated {
        uncertainty.push(ContextUncertainty {
            kind: "bounded_evidence".to_owned(),
            detail:
                "The selected source, relationship, or ranking evidence was projected by the active profile or budget."
                    .to_owned(),
        });
    }
    if history.provenance.completeness.status != HistoryCompletenessStatus::Complete {
        uncertainty.push(ContextUncertainty {
            kind: "incomplete_history".to_owned(),
            detail: "History evidence is incomplete for the selected repository state.".to_owned(),
        });
    }
    uncertainty.extend(
        history
            .limitations
            .iter()
            .map(|detail| ContextUncertainty { kind: "history_limitation".to_owned(), detail: detail.clone() }),
    );
    uncertainty
}

fn risk_candidates(map: &MapReport) -> Vec<ContextRisk> {
    let mut risks = map
        .findings
        .iter()
        .map(|finding| ContextRisk {
            kind: finding.kind.label().to_owned(),
            detail: finding.detail.clone(),
            paths: vec![finding.path.clone()],
        })
        .collect::<Vec<_>>();
    if map.cache.status == CacheStatus::Stale || !map.cache.stale.is_empty() {
        risks.push(ContextRisk {
            kind: "stale_cache".to_owned(),
            detail: "Some source evidence came from cache records that could not be refreshed.".to_owned(),
            paths: map.cache.stale.clone(),
        });
    }
    risks
}

fn omission_candidates(map: &MapReport) -> Vec<ContextOmission> {
    map.selection
        .omitted_relevant_paths
        .iter()
        .map(|omission| ContextOmission { path: omission.path.clone(), reason: omission.reason.clone() })
        .chain(
            map.omissions
                .iter()
                .filter(|omission| omission.reason != OmissionReason::NonSource)
                .map(|omission| ContextOmission { path: omission.path.clone(), reason: omission.detail.clone() }),
        )
        .take(MAX_CONTEXT_OMISSIONS)
        .collect()
}

fn add_if_fits(bundle: &mut ContextBundle, budget: usize, add: impl FnOnce(&mut ContextBundle)) -> bool {
    let before = bundle.clone();
    add(bundle);
    if estimate_tokens(bundle) > budget {
        *bundle = before;
        return false;
    }
    true
}

fn estimate_tokens(bundle: &ContextBundle) -> usize {
    let mut selected = bundle.clone();
    selected.budget.estimated_tokens = 0;
    // Cache status explains provenance but is not selected task evidence. Its
    // changing vocabulary must not alter an otherwise equivalent bundle's budget.
    selected.provenance.cache = CacheProvenance::default();
    serde_json::to_string(&selected).map_or(usize::MAX, |json| token_count(&json))
}
