use std::fmt::Write;

use super::Render;
use crate::utils;

impl Render {
    pub fn explain_markdown(output: &mut String, explain: &dalil_core::ExplainReport) {
        Render::section_heading(output, "Recommendation explanation");
        writeln!(
            output,
            "Target: `{}` ({})",
            utils::escape_inline_code(&explain.target),
            explain.target_kind.label()
        )
        .expect("writing to a string cannot fail");
        if !explain.matched_paths.is_empty() {
            writeln!(
                output,
                "Matched paths: {}",
                utils::inline_code_list(&explain.matched_paths)
            )
            .expect("writing to a string cannot fail");
        }
        if !explain.matched_symbols.is_empty() {
            writeln!(output, "Matched symbols:").expect("writing to a string cannot fail");
            for matched in &explain.matched_symbols {
                writeln!(
                    output,
                    "- `{}` — {} `{}` at {} in `{}` — `{}`",
                    utils::escape_inline_code(&matched.path),
                    matched.symbol.kind.label(),
                    utils::escape_inline_code(&matched.symbol.name),
                    Self::format_location(&matched.symbol.location),
                    utils::escape_inline_code(&matched.symbol.scope.join("::")),
                    utils::escape_inline_code(&matched.symbol.context),
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !explain.focus_matches.is_empty() {
            writeln!(
                output,
                "Focus evidence: {}",
                utils::inline_code_list(&explain.focus_matches)
            )
            .expect("writing to a string cannot fail");
        }
        if let Some(landmark) = &explain.landmark {
            writeln!(
                output,
                "Landmark: **{}** `{}` — {}",
                landmark.kind,
                utils::escape_inline_code(&landmark.path),
                utils::sanitize_text(&landmark.reason)
            )
            .expect("writing to a string cannot fail");
        }

        writeln!(output, "Provenance:").expect("writing to a string cannot fail");
        writeln!(
            output,
            "- profile `{}`; {} analyzed source file(s); {} retained lexical relationship(s); history scope `{}`",
            match explain.provenance.profile {
                dalil_core::AnalysisProfile::Compact => "compact",
                dalil_core::AnalysisProfile::Evidence => "evidence",
            },
            explain.provenance.source_files_analyzed,
            explain.provenance.retained_relationships,
            utils::escape_inline_code(&explain.provenance.history_scope),
        )
        .expect("writing to a string cannot fail");
        let task_seeds = Self::explain_task_seeds(&explain.provenance.task_seeds);
        if task_seeds.is_empty() {
            writeln!(output, "- no task seeds were supplied or derived").expect("writing to a string cannot fail");
        } else {
            writeln!(output, "- task seeds: {}", task_seeds.join("; ")).expect("writing to a string cannot fail");
        }

        if !explain.guidance.is_empty() {
            writeln!(output, "Reading guidance:").expect("writing to a string cannot fail");
            for guidance in &explain.guidance {
                writeln!(
                    output,
                    "- `{}` — {} ({} confidence)",
                    utils::escape_inline_code(&guidance.path),
                    utils::sanitize_text(&guidance.why_read),
                    guidance.confidence.label(),
                )
                .expect("writing to a string cannot fail");
                if let Some(ranking) = &guidance.ranking {
                    Self::explain_ranking_markdown(output, ranking, "  ");
                }
                for relationship in &guidance.relationships {
                    Self::explain_relationship_markdown(output, relationship, "  ");
                }
                for context in &guidance.recent_commits {
                    writeln!(
                        output,
                        "  - {} history: `{}` — {} ({}) — {}",
                        context.evidence_kind.label(),
                        utils::escape_inline_code(&context.commit.id),
                        utils::sanitize_text(&context.commit.subject),
                        context
                            .commit
                            .paths
                            .iter()
                            .map(|path| format!("`{}`", utils::escape_inline_code(path)))
                            .collect::<Vec<_>>()
                            .join(", "),
                        utils::sanitize_text(&context.reason),
                    )
                    .expect("writing to a string cannot fail");
                }
                for finding in &guidance.ambiguity {
                    writeln!(
                        output,
                        "  - ambiguity **{}** in `{}`: {}",
                        finding.kind.label(),
                        utils::escape_inline_code(&finding.path),
                        utils::sanitize_text(&finding.detail),
                    )
                    .expect("writing to a string cannot fail");
                }
                for omission in &guidance.omissions {
                    Self::explain_omission_markdown(output, omission, "  ");
                }
                for truncation in &guidance.truncation {
                    writeln!(
                        output,
                        "  - {} evidence: {} of {} returned{} — {}",
                        utils::sanitize_text(&truncation.evidence),
                        truncation.returned,
                        truncation.total,
                        truncation
                            .reason
                            .map(|reason| format!(" ({reason:?})"))
                            .unwrap_or_default(),
                        utils::sanitize_text(&truncation.detail),
                    )
                    .expect("writing to a string cannot fail");
                }
                for limitation in &guidance.limitations {
                    writeln!(output, "  - limitation: {}", utils::sanitize_text(limitation))
                        .expect("writing to a string cannot fail");
                }
            }
        }

        if let Some(next_read) = &explain.next_read {
            writeln!(output, "Next read:").expect("writing to a string cannot fail");
            writeln!(
                output,
                "- `{}` ({}, {} confidence) — {}; evidence: {}",
                utils::escape_inline_code(&next_read.path),
                next_read.purpose.label(),
                next_read.confidence.label(),
                utils::sanitize_text(&next_read.reason),
                next_read
                    .evidence_kinds
                    .iter()
                    .map(|kind| kind.label())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .expect("writing to a string cannot fail");
            for limitation in &next_read.limitations {
                writeln!(output, "  - limitation: {}", utils::sanitize_text(limitation))
                    .expect("writing to a string cannot fail");
            }
        }
        if let Some(walkthrough) = &explain.walkthrough {
            writeln!(output, "Entry-point walkthrough:").expect("writing to a string cannot fail");
            writeln!(
                output,
                "- `{}` → `{}` via {}",
                utils::escape_inline_code(&walkthrough.entry_point.path),
                utils::escape_inline_code(&walkthrough.target_path),
                utils::inline_code_list(&walkthrough.paths),
            )
            .expect("writing to a string cannot fail");
            for relationship in &walkthrough.relationships {
                Self::explain_relationship_markdown(output, relationship, "  ");
            }
            for limitation in &walkthrough.limitations {
                writeln!(output, "  - limitation: {}", utils::sanitize_text(limitation))
                    .expect("writing to a string cannot fail");
            }
        }

        if !explain.history_overlap.is_empty() {
            writeln!(output, "History overlap:").expect("writing to a string cannot fail");
            for path in &explain.history_overlap {
                writeln!(
                    output,
                    "- `{}` — {} commits{}",
                    utils::escape_inline_code(&path.path),
                    path.commits,
                    path.commits_per_kib_milli
                        .map(|rate| format!(", {:.3} commits/KiB", rate as f64 / 1_000.0))
                        .unwrap_or_default(),
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !explain.graph_edges.is_empty() {
            writeln!(output, "Related lexical evidence:").expect("writing to a string cannot fail");
            for edge in &explain.graph_edges {
                Self::explain_relationship_markdown(output, edge, "");
            }
        }
        if !explain.ambiguity.is_empty() {
            writeln!(output, "Ambiguity:").expect("writing to a string cannot fail");
            for finding in &explain.ambiguity {
                writeln!(
                    output,
                    "- **{}** in `{}`: {}",
                    finding.kind.label(),
                    utils::escape_inline_code(&finding.path),
                    utils::sanitize_text(&finding.detail),
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !explain.omitted_alternatives.is_empty() {
            writeln!(output, "Omitted alternatives:").expect("writing to a string cannot fail");
            for omission in &explain.omitted_alternatives {
                Self::explain_omission_markdown(output, omission, "");
            }
        }
        Render::caveats(output, &explain.limitations);
    }

    fn explain_task_seeds(task_seeds: &dalil_core::TaskSeeds) -> Vec<String> {
        let mut seeds = Vec::new();
        if let Some(task) = &task_seeds.task {
            seeds.push(format!("task `{}`", utils::escape_inline_code(task)));
        }
        for (label, values) in [
            ("symbol", &task_seeds.symbols),
            ("path", &task_seeds.paths),
            (
                "language",
                &task_seeds
                    .languages
                    .iter()
                    .map(|language| language.label().to_owned())
                    .collect(),
            ),
            ("project", &task_seeds.projects),
            ("search", &task_seeds.search_terms),
        ] {
            if !values.is_empty() {
                seeds.push(format!("{label} {}", utils::inline_code_list(values)));
            }
        }
        for change in &task_seeds.changes {
            let (kind, value) = match change {
                dalil_core::TaskChangeSeed::Path(value) => ("changed path", value),
                dalil_core::TaskChangeSeed::Symbol(value) => ("changed symbol", value),
            };
            seeds.push(format!("{kind} `{}`", utils::escape_inline_code(value)));
        }
        seeds
    }

    fn explain_ranking_markdown(output: &mut String, ranking: &dalil_core::ExplainRanking, indent: &str) {
        let contributions = &ranking.contributions;
        writeln!(
            output,
            "{indent}- ranking: score {}; focus matches {}; {} incoming and {} outgoing relationship(s); contributions centrality={}, seed proximity={}, lexical relevance={}, history evidence={}, explicit focus={}",
            ranking.score,
            ranking.focus_matches,
            ranking.incoming_edges,
            ranking.outgoing_edges,
            contributions.centrality,
            contributions.seed_proximity,
            contributions.lexical_relevance,
            contributions.history_evidence,
            contributions.explicit_focus,
        )
        .expect("writing to a string cannot fail");
        if !ranking.matched_seeds.is_empty() {
            writeln!(
                output,
                "{indent}- matched seeds: {}",
                ranking
                    .matched_seeds
                    .iter()
                    .map(|seed| format!("`{}` `{}`", seed.kind.label(), utils::escape_inline_code(&seed.seed)))
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .expect("writing to a string cannot fail");
        }
    }

    pub(super) fn explain_relationship_markdown(output: &mut String, edge: &dalil_core::LexicalEdge, indent: &str) {
        let candidates = if edge.candidates.is_empty() {
            "none retained".to_owned()
        } else {
            utils::inline_code_list(&edge.candidates)
        };
        writeln!(
            output,
            "{indent}- relationship: `{}` → `{}` via `{}` — {} / {}; target visibility {}; candidates {}; candidate group `{}`{}",
            utils::escape_inline_code(&edge.source),
            utils::escape_inline_code(&edge.target),
            utils::escape_inline_code(&edge.symbol),
            edge.resolution_reason.label(),
            edge.confidence.label(),
            edge.target_visibility.label(),
            candidates,
            utils::escape_inline_code(&edge.candidate_group),
            if edge.ambiguous { "; ambiguous" } else { "" },
        )
        .expect("writing to a string cannot fail");
    }

    fn explain_omission_markdown(output: &mut String, omission: &dalil_core::SourceOmission, indent: &str) {
        let classifications = if omission.classifications.is_empty() {
            String::new()
        } else {
            format!(
                "; classifications {}",
                omission
                    .classifications
                    .iter()
                    .map(|classification| format!(
                        "{}: {}",
                        classification.kind.label(),
                        utils::sanitize_text(&classification.reason)
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        writeln!(
            output,
            "{indent}- omission: `{}` — {}: {}{}{}",
            utils::escape_inline_code(&omission.path),
            omission.reason.label(),
            utils::sanitize_text(&omission.detail),
            classifications,
            if omission.classification_overridden { "; classification overridden" } else { "" },
        )
        .expect("writing to a string cannot fail");
    }
}
