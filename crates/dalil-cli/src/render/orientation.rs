use std::fmt::Write;

use super::Render;
use crate::utils;

impl Render {
    pub fn briefing_overview(output: &mut String, map: &dalil_core::MapReport) {
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "## Repository overview").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(
            output,
            "Repository: `{}`",
            utils::escape_inline_code(&map.repository_root)
        )
        .expect("writing to a string cannot fail");
        writeln!(output, "Worktree: {}", map.worktree.state.label()).expect("writing to a string cannot fail");

        let mut languages = map
            .files
            .iter()
            .map(|file| file.language.display_label())
            .collect::<Vec<_>>();
        languages.sort_unstable();
        languages.dedup();
        writeln!(
            output,
            "Primary supported languages: {}",
            if languages.is_empty() { "none detected".to_owned() } else { languages.join(", ") }
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "Project roots: {} detected; {} source files analyzed",
            map.project_roots.len(),
            map.inventory.analyzed
        )
        .expect("writing to a string cannot fail");

        let landmarks = map
            .landmarks
            .iter()
            .filter(|landmark| {
                matches!(
                    landmark.kind,
                    dalil_core::LandmarkKind::AgentInstructions
                        | dalil_core::LandmarkKind::ContributorInstructions
                        | dalil_core::LandmarkKind::Readme
                        | dalil_core::LandmarkKind::Manifest
                        | dalil_core::LandmarkKind::WorkspaceRoot
                        | dalil_core::LandmarkKind::PackageRoot
                )
            })
            .take(5)
            .collect::<Vec<_>>();
        if landmarks.is_empty() {
            writeln!(output, "Orientation landmarks: none detected in the selected scope.")
                .expect("writing to a string cannot fail");
        } else {
            writeln!(output, "Orientation landmarks:").expect("writing to a string cannot fail");
            for landmark in landmarks {
                writeln!(
                    output,
                    "- **{}** `{}` — {}",
                    landmark.kind.label(),
                    utils::escape_inline_code(&landmark.path),
                    utils::sanitize_text(&landmark.reason)
                )
                .expect("writing to a string cannot fail");
            }
        }
    }

    pub fn orientation_markdown(output: &mut String, orientation: &dalil_core::OrientationReport) {
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "## Repository overview").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(
            output,
            "Repository: `{}`",
            utils::escape_inline_code(&orientation.repository.root)
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "Scope: `{}`",
            utils::escape_inline_code(&orientation.repository.scope_path)
        )
        .expect("writing to a string cannot fail");
        writeln!(output, "Worktree: {}", orientation.repository.worktree.label())
            .expect("writing to a string cannot fail");
        let reference = orientation
            .repository
            .head
            .reference
            .as_deref()
            .unwrap_or("not resolved");
        let revision = orientation.repository.head.oid.as_deref().unwrap_or("not resolved");
        let mut head_state = Vec::new();
        if orientation.repository.head.detached {
            head_state.push("detached");
        }
        if orientation.repository.head.unborn {
            head_state.push("unborn");
        }
        let head_state = if head_state.is_empty() { String::new() } else { format!(" ({})", head_state.join(", ")) };
        writeln!(
            output,
            "Revision: `{}` at `{}`{}",
            utils::escape_inline_code(reference),
            utils::escape_inline_code(revision),
            head_state
        )
        .expect("writing to a string cannot fail");
        let languages = orientation
            .repository
            .primary_languages
            .iter()
            .map(|language| language.display_label())
            .collect::<Vec<_>>();
        writeln!(
            output,
            "Primary supported languages: {}",
            if languages.is_empty() { "none detected".to_owned() } else { languages.join(", ") }
        )
        .expect("writing to a string cannot fail");

        Self::orientation_recommendations(output, "Start here", &orientation.starting_points);
        Self::orientation_roots(output, &orientation.important_roots);
        Self::orientation_recommendations(output, "Runtime entry points", &orientation.runtime_entry_points);
        Self::orientation_recommendations(output, "Tests", &orientation.tests);

        if !orientation.history.is_empty() {
            Self::section_heading(output, "Useful history");
            for observation in &orientation.history {
                Self::history_observation(output, observation);
            }
        }

        Self::orientation_recommendations(output, "Next reads", &orientation.next_reads);
        if !orientation.uncertainty.is_empty() {
            Self::section_heading(output, "Limitations");
            for uncertainty in &orientation.uncertainty {
                writeln!(
                    output,
                    "- `{}`: {}",
                    utils::escape_inline_code(&uncertainty.kind),
                    utils::sanitize_text(&uncertainty.detail)
                )
                .expect("writing to a string cannot fail");
            }
        }
        writeln!(
            output,
            "\nFor the repository-wide structure, use `dalil map`. Use `dalil explain PATH-OR-SYMBOL` for evidence behind one read."
        )
        .expect("writing to a string cannot fail");
    }

    pub fn search_markdown(output: &mut String, search: &dalil_core::SearchResults) {
        Render::section_heading(output, "Search results");
        writeln!(
            output,
            "Query: `{}` ({})",
            utils::escape_inline_code(&search.request.query),
            search.request.mode.label(),
        )
        .expect("writing to a string cannot fail");
        if search.matches.is_empty() {
            writeln!(output, "No strong anchors fit this search.").expect("writing to a string cannot fail");
        }
        for result in &search.matches {
            let recommendation = &result.recommendation;
            let target = match &result.symbol {
                Some(symbol) => format!("{} `{}`", symbol.kind.label(), utils::escape_inline_code(&symbol.name)),
                None => "path".to_owned(),
            };
            writeln!(
                output,
                "{}. `{}` ({target}, {} confidence) — {}",
                recommendation.ordinal,
                utils::escape_inline_code(&recommendation.path),
                recommendation.confidence.label(),
                utils::sanitize_text(&recommendation.reason),
            )
            .expect("writing to a string cannot fail");
            let evidence = recommendation
                .evidence_kinds
                .iter()
                .map(|kind| kind.label())
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(output, "   Evidence: {evidence}; score {}", result.score)
                .expect("writing to a string cannot fail");
            if result.anchor {
                writeln!(output, "   Direct lexical anchor for the next read.")
                    .expect("writing to a string cannot fail");
            }
            for limitation in &recommendation.limitations {
                writeln!(output, "   Limitation: {}", utils::sanitize_text(limitation))
                    .expect("writing to a string cannot fail");
            }
        }
        if let Some(shortfall) = &search.shortfall {
            writeln!(
                output,
                "Shortfall: {} of {} requested anchors — {}",
                shortfall.returned,
                shortfall.requested,
                utils::sanitize_text(&shortfall.reason),
            )
            .expect("writing to a string cannot fail");
        }
        writeln!(
            output,
            "Search budget: {} of {} estimated tokens; {} of {} candidate anchors returned.",
            search.budget.estimated_tokens,
            search.budget.token_budget,
            search.budget.returned,
            search.budget.total_candidates,
        )
        .expect("writing to a string cannot fail");
        Render::caveats(output, &search.limitations);
    }

    fn orientation_recommendations(
        output: &mut String, heading: &str, recommendations: &[dalil_core::ReadingRecommendation],
    ) {
        if recommendations.is_empty() {
            return;
        }
        Self::section_heading(output, heading);
        for recommendation in recommendations {
            let root = recommendation
                .project_root
                .as_deref()
                .filter(|root| *root != ".")
                .map(|root| format!(", project root `{}`", utils::escape_inline_code(root)))
                .unwrap_or_default();
            writeln!(
                output,
                "{}. `{}`{} — {} ({}; {})",
                recommendation.ordinal,
                utils::escape_inline_code(&recommendation.path),
                root,
                utils::sanitize_text(&recommendation.reason),
                recommendation.confidence.label(),
                recommendation
                    .evidence_kinds
                    .iter()
                    .map(|kind| kind.label())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .expect("writing to a string cannot fail");
            for limitation in &recommendation.limitations {
                writeln!(output, "   Limitation: {}", utils::sanitize_text(limitation))
                    .expect("writing to a string cannot fail");
            }
        }
    }

    fn orientation_roots(output: &mut String, roots: &[dalil_core::OrientationRoot]) {
        if roots.is_empty() {
            return;
        }
        Self::section_heading(output, "Important project roots");
        for root in roots {
            writeln!(
                output,
                "- `{}` ({}) — {}",
                utils::escape_inline_code(&root.path),
                root.kind.label(),
                utils::sanitize_text(&root.reason)
            )
            .expect("writing to a string cannot fail");
        }
    }

    pub fn reading_plan_markdown(output: &mut String, plan: &dalil_core::ReadingPlan) {
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "## Reading plan").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
        if plan.recommendations.is_empty() {
            writeln!(output, "No evidence-backed paths were selected for the reading plan.")
                .expect("writing to a string cannot fail");
        } else {
            let mut current_purpose = None;
            for recommendation in &plan.recommendations {
                if current_purpose != Some(recommendation.purpose) {
                    if current_purpose.is_some() {
                        writeln!(output).expect("writing to a string cannot fail");
                    }
                    current_purpose = Some(recommendation.purpose);
                    writeln!(output, "### {}", recommendation.purpose.label())
                        .expect("writing to a string cannot fail");
                    writeln!(output).expect("writing to a string cannot fail");
                }
                let root = recommendation
                    .project_root
                    .as_deref()
                    .filter(|root| *root != ".")
                    .map(|root| format!(", project root `{}`", utils::escape_inline_code(root)))
                    .unwrap_or_default();
                let evidence = recommendation
                    .evidence_kinds
                    .iter()
                    .map(|kind| kind.label())
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    output,
                    "{}. `{}`{} — {} ({}; {})",
                    recommendation.ordinal,
                    utils::escape_inline_code(&recommendation.path),
                    root,
                    utils::sanitize_text(&recommendation.reason),
                    recommendation.confidence.label(),
                    evidence
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !plan.primary_languages.is_empty() {
            let languages = plan
                .primary_languages
                .iter()
                .map(|language| language.display_label())
                .collect::<Vec<_>>();
            writeln!(output, "Likely primary languages: {}", languages.join(", "))
                .expect("writing to a string cannot fail");
        }
        if let Some(shortfall) = &plan.shortfall {
            writeln!(
                output,
                "Short plan: {} of {} minimum recommendations — {}",
                shortfall.returned,
                shortfall.target_minimum,
                utils::sanitize_text(&shortfall.reason)
            )
            .expect("writing to a string cannot fail");
        }
        if !plan.omitted_relevant_paths.is_empty() {
            writeln!(output, "Task-relevant paths omitted by the map bound:").expect("writing to a string cannot fail");
            for omission in &plan.omitted_relevant_paths {
                writeln!(
                    output,
                    "- `{}` — {}",
                    utils::escape_inline_code(&omission.path),
                    utils::sanitize_text(&omission.reason)
                )
                .expect("writing to a string cannot fail");
            }
        }
        let limited_recommendations = plan
            .recommendations
            .iter()
            .filter(|recommendation| !recommendation.limitations.is_empty())
            .count();
        if limited_recommendations > 0 {
            writeln!(
                output,
                "{limited_recommendations} recommendation(s) have limitations recorded in JSON."
            )
            .expect("writing to a string cannot fail");
        }
        for omission in &plan.omitted_project_roots {
            writeln!(
                output,
                "Omitted project root `{}` — {}",
                utils::escape_inline_code(&omission.project_root),
                utils::sanitize_text(&omission.reason)
            )
            .expect("writing to a string cannot fail");
        }
        if !plan.limitations.is_empty() {
            writeln!(output, "Plan limitations:").expect("writing to a string cannot fail");
            for limitation in &plan.limitations {
                writeln!(output, "- {}", utils::sanitize_text(limitation)).expect("writing to a string cannot fail");
            }
        }
    }

    pub fn quality_markdown(
        output: &mut String, quality: &dalil_core::ReportQuality, command: dalil_core::CommandName,
    ) {
        if !quality.projection && quality.strict_issues.is_empty() {
            return;
        }
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "## Quality").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
        if quality.strict_issues.is_empty() {
            writeln!(
                output,
                "Expected bounded projection only; collection totals and reasons remain available in JSON."
            )
            .expect("writing to a string cannot fail");
        } else {
            let issues = quality
                .strict_issues
                .iter()
                .map(|issue| issue.label())
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(output, "Actionable degradation: {issues}.").expect("writing to a string cannot fail");
            let next = if quality.unsafe_paths {
                "dalil doctor"
            } else if quality.stale {
                "dalil map --cache always"
            } else if command == dalil_core::CommandName::History {
                "dalil history --profile evidence"
            } else {
                "dalil map --profile evidence"
            };
            writeln!(output, "Next useful command: `{next}`.").expect("writing to a string cannot fail");
        }
    }
}
