use std::fmt::Write;

use crate::{html, render::Render, utils};

pub use dalil_core::*;

pub fn render_capabilities(report: &CapabilitiesReport, format: OutputFormat) -> anyhow::Result<String> {
    match format {
        OutputFormat::Json => Ok(format!("{}\n", serde_json::to_string_pretty(report)?)),
        OutputFormat::Html => html::render_capabilities(report),
        OutputFormat::Markdown => {
            let mut output = format!(
                "# Dalil capabilities\n\nSchema version: {}\nTool version: {}\nQuery packs valid: {}\n\n## Languages\n",
                report.schema_version, report.tool_version, report.query_packs_valid
            );
            for language in &report.languages {
                writeln!(
                    output,
                    "- {} (`{}`) — grammar {} {}, query pack {} {}",
                    language.language.display_label(),
                    language.extensions.join(", "),
                    language.grammar,
                    language.grammar_version,
                    language.query_pack,
                    language.query_pack_version
                )?;
            }
            Ok(output)
        }
    }
}

pub fn render_doctor(report: &DoctorReport, format: OutputFormat) -> anyhow::Result<String> {
    match format {
        OutputFormat::Json => Ok(format!("{}\n", serde_json::to_string_pretty(report)?)),
        OutputFormat::Html => html::render_doctor(report),
        OutputFormat::Markdown => {
            let mut output = format!(
                "# Dalil doctor\n\nTool version: {}\nSource evidence collected: {}\nRepository state changed: {}\n",
                report.tool_version, report.source_evidence_collected, report.repository_state_changed
            );
            for check in &report.checks {
                writeln!(output, "- **{:?}** {}: {}", check.status, check.name, check.detail)?;
            }
            Ok(output)
        }
    }
}

pub fn render_relationships(report: &RelationshipResults, format: OutputFormat) -> anyhow::Result<String> {
    match format {
        OutputFormat::Json => Ok(format!("{}\n", serde_json::to_string_pretty(report)?)),
        OutputFormat::Html => anyhow::bail!("relationship queries support Markdown or JSON output, not HTML"),
        OutputFormat::Markdown => {
            let mut output = format!(
                "# Dalil {}\n\nTarget: `{}`\nRepository: `{}`\n\n",
                report.request.operation.label(),
                utils::escape_inline_code(&report.request.target),
                utils::escape_inline_code(&report.request.repository),
            );
            writeln!(
                output,
                "Results: {} returned of {} ({} omitted); {} relationship(s) returned.",
                report.bounds.returned,
                report.bounds.total,
                report.bounds.omitted,
                report.bounds.returned_relationships,
            )?;
            if let Some(cursor) = report.bounds.continuation {
                writeln!(output, "Next page: offset {}, limit {}.", cursor.offset, cursor.limit)?;
            }
            writeln!(output, "\n## Matches\n")?;
            if report.matches.is_empty() {
                writeln!(output, "No matching repository evidence was found.")?;
            } else {
                for item in &report.matches {
                    let location = item
                        .node
                        .symbol
                        .as_ref()
                        .map(|symbol| format!(" {}", format_location(&symbol.location)))
                        .unwrap_or_default();
                    writeln!(
                        output,
                        "- `{}` — {} `{}`{} (confidence: {}, {})",
                        utils::escape_inline_code(&item.node.path),
                        item.relation.label(),
                        utils::escape_inline_code(
                            item.node
                                .symbol
                                .as_ref()
                                .map(|symbol| symbol.name.as_str())
                                .unwrap_or(item.node.path.as_str()),
                        ),
                        location,
                        item.confidence.label(),
                        if item.ambiguous { "ambiguous" } else { "direct" },
                    )?;
                    writeln!(output, "  - node: `{}`", utils::escape_inline_code(&item.node.id))?;
                    writeln!(output, "  - {}", utils::sanitize_text(&item.reason))?;
                }
            }
            if !report.relationships.is_empty() {
                writeln!(output, "\n## Relationships\n")?;
                for relationship in &report.relationships {
                    writeln!(
                        output,
                        "- {} `{}` → `{}` (confidence: {}, {})",
                        relationship.kind.label(),
                        utils::escape_inline_code(&relationship.source_path),
                        utils::escape_inline_code(&relationship.target_path),
                        relationship.confidence.label(),
                        if relationship.ambiguous { "ambiguous" } else { "resolved lexically" },
                    )?;
                    writeln!(
                        output,
                        "  - relationship: `{}`",
                        utils::escape_inline_code(&relationship.id)
                    )?;
                }
            }
            if !report.omissions.is_empty() {
                writeln!(output, "\n## Omissions\n")?;
                for omission in &report.omissions {
                    writeln!(
                        output,
                        "- {}: {}",
                        omission.reason.label(),
                        utils::sanitize_text(&omission.detail)
                    )?;
                }
            }
            if !report.limitations.is_empty() {
                writeln!(output, "\n## Limitations\n")?;
                for limitation in &report.limitations {
                    writeln!(output, "- {}", utils::sanitize_text(limitation))?;
                }
            }
            Ok(output)
        }
    }
}

fn format_location(location: &SourceLocation) -> String {
    format!(
        "{}:{}-{}:{}",
        location.start.line, location.start.column, location.end.line, location.end.column
    )
}

