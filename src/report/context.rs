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
pub fn compile(
    request: ContextRequest, map: &MapReport, history: &HistoryReport, change_resolution: ChangeResolution,
) -> ContextBundle {
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
        change_resolution: change_resolution.clone(),
        budget: request.budget,
        profile: request.profile,
        teaching: request.teaching,
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
        change_resolution,
        provenance: ContextProvenance {
            head: map.head.clone(),
            cache: CacheProvenance {
                mode: map.cache.mode,
                status: map.cache.status,
                index_status: map.cache.index_status,
                index_detail: map.cache.index_detail.clone(),
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
    let mut recommendations = plan.recommendations.iter().collect::<Vec<_>>();
    if bundle.request.teaching {
        recommendations.sort_by_key(|recommendation| teaching_recommendation_order(recommendation.purpose));
    }
    let mut uncertainty = uncertainty_candidates(map, history);
    uncertainty.extend(
        bundle
            .change_resolution
            .uncertainty
            .iter()
            .map(|uncertainty| ContextUncertainty {
                kind: format!("change_{}", uncertainty.kind),
                detail: uncertainty.detail.clone(),
            }),
    );
    let uncertainty = uncertainty
        .into_iter()
        .take(MAX_CONTEXT_UNCERTAINTIES)
        .collect::<Vec<_>>();
    let risks = risk_candidates(map)
        .into_iter()
        .take(MAX_CONTEXT_RISKS)
        .collect::<Vec<_>>();
    if !bundle.request.teaching {
        for uncertainty in uncertainty.clone() {
            add_if_fits(&mut bundle, budget, |bundle| bundle.uncertainty.push(uncertainty));
        }
        for risk in risks.clone() {
            add_if_fits(&mut bundle, budget, |bundle| bundle.risks.push(risk));
        }
    }

    for recommendation in &recommendations {
        let file = context_file(recommendation, map);
        let added = add_if_fits(&mut bundle, budget, |bundle| bundle.files.push(file));
        if bundle.request.teaching && added && recommendation.purpose == ReadingPurpose::Runtime {
            break;
        }
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

    let teaching_projected = if bundle.request.teaching { add_teaching_scaffold(&mut bundle, budget) } else { false };

    let selected_paths = bundle
        .files
        .iter()
        .map(|file| file.recommendation.path.clone())
        .collect::<BTreeSet<_>>();
    for next_read in recommendations
        .iter()
        .filter(|recommendation| !selected_paths.contains(&recommendation.path))
    {
        let next_read = (*next_read).clone();
        add_if_fits(&mut bundle, budget, |bundle| bundle.next_reads.push(next_read));
    }
    if bundle.request.teaching {
        for uncertainty in uncertainty {
            add_if_fits(&mut bundle, budget, |bundle| bundle.uncertainty.push(uncertainty));
        }
        for risk in risks {
            add_if_fits(&mut bundle, budget, |bundle| bundle.risks.push(risk));
        }
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
        || history.collections.commits.truncated
        || teaching_projected;
    bundle
}

fn teaching_recommendation_order(purpose: ReadingPurpose) -> u8 {
    match purpose {
        ReadingPurpose::Runtime => 0,
        ReadingPurpose::Architecture => 1,
        ReadingPurpose::Tests => 2,
        ReadingPurpose::StartHere => 3,
        ReadingPurpose::SupportingContext => 4,
    }
}

fn add_teaching_scaffold(bundle: &mut ContextBundle, budget: usize) -> bool {
    let had_scaffold = teaching_scaffold(bundle).is_some();
    let mut projected = false;
    loop {
        if let Some(scaffold) = teaching_scaffold(bundle)
            && add_if_fits(bundle, budget, |bundle| bundle.teaching = Some(scaffold))
        {
            return projected;
        }

        // A requested scaffold takes precedence over supplemental context. Do
        // not remove files because each teaching observation must still point
        // to selected source evidence.
        if bundle.omissions.pop().is_some()
            || bundle.history.pop().is_some()
            || bundle.risks.pop().is_some()
            || bundle.uncertainty.pop().is_some()
        {
            projected = true;
            continue;
        }
        return had_scaffold || projected;
    }
}

/// Build teaching guidance only from the evidence already returned in `bundle`.
/// The observations name direct evidence; reading order is marked separately so
/// a lexical relationship cannot be mistaken for proven runtime control flow.
fn teaching_scaffold(bundle: &ContextBundle) -> Option<TeachingScaffold> {
    let mut steps = Vec::new();
    let runtime_files = bundle
        .files
        .iter()
        .filter(|file| file.recommendation.purpose == ReadingPurpose::Runtime)
        .collect::<Vec<_>>();
    match runtime_files.as_slice() {
        [file] => steps.push(TeachingStep {
            topic: TeachingTopic::BehaviorStart,
            explanation: format!(
                "Read `{}` first. It is the only returned runtime target for this bundle.",
                file.recommendation.path
            ),
            observed: vec![TeachingEvidence {
                kind: TeachingEvidenceKind::File,
                path: file.recommendation.path.clone(),
                symbol: None,
            }],
            ordering: TeachingOrdering::Inferred,
        }),
        [] => {}
        files => steps.push(TeachingStep {
            topic: TeachingTopic::BehaviorStart,
            explanation: format!(
                "The bundle has {} returned runtime targets. It does not establish one behavior start for this task.",
                files.len()
            ),
            observed: files
                .iter()
                .map(|file| TeachingEvidence {
                    kind: TeachingEvidenceKind::File,
                    path: file.recommendation.path.clone(),
                    symbol: None,
                })
                .collect(),
            ordering: TeachingOrdering::Ambiguous,
        }),
    }

    if let Some(relationship) = bundle.relationships.iter().find(|relationship| !relationship.ambiguous) {
        steps.push(TeachingStep {
            topic: TeachingTopic::ControlFlow,
            explanation: format!(
                "Read `{}` before `{}`. The bundle observed a lexical relationship via `{}`.",
                relationship.source, relationship.target, relationship.symbol
            ),
            observed: vec![TeachingEvidence {
                kind: TeachingEvidenceKind::Relationship,
                path: relationship.source.clone(),
                symbol: Some(relationship.symbol.clone()),
            }],
            ordering: TeachingOrdering::Inferred,
        });
    }

    if let Some(symbol) = bundle
        .files
        .iter()
        .filter(|file| file.recommendation.purpose != ReadingPurpose::Tests)
        .flat_map(|file| file.symbols.iter())
        .find(|symbol| {
            symbol.symbol.role == SymbolRole::Definition
                && matches!(
                    symbol.symbol.kind,
                    SymbolKind::Struct
                        | SymbolKind::Enum
                        | SymbolKind::Type
                        | SymbolKind::Class
                        | SymbolKind::Interface
                        | SymbolKind::Field
                        | SymbolKind::Static
                        | SymbolKind::Variable
                )
        })
    {
        steps.push(TeachingStep {
            topic: TeachingTopic::StateOrDataBoundary,
            explanation: format!(
                "Inspect `{}` in `{}`. Its {} declaration is a possible data boundary.",
                symbol.symbol.name,
                symbol.path,
                symbol.symbol.kind.label()
            ),
            observed: vec![TeachingEvidence {
                kind: TeachingEvidenceKind::Symbol,
                path: symbol.path.clone(),
                symbol: Some(symbol.symbol.name.clone()),
            }],
            ordering: TeachingOrdering::Inferred,
        });
    }

    if let Some(test) = bundle.relevant_tests.first() {
        steps.push(TeachingStep {
            topic: TeachingTopic::RelevantTests,
            explanation: format!("Read `{}` to inspect the selected test evidence.", test.path),
            observed: vec![TeachingEvidence {
                kind: TeachingEvidenceKind::Test,
                path: test.path.clone(),
                symbol: None,
            }],
            ordering: TeachingOrdering::Inferred,
        });
    }

    if let Some(next_read) = bundle.next_reads.first() {
        steps.push(TeachingStep {
            topic: TeachingTopic::NextRead,
            explanation: format!("Then read `{}` for the next selected lead.", next_read.path),
            observed: vec![TeachingEvidence {
                kind: TeachingEvidenceKind::NextRead,
                path: next_read.path.clone(),
                symbol: None,
            }],
            ordering: TeachingOrdering::Inferred,
        });
    }

    (!steps.is_empty()).then_some(TeachingScaffold { steps })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn recommendation(path: &str, purpose: ReadingPurpose) -> ReadingRecommendation {
        ReadingRecommendation {
            ordinal: 1,
            purpose,
            path: path.to_owned(),
            project_root: Some(".".to_owned()),
            reason: format!("fixture evidence selected `{path}`"),
            evidence_kinds: vec![ReadingEvidenceKind::SourceMap],
            confidence: ConfidenceTier::High,
            limitations: Vec::new(),
        }
    }

    fn context_file(path: &str, purpose: ReadingPurpose, symbols: Vec<ContextSymbol>) -> ContextFile {
        ContextFile { recommendation: recommendation(path, purpose), ranking: None, symbols, snippets: Vec::new() }
    }

    fn data_symbol(path: &str) -> ContextSymbol {
        ContextSymbol {
            path: path.to_owned(),
            symbol: SourceSymbol {
                name: "RequestState".to_owned(),
                kind: SymbolKind::Struct,
                role: SymbolRole::Definition,
                scope: Vec::new(),
                location: SourceLocation {
                    start: Position { line: 3, column: 1 },
                    end: Position { line: 3, column: 20 },
                },
                context: "struct RequestState;".to_owned(),
                visibility: SymbolVisibility::Public,
                evidence: SymbolEvidence::Declaration,
            },
            score: 1,
        }
    }

    #[test]
    fn teaching_fixture_clear_entry_flow_uses_only_returned_evidence() {
        let bundle = ContextBundle {
            files: vec![
                context_file("src/main.rs", ReadingPurpose::Runtime, Vec::new()),
                context_file(
                    "src/service.rs",
                    ReadingPurpose::Architecture,
                    vec![data_symbol("src/service.rs")],
                ),
            ],
            relationships: vec![LexicalEdge {
                source: "src/main.rs".to_owned(),
                target: "src/service.rs".to_owned(),
                symbol: "run".to_owned(),
                ambiguous: false,
                candidates: vec!["src/service.rs".to_owned()],
                candidate_group: "fixture-run".to_owned(),
                resolution_reason: LexicalResolutionReason::SameModule,
                confidence: ConfidenceTier::High,
                target_visibility: SymbolVisibility::Public,
            }],
            relevant_tests: vec![ContextTest {
                path: "tests/service.rs".to_owned(),
                reason: "the path is inside the recognized test root tests".to_owned(),
                confidence: ConfidenceTier::High,
            }],
            next_reads: vec![recommendation("src/config.rs", ReadingPurpose::SupportingContext)],
            ..ContextBundle::default()
        };

        let scaffold = teaching_scaffold(&bundle).expect("clear fixture has teaching evidence");
        assert_eq!(
            scaffold.steps.iter().map(|step| step.topic).collect::<Vec<_>>(),
            vec![
                TeachingTopic::BehaviorStart,
                TeachingTopic::ControlFlow,
                TeachingTopic::StateOrDataBoundary,
                TeachingTopic::RelevantTests,
                TeachingTopic::NextRead,
            ]
        );
        assert_eq!(scaffold.steps[0].ordering, TeachingOrdering::Inferred);
        assert_eq!(scaffold.steps[1].ordering, TeachingOrdering::Inferred);
        assert_eq!(scaffold.steps[2].ordering, TeachingOrdering::Inferred);
        assert!(scaffold.steps.iter().flat_map(|step| &step.observed).all(|evidence| {
            ["src/main.rs", "src/service.rs", "tests/service.rs", "src/config.rs"].contains(&evidence.path.as_str())
        }));
    }

    #[test]
    fn teaching_fixture_multiple_entry_points_marks_the_start_ambiguous() {
        let bundle = ContextBundle {
            files: vec![
                context_file("src/bin/worker.rs", ReadingPurpose::Runtime, Vec::new()),
                context_file("src/bin/server.rs", ReadingPurpose::Runtime, Vec::new()),
            ],
            ..ContextBundle::default()
        };

        let scaffold = teaching_scaffold(&bundle).expect("runtime candidates produce a teaching step");
        let start = scaffold.steps.first().expect("behavior start step");
        assert_eq!(start.topic, TeachingTopic::BehaviorStart);
        assert_eq!(start.ordering, TeachingOrdering::Ambiguous);
        assert_eq!(start.observed.len(), 2);
        assert!(start.explanation.contains("does not establish one behavior start"));
    }

    #[test]
    fn teaching_fixture_insufficient_evidence_omits_the_scaffold() {
        assert!(teaching_scaffold(&ContextBundle::default()).is_none());
    }
}
