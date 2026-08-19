use std::fmt::Write;

use super::Render;
use crate::utils;

impl Render {
    pub fn impact_markdown(output: &mut String, impact: &dalil_core::ImpactReport) {
        Render::section_heading(output, "Impact context");
        if let Some(task) = &impact.request.task {
            writeln!(output, "Task: {}", utils::sanitize_text(task)).expect("writing to a string cannot fail");
        }
        writeln!(output, "Change resolution: {}", impact.change_resolution.status.label())
            .expect("writing to a string cannot fail");
        writeln!(
            output,
            "Impact traversal: {} node(s) from {} seed node(s); {} edge inspections of {} allowed (depth {}).{}",
            impact.traversal.affected_nodes,
            impact.traversal.seed_nodes,
            impact.traversal.inspected_edges,
            impact.traversal.work_limit,
            impact.traversal.max_depth,
            if impact.traversal.truncated { " Incomplete evidence was omitted." } else { "" },
        )
        .expect("writing to a string cannot fail");
        if impact.change_resolution.changes.is_empty() {
            writeln!(
                output,
                "No changed paths were resolved from the supplied local change inputs."
            )
            .expect("writing to a string cannot fail");
        } else {
            writeln!(output, "Changed paths and symbols:").expect("writing to a string cannot fail");
            for change in &impact.change_resolution.changes {
                let symbols = if change.symbols.is_empty() {
                    "no current changed symbols".to_owned()
                } else {
                    change
                        .symbols
                        .iter()
                        .map(|symbol| format!("{} `{}`", symbol.kind.label(), utils::escape_inline_code(&symbol.name)))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                writeln!(
                    output,
                    "- {} `{}`: {}",
                    change.kind.label(),
                    utils::escape_inline_code(&change.path),
                    symbols,
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !impact.targets.is_empty() {
            Render::section_heading(output, "Inspection targets");
            for target in &impact.targets {
                let evidence = target
                    .evidence
                    .iter()
                    .map(|kind| kind.label())
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    output,
                    "- `{}` ({} {}, depth {}; {} confidence; {}) — {}",
                    utils::escape_inline_code(&target.path),
                    target.reachability.label(),
                    if target.depth == 0 { "seed" } else { "downstream" },
                    target.depth,
                    target.confidence.label(),
                    evidence,
                    utils::sanitize_text(&target.reason),
                )
                .expect("writing to a string cannot fail");
                if !target.relationship_path.is_empty() {
                    let path =
                        target
                            .relationship_path
                            .iter()
                            .map(|relationship| format!("`{}`", utils::escape_inline_code(&relationship.source_path)))
                            .chain(target.relationship_path.last().map(|relationship| {
                                format!("`{}`", utils::escape_inline_code(&relationship.target_path))
                            }))
                            .collect::<Vec<_>>()
                            .join(" → ");
                    writeln!(output, "  Path: {path}").expect("writing to a string cannot fail");
                }
                for symbol in &target.symbols {
                    writeln!(
                        output,
                        "  Symbol: {} `{}` at {}:{}",
                        symbol.symbol.kind.label(),
                        utils::escape_inline_code(&symbol.symbol.name),
                        symbol.symbol.location.start.line,
                        symbol.symbol.location.start.column,
                    )
                    .expect("writing to a string cannot fail");
                }
            }
        }
        if !impact.relationships.is_empty() {
            Render::section_heading(output, "Evidence relationships");
            for relationship in &impact.relationships {
                let symbol = relationship
                    .symbol
                    .as_deref()
                    .map(|symbol| format!(" via `{}`", utils::escape_inline_code(symbol)))
                    .unwrap_or_default();
                writeln!(
                    output,
                    "- {} `{}` to `{}`{} ({} {}, depth {}; {} confidence) — {}{}",
                    relationship.evidence.label(),
                    utils::escape_inline_code(&relationship.source),
                    utils::escape_inline_code(&relationship.target),
                    symbol,
                    relationship.reachability.label(),
                    if relationship.depth == 0 { "seed" } else { "downstream" },
                    relationship.depth,
                    relationship.confidence.label(),
                    utils::sanitize_text(&relationship.reason),
                    if relationship.ambiguous { "; ambiguous candidate" } else { "" },
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !impact.projects.is_empty() {
            Render::section_heading(output, "Affected projects");
            for project in &impact.projects {
                writeln!(
                    output,
                    "- `{}` ({}; {} confidence) — {} path(s), {} symbol(s), {} test(s)",
                    utils::escape_inline_code(&project.path),
                    project.reachability.label(),
                    project.confidence.label(),
                    project.affected_paths.len(),
                    project.affected_symbols.len(),
                    project.affected_tests.len(),
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !impact.likely_tests.is_empty() {
            Render::section_heading(output, "Likely tests");
            for test in &impact.likely_tests {
                let ranking = test.score.map(|score| format!(", score {score}")).unwrap_or_default();
                writeln!(
                    output,
                    "- `{}` ({} confidence{}) — {}",
                    utils::escape_inline_code(&test.path),
                    test.confidence.label(),
                    ranking,
                    utils::sanitize_text(&test.reason),
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !impact.ownership.is_empty() {
            Render::section_heading(output, "Ownership signals");
            for signal in &impact.ownership {
                writeln!(
                    output,
                    "- `{}` ({} confidence) — {}",
                    utils::escape_inline_code(&signal.path),
                    signal.confidence.label(),
                    utils::sanitize_text(&signal.reason),
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !impact.history.is_empty() {
            Render::section_heading(output, "Relevant history");
            for evidence in &impact.history {
                writeln!(
                    output,
                    "- `{}` ({} confidence) — {}",
                    utils::escape_inline_code(&evidence.path),
                    evidence.confidence.label(),
                    utils::sanitize_text(&evidence.reason),
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !impact.uncertainty.is_empty() {
            Render::section_heading(output, "Uncertainty");
            for uncertainty in &impact.uncertainty {
                writeln!(output, "- {}", utils::sanitize_text(&uncertainty.detail))
                    .expect("writing to a string cannot fail");
            }
        }
        Render::section_heading(output, "Impact budget");
        writeln!(
            output,
            "{} estimated tokens of {} requested{}.",
            impact.budget.estimated_tokens,
            impact.budget.token_budget,
            if impact.budget.truncated { "; lower-priority evidence was omitted" } else { "" },
        )
        .expect("writing to a string cannot fail");
    }
}