/// Render a typed report without re-running analysis or transforming its evidence.
pub fn render_report(report: &Report, format: OutputFormat) -> anyhow::Result<String> {
    let output = match format {
        OutputFormat::Markdown => {
            let output = render_markdown(report);
            if report.profile == AnalysisProfile::Compact && report.command.name != CommandName::Search {
                Ok(bound_compact_markdown(
                    output,
                    report.provenance.effective_options.map.map_tokens,
                ))
            } else {
                Ok(output)
            }
        }
        OutputFormat::Json => {
            let mut output = serde_json::to_string_pretty(report)?;
            output.push('\n');
            Ok(output)
        }
        OutputFormat::Html => html::render_report(report),
    }?;
    if output.len() > report.limits.max_output_bytes {
        anyhow::bail!(
            "rendered report exceeds the {}-byte output limit; use the compact profile or a narrower scope",
            report.limits.max_output_bytes
        );
    }
    Ok(output)
}

fn render_markdown(report: &Report) -> String {
    let mut output = String::new();
    let command = match (report.command.name, report.command.operation) {
        (CommandName::Orient, None) => "Orientation".to_owned(),
        (_, Some(operation)) => format!("{}: {}", report.command.name.label(), operation.label()),
        (_, None) => report.command.name.label().to_owned(),
    };

    writeln!(output, "# Dalil {command}").expect("writing to a string cannot fail");
    writeln!(output).expect("writing to a string cannot fail");
    writeln!(output, "Schema version: {}", report.schema_version).expect("writing to a string cannot fail");
    writeln!(
        output,
        "Scope: `{}`",
        utils::escape_inline_code(&report.scope.selected_path)
    )
    .expect("writing to a string cannot fail");
    writeln!(output, "Status: {:?}", report.status).expect("writing to a string cannot fail");
    writeln!(output).expect("writing to a string cannot fail");
    writeln!(output, "## Summary").expect("writing to a string cannot fail");
    writeln!(output).expect("writing to a string cannot fail");
    writeln!(output, "{}", utils::sanitize_text(&report.summary)).expect("writing to a string cannot fail");
    let compact_orientation = report.command.name == CommandName::Orient && report.profile == AnalysisProfile::Compact;
    let compact_briefing = report.command.name == CommandName::Briefing && report.profile == AnalysisProfile::Compact;
    if !compact_orientation && !compact_briefing {
        Render::quality_markdown(&mut output, &report.quality, report.command.name);
    }

    if let Some(orientation) = &report.orientation {
        Render::orientation_markdown(&mut output, orientation);
    }
    if let Some(explain) = &report.explain {
        Render::explain_markdown(&mut output, explain);
    }
    if let Some(context) = &report.context {
        Render::context_markdown(&mut output, context);
    }
    if let Some(impact) = &report.impact {
        Render::impact_markdown(&mut output, impact);
    }
    if let Some(search) = &report.search {
        Render::search_markdown(&mut output, search);
    }

    if report.command.name == CommandName::Briefing {
        if let Some(map) = &report.map {
            Render::briefing_overview(&mut output, map);
        }
        if let Some(reading_plan) = &report.reading_plan {
            Render::reading_plan_markdown(&mut output, reading_plan);
        }
        if let Some(history) = &report.history {
            if compact_briefing {
                Render::history_briefing_markdown(&mut output, history);
            } else {
                Render::history_markdown(&mut output, history);
            }
        }
        if compact_briefing {
            Render::quality_markdown(&mut output, &report.quality, report.command.name);
            if let Some(map) = &report.map {
                Render::briefing_evidence_notes(&mut output, map);
            }
        } else if let Some(map) = &report.map {
            Render::map_markdown(&mut output, map);
        }
    } else if !matches!(
        report.command.name,
        CommandName::Orient | CommandName::Explain | CommandName::Context | CommandName::Impact | CommandName::Search
    ) {
        if let Some(history) = &report.history {
            if report.profile == AnalysisProfile::Compact && report.command.operation.is_none() {
                Render::history_briefing_markdown(&mut output, history);
            } else {
                Render::history_markdown(&mut output, history);
            }
        }
        if let Some(map) = &report.map {
            Render::map_markdown(&mut output, map);
        }
    }

    if !report.findings.is_empty() {
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "## Findings").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
        for finding in &report.findings {
            writeln!(
                output,
                "- **{}:** {}",
                utils::escape_markdown(&finding.title),
                utils::sanitize_text(&finding.detail)
            )
            .expect("writing to a string cannot fail");
        }
    }

    if !report.limitations.is_empty() {
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "## Limitations").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
        for limitation in &report.limitations {
            writeln!(output, "- {}", utils::sanitize_text(&limitation.detail))
                .expect("writing to a string cannot fail");
        }
    }

    output
}
const COMPACT_MARKDOWN_TRUNCATION_NOTICE: &str = "\n\n_Report truncated at the compact Markdown token budget; use `--json` for complete typed collections or `--profile evidence` for verbose Markdown._\n";

fn bound_compact_markdown(output: String, token_budget: usize) -> String {
    if utils::token_count(&output) <= token_budget {
        return output;
    }

    let character_budget = token_budget.saturating_mul(4);
    let notice_characters = COMPACT_MARKDOWN_TRUNCATION_NOTICE.chars().count();
    if character_budget <= notice_characters {
        return output.chars().take(character_budget).collect();
    }

    let content_budget = character_budget - notice_characters;
    let mut bounded = output.chars().take(content_budget).collect::<String>();
    if let Some(last_line_end) = bounded.rfind('\n') {
        bounded.truncate(last_line_end);
    }
    bounded.push_str(COMPACT_MARKDOWN_TRUNCATION_NOTICE);
    bounded
}
