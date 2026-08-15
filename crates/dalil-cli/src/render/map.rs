use std::collections::BTreeMap;
use std::fmt::Write;

use super::Render;
use crate::utils;

impl Render {
    pub fn map_markdown(output: &mut String, map: &dalil_core::MapReport) {
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "## Source map").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(
            output,
            "Repository: `{}`",
            utils::escape_inline_code(&map.repository_root)
        )
        .expect("writing to a string cannot fail");
        writeln!(output, "Map scope: `{}`", utils::escape_inline_code(&map.scope_path))
            .expect("writing to a string cannot fail");
        writeln!(output, "Query pack: `{}`", utils::escape_inline_code(&map.query_pack))
            .expect("writing to a string cannot fail");
        if map.query_packs.len() > 1 {
            let provenance = map
                .query_packs
                .iter()
                .map(|(language, query_pack)| format!("{language}={query_pack}"))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(output, "Query packs: `{}`", utils::escape_inline_code(&provenance))
                .expect("writing to a string cannot fail");
        }
        writeln!(
            output,
            "Inventory: {} tracked ({} modified), {} untracked, {} analyzed, {} omitted, {} classified",
            map.inventory.tracked,
            map.inventory.modified,
            map.inventory.untracked,
            map.inventory.analyzed,
            map.inventory.omitted,
            map.classifications.total
        )
        .expect("writing to a string cannot fail");
        if map.classifications.total > 0 {
            writeln!(
                output,
                "Classifications: {} paths ({} generated, {} vendor, {} minified, {} source maps); {} samples returned{}",
                map.classifications.total,
                map.classifications.generated,
                map.classifications.vendor,
                map.classifications.minified,
                map.classifications.source_map,
                map.classifications.returned,
                if map.classifications.truncated { "; sample truncated" } else { "" }
            )
            .expect("writing to a string cannot fail");
            Render::section_heading(output, "Generated, vendor, and minified paths");
            for sample in &map.classifications.samples {
                let reasons = sample
                    .classifications
                    .iter()
                    .map(|classification| {
                        format!(
                            "{} ({})",
                            classification.kind.label(),
                            utils::sanitize_text(&classification.reason)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    output,
                    "- `{}` — {} [{}]",
                    utils::escape_inline_code(&sample.path),
                    if sample.overridden { "included by explicit focus override" } else { "excluded before parsing" },
                    reasons
                )
                .expect("writing to a string cannot fail");
            }
        }
        if map.collections.files.truncated
            || map.collections.symbols.truncated
            || map.collections.omissions.truncated
            || map.collections.findings.truncated
            || map.collections.edges.truncated
            || map.collections.ranking.truncated
            || map.collections.snippets.truncated
            || map.collections.landmarks.truncated
            || map.collections.project_roots.truncated
        {
            writeln!(
                output,
                "Collections are bounded; JSON contains totals and truncation reasons."
            )
            .expect("writing to a string cannot fail");
        }
        if !map.exclusions.is_empty() {
            writeln!(output, "Exclusions: {}", utils::inline_code_list(&map.exclusions))
                .expect("writing to a string cannot fail");
        }
        if !map.findings.is_empty() {
            Render::section_heading(output, "Map findings");
            for finding in &map.findings {
                let location = finding
                    .location
                    .as_ref()
                    .map(Self::format_location)
                    .unwrap_or_else(|| "unknown location".to_owned());
                writeln!(
                    output,
                    "- **{}** `{}`{} — {}",
                    finding.kind.label(),
                    utils::escape_inline_code(&finding.path),
                    if finding.location.is_some() { format!(" at {location}") } else { String::new() },
                    utils::sanitize_text(&finding.detail)
                )
                .expect("writing to a string cannot fail");
            }
        }

        Render::section_heading(output, "Map limitations");
        for limitation in &map.limitations {
            writeln!(output, "- {}", utils::sanitize_text(limitation)).expect("writing to a string cannot fail");
        }

        let mut files_by_language: BTreeMap<dalil_core::SourceLanguage, Vec<&dalil_core::SourceFile>> = BTreeMap::new();
        for file in &map.files {
            files_by_language.entry(file.language).or_default().push(file);
        }
        if files_by_language.len() <= 1 {
            if map.files.is_empty() {
                Render::section_heading(output, "Rust files");
                writeln!(output, "No Rust files were analyzed.").expect("writing to a string cannot fail");
            } else {
                let (language, files) = files_by_language.iter().next().expect("one language group");
                Render::section_heading(output, &format!("{} files", language.display_label()));
                Render::source_files(output, files);
            }
        } else {
            for (language, files) in &files_by_language {
                Render::section_heading(output, &format!("{} files", language.display_label()));
                Render::source_files(output, files);
            }
        }

        if !map.landmarks.is_empty() || !map.project_roots.is_empty() {
            Render::section_heading(output, "Repository landmarks");
            writeln!(
                output,
                "Landmarks: {} returned of {}; project roots: {} returned of {}",
                map.collections.landmarks.returned,
                map.collections.landmarks.total,
                map.collections.project_roots.returned,
                map.collections.project_roots.total
            )
            .expect("writing to a string cannot fail");
            for root in &map.project_roots {
                writeln!(
                    output,
                    "- Project root `{}` — {} — {}",
                    utils::escape_inline_code(&root.path),
                    root.kind.label(),
                    utils::sanitize_text(&root.reason)
                )
                .expect("writing to a string cannot fail");
                if !root.recommended_paths.is_empty() {
                    writeln!(
                        output,
                        "  - Recommended source paths: {}",
                        utils::inline_code_list(&root.recommended_paths)
                    )
                    .expect("writing to a string cannot fail");
                }
                for metadata in &root.manifest_metadata {
                    if metadata.truncated {
                        writeln!(
                            output,
                            "  - Manifest metadata from `{}` reached its per-kind item limit.",
                            utils::escape_inline_code(&metadata.path)
                        )
                        .expect("writing to a string cannot fail");
                    }
                    if !metadata.runtime_entry_points.is_empty() {
                        let entries = metadata
                            .runtime_entry_points
                            .iter()
                            .map(|target| {
                                target
                                    .resolved_path
                                    .as_deref()
                                    .map_or_else(|| target.declared.clone(), |path| path.to_owned())
                            })
                            .collect::<Vec<_>>();
                        writeln!(
                            output,
                            "  - Declared runtime entry points from `{}`: {}",
                            utils::escape_inline_code(&metadata.path),
                            utils::inline_code_list(&entries)
                        )
                        .expect("writing to a string cannot fail");
                    }
                    if !metadata.library_exports.is_empty() {
                        let exports = metadata
                            .library_exports
                            .iter()
                            .map(|target| {
                                target
                                    .resolved_path
                                    .as_deref()
                                    .map_or_else(|| target.declared.clone(), |path| path.to_owned())
                            })
                            .collect::<Vec<_>>();
                        writeln!(
                            output,
                            "  - Declared library exports from `{}`: {}",
                            utils::escape_inline_code(&metadata.path),
                            utils::inline_code_list(&exports)
                        )
                        .expect("writing to a string cannot fail");
                    }
                    if !metadata.commands.is_empty() {
                        let commands = metadata
                            .commands
                            .iter()
                            .map(|command| command.command.clone())
                            .collect::<Vec<_>>();
                        writeln!(
                            output,
                            "  - Common commands from `{}`: {}",
                            utils::escape_inline_code(&metadata.path),
                            utils::inline_code_list(&commands)
                        )
                        .expect("writing to a string cannot fail");
                    }
                }
            }
            for landmark in &map.landmarks {
                writeln!(
                    output,
                    "- **{}** `{}` — {} [{}{}]",
                    landmark.kind.label(),
                    utils::escape_inline_code(&landmark.path),
                    utils::sanitize_text(&landmark.reason),
                    landmark.worktree_state.label(),
                    landmark.project_root.as_deref().map_or(String::new(), |root| {
                        format!(", project root `{}`", utils::escape_inline_code(root))
                    })
                )
                .expect("writing to a string cannot fail");
            }
        }

        if !map.files.is_empty()
            || map.cache.matched > 0
            || map.cache.unmatched > 0
            || map.cache.unavailable > 0
            || !map.cache.reused.is_empty()
            || !map.cache.invalidated.is_empty()
            || map.cache.hits > 0
            || map.cache.misses > 0
            || !map.cache.refreshed.is_empty()
            || !map.cache.stale.is_empty()
        {
            writeln!(
                output,
                "Cache: {} ({}) — {} matched, {} unmatched, {} unavailable, {} reused, {} invalidated, {} hits, {} misses, {} refreshed, {} stale",
                map.cache.mode.label(),
                map.cache.status.label(),
                map.cache.matched,
                map.cache.unmatched,
                map.cache.unavailable,
                map.cache.reused.len(),
                map.cache.invalidated.len(),
                map.cache.hits,
                map.cache.misses,
                map.cache.refreshed.len(),
                map.cache.stale.len()
            )
            .expect("writing to a string cannot fail");
            if let Some(detail) = &map.cache.index_detail {
                writeln!(
                    output,
                    "Repository index: {} — {}",
                    map.cache.index_status.label(),
                    utils::sanitize_text(detail),
                )
                .expect("writing to a string cannot fail");
            } else {
                writeln!(output, "Repository index: {}", map.cache.index_status.label(),)
                    .expect("writing to a string cannot fail");
            }
            if !map.files.is_empty() {
                let mut task_seed_groups = Vec::new();
                if let Some(task) = &map.task_seeds.task {
                    task_seed_groups.push(format!("task `{}`", utils::escape_inline_code(task)));
                }
                for (label, seeds) in [
                    ("symbols", &map.task_seeds.symbols),
                    ("paths", &map.task_seeds.paths),
                    ("projects", &map.task_seeds.projects),
                    ("search", &map.task_seeds.search_terms),
                ] {
                    if !seeds.is_empty() {
                        task_seed_groups.push(format!("{label} {}", utils::inline_code_list(seeds)));
                    }
                }
                if !map.task_seeds.languages.is_empty() {
                    let languages = map
                        .task_seeds
                        .languages
                        .iter()
                        .map(|language| language.label().to_owned())
                        .collect::<Vec<_>>();
                    task_seed_groups.push(format!("languages {}", utils::inline_code_list(&languages)));
                }
                if !map.task_seeds.changes.is_empty() {
                    let changes = map
                        .task_seeds
                        .changes
                        .iter()
                        .map(|change| match change {
                            dalil_core::TaskChangeSeed::Path(path) => format!("path:{path}"),
                            dalil_core::TaskChangeSeed::Symbol(symbol) => format!("symbol:{symbol}"),
                        })
                        .collect::<Vec<_>>();
                    task_seed_groups.push(format!("changes {}", utils::inline_code_list(&changes)));
                }
                if !task_seed_groups.is_empty() {
                    writeln!(output, "Task seeds: {}", task_seed_groups.join("; "))
                        .expect("writing to a string cannot fail");
                }
                writeln!(
                    output,
                    "Ranking: {} files; map budget {} tokens, selected {} across {} file(s)",
                    map.ranking.len(),
                    map.selection.token_budget,
                    map.selection.estimated_tokens,
                    map.selection.snippets.len(),
                )
                .expect("writing to a string cannot fail");
                if !map.selection.primary_languages.is_empty() {
                    let languages = map
                        .selection
                        .primary_languages
                        .iter()
                        .map(|language| language.display_label())
                        .collect::<Vec<_>>();
                    writeln!(output, "Likely primary languages: {}", languages.join(", "))
                        .expect("writing to a string cannot fail");
                }
                if let Some(shortfall) = &map.selection.shortfall {
                    writeln!(
                        output,
                        "Short selection: {} of {} minimum source files — {}",
                        shortfall.returned,
                        shortfall.target_minimum,
                        utils::sanitize_text(&shortfall.reason)
                    )
                    .expect("writing to a string cannot fail");
                }
                if !map.selection.omitted_relevant_paths.is_empty() {
                    writeln!(output, "Task-relevant paths omitted by the map bound:")
                        .expect("writing to a string cannot fail");
                    for omission in &map.selection.omitted_relevant_paths {
                        writeln!(
                            output,
                            "- `{}` — {}",
                            utils::escape_inline_code(&omission.path),
                            utils::sanitize_text(&omission.reason)
                        )
                        .expect("writing to a string cannot fail");
                    }
                }
                Render::section_heading(output, "Ranked map selection");
                if map.selection.snippets.is_empty() {
                    writeln!(output, "No structural snippets fit the map token budget.")
                        .expect("writing to a string cannot fail");
                } else {
                    for snippet in &map.selection.snippets {
                        let location = Self::format_location(&snippet.symbol.location);
                        let scope = if snippet.symbol.scope.is_empty() {
                            "root".to_owned()
                        } else {
                            snippet.symbol.scope.join("::")
                        };
                        writeln!(
                            output,
                            "- `{}` — {} `{}` at {} in `{}` (score {}, {} tokens) — `{}`{}",
                            utils::escape_inline_code(&snippet.path),
                            snippet.symbol.kind.label(),
                            utils::escape_inline_code(&snippet.symbol.name),
                            location,
                            utils::escape_inline_code(&scope),
                            snippet.score,
                            snippet.estimated_tokens,
                            utils::escape_inline_code(&snippet.symbol.context),
                            if snippet.truncated { " (elided)" } else { "" }
                        )
                        .expect("writing to a string cannot fail");
                    }
                }
            }
        }

        if !map.edges.is_empty() {
            Render::section_heading(output, "Lexical dependency edges");
            for edge in &map.edges {
                writeln!(
                    output,
                    "- `{}` → `{}` via `{}` — {} / {}{}",
                    utils::escape_inline_code(&edge.source),
                    utils::escape_inline_code(&edge.target),
                    utils::escape_inline_code(&edge.symbol),
                    edge.resolution_reason.label(),
                    edge.confidence.label(),
                    if edge.ambiguous { " (ambiguous candidate)" } else { "" }
                )
                .expect("writing to a string cannot fail");
            }
        }

        if !map.omissions.is_empty() {
            Render::section_heading(output, "Omitted paths");
            for omission in &map.omissions {
                writeln!(
                    output,
                    "- `{}` — **{}:** {}",
                    utils::escape_inline_code(&omission.path),
                    omission.reason.label(),
                    utils::sanitize_text(&omission.detail)
                )
                .expect("writing to a string cannot fail");
            }
        }
    }

    fn source_files(output: &mut String, files: &[&dalil_core::SourceFile]) {
        for file in files {
            writeln!(
                output,
                "- `{}` — {} (.{}), {} {}, {} symbols",
                utils::escape_inline_code(&file.path),
                file.language.display_label(),
                file.extension,
                file.worktree_state.label(),
                file.status.label(),
                file.symbols.len()
            )
            .expect("writing to a string cannot fail");
            writeln!(
                output,
                "  - Structural snippets are shown in the ranked selection above."
            )
            .expect("writing to a string cannot fail");
            for limitation in &file.limitations {
                writeln!(output, "  - Limitation: {}", utils::sanitize_text(limitation))
                    .expect("writing to a string cannot fail");
            }
        }
    }
}
