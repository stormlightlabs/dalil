use std::fmt::Write;

use super::Render;
use crate::utils;

impl Render {
    pub fn context_markdown(output: &mut String, context: &dalil_core::ContextBundle) {
        Render::section_heading(output, "Task context");
        if let Some(task) = &context.request.task {
            writeln!(output, "Task: {}", utils::sanitize_text(task)).expect("writing to a string cannot fail");
        } else {
            writeln!(output, "Task: no task text was supplied.").expect("writing to a string cannot fail");
        }
        writeln!(
            output,
            "Repository: `{}`; scope: `{}`; worktree: {}",
            utils::escape_inline_code(&context.orientation.repository_root),
            utils::escape_inline_code(&context.orientation.scope_path),
            context.orientation.worktree.label(),
        )
        .expect("writing to a string cannot fail");
        if !context.orientation.primary_languages.is_empty() {
            writeln!(
                output,
                "Primary languages: {}",
                context
                    .orientation
                    .primary_languages
                    .iter()
                    .map(|language| language.display_label())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .expect("writing to a string cannot fail");
        }
        if !context.orientation.landmarks.is_empty() {
            writeln!(output, "Orientation landmarks:").expect("writing to a string cannot fail");
            for landmark in &context.orientation.landmarks {
                writeln!(
                    output,
                    "- {} `{}` — {}",
                    landmark.kind.label(),
                    utils::escape_inline_code(&landmark.path),
                    utils::sanitize_text(&landmark.reason),
                )
                .expect("writing to a string cannot fail");
            }
        }

        if context.change_resolution.status != dalil_core::ChangeResolutionStatus::NotRequested {
            Render::section_heading(output, "Resolved changes");
            writeln!(output, "Status: {}", context.change_resolution.status.label())
                .expect("writing to a string cannot fail");
            for change in &context.change_resolution.changes {
                let previous = change
                    .previous_path
                    .as_deref()
                    .map(|path| format!(" from `{}`", utils::escape_inline_code(path)))
                    .unwrap_or_default();
                writeln!(
                    output,
                    "- {} `{}`{}; {} changed line range(s), {} changed symbol(s)",
                    change.kind.label(),
                    utils::escape_inline_code(&change.path),
                    previous,
                    change.changed_lines.len(),
                    change.symbols.len(),
                )
                .expect("writing to a string cannot fail");
            }
        }

        Render::section_heading(output, "Recommended files");
        if context.files.is_empty() {
            writeln!(
                output,
                "The total context budget did not retain a recommended source file."
            )
            .expect("writing to a string cannot fail");
        }
        for file in &context.files {
            let recommendation = &file.recommendation;
            writeln!(
                output,
                "{}. `{}` ({}, {} confidence) — {}",
                recommendation.ordinal,
                utils::escape_inline_code(&recommendation.path),
                recommendation.purpose.label(),
                recommendation.confidence.label(),
                utils::sanitize_text(&recommendation.reason),
            )
            .expect("writing to a string cannot fail");
            if let Some(ranking) = &file.ranking {
                writeln!(
                    output,
                    "   Ranking: {}; centrality={}, seed proximity={}, lexical relevance={}, history evidence={}, focus={}",
                    ranking.score,
                    ranking.contributions.centrality,
                    ranking.contributions.seed_proximity,
                    ranking.contributions.lexical_relevance,
                    ranking.contributions.history_evidence,
                    ranking.contributions.explicit_focus,
                )
                .expect("writing to a string cannot fail");
            }
            for symbol in &file.symbols {
                writeln!(
                    output,
                    "   Symbol: {} `{}` at {}:{} (score {})",
                    symbol.symbol.kind.label(),
                    utils::escape_inline_code(&symbol.symbol.name),
                    symbol.symbol.location.start.line,
                    symbol.symbol.location.start.column,
                    symbol.score,
                )
                .expect("writing to a string cannot fail");
            }
        }

        if let Some(teaching) = &context.teaching {
            Render::section_heading(output, "Teaching scaffold");
            for step in &teaching.steps {
                writeln!(
                    output,
                    "- {} ({} ordering) — {}",
                    step.topic.label(),
                    step.ordering.label(),
                    utils::sanitize_text(&step.explanation),
                )
                .expect("writing to a string cannot fail");
                for evidence in &step.observed {
                    let symbol = evidence
                        .symbol
                        .as_deref()
                        .map(|symbol| format!(" `{}`", utils::escape_inline_code(symbol)))
                        .unwrap_or_default();
                    writeln!(
                        output,
                        "  Observed {}: `{}`{}",
                        evidence.kind.label(),
                        utils::escape_inline_code(&evidence.path),
                        symbol,
                    )
                    .expect("writing to a string cannot fail");
                }
            }
        }
        if !context.relationships.is_empty() {
            Render::section_heading(output, "Relationships");
            for relationship in &context.relationships {
                Render::explain_relationship_markdown(output, relationship, "");
            }
        }
        if !context.relevant_tests.is_empty() {
            Render::section_heading(output, "Relevant tests");
            for test in &context.relevant_tests {
                writeln!(
                    output,
                    "- `{}` ({} confidence) — {}",
                    utils::escape_inline_code(&test.path),
                    test.confidence.label(),
                    utils::sanitize_text(&test.reason),
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !context.history.is_empty() {
            Render::section_heading(output, "History");
            for observation in &context.history {
                writeln!(output, "- {}", Self::context_history_observation(observation))
                    .expect("writing to a string cannot fail");
            }
        }
        if !context.risks.is_empty() {
            Render::section_heading(output, "Risks");
            for risk in &context.risks {
                let paths = if risk.paths.is_empty() {
                    String::new()
                } else {
                    format!(
                        " (`{}`)",
                        risk.paths
                            .iter()
                            .map(|path| utils::escape_inline_code(path))
                            .collect::<Vec<_>>()
                            .join("`, `")
                    )
                };
                writeln!(output, "- {}{}", utils::sanitize_text(&risk.detail), paths)
                    .expect("writing to a string cannot fail");
            }
        }
        if !context.uncertainty.is_empty() {
            Render::section_heading(output, "Uncertainty");
            for uncertainty in &context.uncertainty {
                writeln!(output, "- {}", utils::sanitize_text(&uncertainty.detail))
                    .expect("writing to a string cannot fail");
            }
        }
        if !context.omissions.is_empty() {
            Render::section_heading(output, "Omissions");
            for omission in &context.omissions {
                writeln!(
                    output,
                    "- `{}` — {}",
                    utils::escape_inline_code(&omission.path),
                    utils::sanitize_text(&omission.reason),
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !context.next_reads.is_empty() {
            Render::section_heading(output, "Next reads");
            for recommendation in &context.next_reads {
                writeln!(
                    output,
                    "- `{}` ({}) — {}",
                    utils::escape_inline_code(&recommendation.path),
                    recommendation.purpose.label(),
                    utils::sanitize_text(&recommendation.reason),
                )
                .expect("writing to a string cannot fail");
            }
        }
        Render::section_heading(output, "Context budget");
        writeln!(
            output,
            "{} estimated tokens of {} requested{}.",
            context.budget.estimated_tokens,
            context.budget.token_budget,
            if context.budget.truncated { "; lower-priority evidence was omitted" } else { "" },
        )
        .expect("writing to a string cannot fail");
    }

    fn context_history_observation(observation: &dalil_core::HistoryObservation) -> String {
        match observation {
            dalil_core::HistoryObservation::Churn { paths, window_days, caveat } => format!(
                "{} churn path(s) over {window_days} days: {}; {}",
                paths.len(),
                paths
                    .iter()
                    .map(|path| format!("`{}`", utils::escape_inline_code(&path.path)))
                    .collect::<Vec<_>>()
                    .join(", "),
                utils::sanitize_text(caveat),
            ),
            dalil_core::HistoryObservation::Contributors { contributor, total_commits, caveat, .. } => format!(
                "{} has {} of {total_commits} commits; {}",
                utils::sanitize_text(&contributor.name),
                contributor.commits,
                utils::sanitize_text(caveat),
            ),
            dalil_core::HistoryObservation::BugOverlap { paths, bug_commits, caveat, .. } => format!(
                "{bug_commits} bug-keyword commit(s) overlap {}; {}",
                paths
                    .iter()
                    .map(|path| format!("`{}`", utils::escape_inline_code(&path.path)))
                    .collect::<Vec<_>>()
                    .join(", "),
                utils::sanitize_text(caveat),
            ),
            dalil_core::HistoryObservation::Activity { month, commits, caveat, .. } => format!(
                "{month} has {commits} observed commit(s); {}",
                utils::sanitize_text(caveat),
            ),
            dalil_core::HistoryObservation::Firefighting { commits, paths, caveat, .. } => format!(
                "{commits} firefighting-keyword commit(s) touch {}; {}",
                paths
                    .iter()
                    .map(|path| format!("`{}`", utils::escape_inline_code(&path.path)))
                    .collect::<Vec<_>>()
                    .join(", "),
                utils::sanitize_text(caveat),
            ),
        }
    }
}
