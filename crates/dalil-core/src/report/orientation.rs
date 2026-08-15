use std::collections::BTreeSet;

use super::*;

const MAX_ORIENTATION_ROOTS: usize = 4;
const MAX_ORIENTATION_HISTORY: usize = 5;
const MAX_ORIENTATION_UNCERTAINTY: usize = 6;

/// Build the first-read report directly from selected map and history evidence.
/// The complete analysis reports are inputs only and are never embedded here.
pub fn compile(history: &HistoryReport, map: &MapReport, plan: &ReadingPlan) -> OrientationReport {
    let selected_roots = plan
        .recommendations
        .iter()
        .filter_map(|recommendation| recommendation.project_root.as_deref())
        .collect::<BTreeSet<_>>();
    let important_roots = map
        .project_roots
        .iter()
        .filter(|root| selected_roots.contains(root.path.as_str()) || root.path == ".")
        .chain(
            map.project_roots
                .iter()
                .filter(|root| !selected_roots.contains(root.path.as_str()) && root.path != "."),
        )
        .take(MAX_ORIENTATION_ROOTS)
        .map(|root| OrientationRoot { path: root.path.clone(), kind: root.kind, reason: root.reason.clone() })
        .collect();

    let mut uncertainty = map
        .limitations
        .iter()
        .cloned()
        .map(|detail| OrientationUncertainty { kind: "map_limitation".to_owned(), detail })
        .collect::<Vec<_>>();
    if map.collections.snippets.truncated || map.collections.edges.truncated || map.collections.ranking.truncated {
        uncertainty.push(OrientationUncertainty {
            kind: "bounded_evidence".to_owned(),
            detail: "Some source, relationship, or ranking evidence was projected by the active profile or budget."
                .to_owned(),
        });
    }
    if map.availability.unsupported_paths > 0 {
        uncertainty.push(OrientationUncertainty {
            kind: "unsupported_source".to_owned(),
            detail: format!(
                "{} source-like path(s) use languages Dalil does not support.",
                map.availability.unsupported_paths
            ),
        });
    }
    if map.availability.partial_files > 0 {
        uncertainty.push(OrientationUncertainty {
            kind: "partial_source".to_owned(),
            detail: format!(
                "{} selected-scope source file(s) have bounded or incomplete structural evidence.",
                map.availability.partial_files
            ),
        });
    }
    if let Some(shortfall) = &plan.shortfall {
        uncertainty.push(OrientationUncertainty {
            kind: "reading_plan_shortfall".to_owned(),
            detail: format!(
                "Only {} of {} useful first reads were selected: {}",
                shortfall.returned, shortfall.target_minimum, shortfall.reason
            ),
        });
    }
    if history.provenance.completeness.status != HistoryCompletenessStatus::Complete {
        uncertainty.push(OrientationUncertainty {
            kind: "incomplete_history".to_owned(),
            detail: "History evidence is incomplete for the selected repository state.".to_owned(),
        });
    }
    uncertainty.extend(
        history
            .limitations
            .iter()
            .cloned()
            .map(|detail| OrientationUncertainty { kind: "history_limitation".to_owned(), detail }),
    );
    uncertainty.truncate(MAX_ORIENTATION_UNCERTAINTY);

    OrientationReport {
        repository: OrientationRepository {
            root: map.repository_root.clone(),
            scope_path: map.scope_path.clone(),
            head: map.head.clone(),
            worktree: map.worktree.state,
            primary_languages: if plan.primary_languages.is_empty() {
                map.selection.primary_languages.clone()
            } else {
                plan.primary_languages.clone()
            },
        },
        starting_points: recommendations_for(plan, &[ReadingPurpose::StartHere, ReadingPurpose::Architecture]),
        important_roots,
        runtime_entry_points: recommendations_for(plan, &[ReadingPurpose::Runtime]),
        tests: recommendations_for(plan, &[ReadingPurpose::Tests]),
        history: history
            .observations
            .iter()
            .take(MAX_ORIENTATION_HISTORY)
            .cloned()
            .collect(),
        next_reads: recommendations_for(plan, &[ReadingPurpose::SupportingContext]),
        uncertainty,
    }
}

fn recommendations_for(plan: &ReadingPlan, purposes: &[ReadingPurpose]) -> Vec<ReadingRecommendation> {
    plan.recommendations
        .iter()
        .filter(|recommendation| purposes.contains(&recommendation.purpose))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_uses_unique_recommendations_across_reading_categories() {
        let recommendation = |ordinal: usize, purpose: ReadingPurpose, path: &str| ReadingRecommendation {
            ordinal,
            purpose,
            path: path.to_owned(),
            project_root: Some(".".to_owned()),
            reason: "fixture".to_owned(),
            evidence_kinds: vec![ReadingEvidenceKind::Landmark],
            confidence: ConfidenceTier::High,
            limitations: Vec::new(),
        };
        let plan = ReadingPlan {
            recommendations: vec![
                recommendation(1, ReadingPurpose::StartHere, "README.md"),
                recommendation(2, ReadingPurpose::Runtime, "src/main.rs"),
                recommendation(3, ReadingPurpose::Tests, "tests/cli.rs"),
                recommendation(4, ReadingPurpose::SupportingContext, "Cargo.toml"),
            ],
            ..ReadingPlan::default()
        };
        let map: MapReport = serde_json::from_value(serde_json::json!({
            "repository_root": "/fixture",
            "scope_path": ".",
            "query_pack": "rust-v1",
            "exclusions": [],
            "inventory": {"tracked": 1, "modified": 0, "untracked": 0, "analyzed": 1, "omitted": 0},
            "files": [], "omissions": [], "findings": [], "limitations": [], "edges": [], "ranking": [],
            "selection": {"token_budget": 1000, "estimated_tokens": 0, "snippets": []},
            "cache": {"mode": "disabled", "status": "disabled", "hits": 0, "misses": 0, "refreshed": [], "stale": []},
            "project_roots": [{"path": ".", "kind": "workspace", "reason": "fixture", "manifests": []}]
        }))
        .expect("fixture map is valid");
        let history = HistoryReport {
            repository_root: "/fixture".to_owned(),
            scope_path: ".".to_owned(),
            head: HeadSnapshot::default(),
            provenance: HistoryProvenance::default(),
            settings: HistorySettings::default(),
            commits_seen: 0,
            non_merge_commits_seen: 0,
            collections: HistoryCollections::default(),
            limitations: Vec::new(),
            observations: Vec::new(),
            churn: None,
            contributors: None,
            bugs: None,
            activity: None,
            firefighting: None,
        };
        let orientation = compile(&history, &map, &plan);
        let paths = orientation
            .starting_points
            .iter()
            .chain(&orientation.runtime_entry_points)
            .chain(&orientation.tests)
            .chain(&orientation.next_reads)
            .map(|recommendation| recommendation.path.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(orientation.read_count(), paths.len());
        assert_eq!(orientation.important_roots[0].path, ".");
    }
}
