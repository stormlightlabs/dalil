use std::collections::BTreeMap;
use std::fmt::Write;

use crate::utils;

pub struct Render;

impl Render {
    fn commits(output: &mut String, commits: &[dalil_core::CommitEvidence]) {
        writeln!(output, "#### Evidence commits").expect("writing to a string cannot fail");
        if commits.is_empty() {
            writeln!(output, "No matching commits were found.").expect("writing to a string cannot fail");
        } else {
            for commit in commits {
                let paths =
                    if commit.paths.is_empty() { "no in-scope paths".to_owned() } else { commit.paths.join(", ") };
                writeln!(
                    output,
                    "- `{}` — {} ({}){}",
                    utils::escape_inline_code(&commit.id),
                    utils::sanitize_text(&commit.subject),
                    utils::sanitize_text(&paths),
                    if commit.matched_terms.is_empty() {
                        String::new()
                    } else {
                        format!(" — matched {}", utils::inline_code_list(&commit.matched_terms))
                    }
                )
                .expect("writing to a string cannot fail");
            }
        }
    }

    fn section_heading(output: &mut String, heading: &str) {
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "### {heading}").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
    }

    fn caveats(output: &mut String, caveats: &[String]) {
        if caveats.is_empty() {
            return;
        }
        writeln!(output, "Caveats:").expect("writing to a string cannot fail");
        for caveat in caveats {
            writeln!(output, "- {}", utils::sanitize_text(caveat)).expect("writing to a string cannot fail");
        }
    }

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

    pub fn history_markdown(output: &mut String, history: &dalil_core::HistoryReport) {
        Render::history_header(output, history);

        if let Some(churn) = &history.churn {
            Render::churn_markdown(output, churn);
        }
        if let Some(contributors) = &history.contributors {
            Render::contributors_markdown(output, contributors);
        }
        if let Some(bugs) = &history.bugs {
            Render::bugs_markdown(output, bugs);
        }
        if let Some(activity) = &history.activity {
            Render::activity_markdown(output, activity);
        }
        if let Some(firefighting) = &history.firefighting {
            Render::firefighting_markdown(output, firefighting);
        }
        Render::history_limitations(output, history);
    }

    pub fn history_briefing_markdown(output: &mut String, history: &dalil_core::HistoryReport) {
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "## History analysis").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(
            output,
            "Scope `{}`: {} reachable commits ({} non-merge).",
            utils::escape_inline_code(&history.scope_path),
            history.commits_seen,
            history.non_merge_commits_seen
        )
        .expect("writing to a string cannot fail");
        Render::section_heading(output, "History observations");
        if history.observations.is_empty() {
            writeln!(
                output,
                "No distinct observations were supported by the available history evidence."
            )
            .expect("writing to a string cannot fail");
        } else {
            for observation in &history.observations {
                Render::history_observation(output, observation);
            }
        }
        writeln!(
            output,
            "Detailed history evidence: use `dalil history`, a focused history subcommand, `--profile evidence`, or `--json`."
        )
        .expect("writing to a string cannot fail");
        Render::history_limitations(output, history);
    }

    pub fn briefing_evidence_notes(output: &mut String, map: &dalil_core::MapReport) {
        let has_notes = map.classifications.total > 0
            || map.availability.unsupported_paths > 0
            || map.availability.partial_files > 0
            || map.collections.files.truncated
            || map.collections.omissions.truncated;
        if !has_notes {
            return;
        }
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "## Evidence notes").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
        if map.classifications.total > 0 {
            writeln!(
                output,
                "- Excluded {} generated, vendor, minified, or source-map path(s) before parsing.",
                map.classifications.total
            )
            .expect("writing to a string cannot fail");
        }
        if map.availability.unsupported_paths > 0 {
            writeln!(
                output,
                "- {} source-like path(s) use an unsupported language; relevant paths are reflected in quality.",
                map.availability.unsupported_paths
            )
            .expect("writing to a string cannot fail");
        }
        if map.availability.partial_files > 0 {
            writeln!(
                output,
                "- {} analyzed file(s) contain bounded or incomplete structural evidence; recommendation limitations identify relevant cases.",
                map.availability.partial_files
            )
            .expect("writing to a string cannot fail");
        }
        if map.collections.files.truncated || map.collections.omissions.truncated {
            writeln!(
                output,
                "- Compact collections are projected; JSON retains totals and truncation reasons."
            )
            .expect("writing to a string cannot fail");
        }
        writeln!(
            output,
            "Detailed structural evidence: use `dalil map`, `dalil explain`, or `--json`."
        )
        .expect("writing to a string cannot fail");
    }

    fn history_header(output: &mut String, history: &dalil_core::HistoryReport) {
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(output, "## History analysis").expect("writing to a string cannot fail");
        writeln!(output).expect("writing to a string cannot fail");
        writeln!(
            output,
            "Repository: `{}`",
            utils::escape_inline_code(&history.repository_root)
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "History scope: `{}`",
            utils::escape_inline_code(&history.scope_path)
        )
        .expect("writing to a string cannot fail");
        writeln!(output, "Reachable commits: {}", history.commits_seen).expect("writing to a string cannot fail");
        writeln!(output, "Non-merge commits: {}", history.non_merge_commits_seen)
            .expect("writing to a string cannot fail");
        writeln!(
            output,
            "Windows: {} days for churn/bugs/firefighting; {} days for recent contributors",
            history.settings.window_days, history.settings.recent_window_days
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "Bug keywords: {}",
            utils::inline_code_list(&history.settings.bug_keywords)
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "Firefighting keywords: {}",
            utils::inline_code_list(&history.settings.firefighting_keywords)
        )
        .expect("writing to a string cannot fail");
        writeln!(output, "Keyword matching: {}", history.settings.keyword_match.label())
            .expect("writing to a string cannot fail");
        if history.collections.commits.truncated
            || history.collections.churn_paths.truncated
            || history.collections.contributor_identity_mappings.truncated
            || history.collections.contributors_overall.truncated
            || history.collections.contributors_recent.truncated
            || history.collections.bug_paths.truncated
            || history.collections.bug_overlap_paths.truncated
            || history.collections.bug_commits.truncated
            || history.collections.activity_months.truncated
            || history.collections.firefighting_commits.truncated
        {
            writeln!(
                output,
                "Evidence collections are bounded; JSON contains totals and truncation reasons."
            )
            .expect("writing to a string cannot fail");
        }
    }

    fn history_limitations(output: &mut String, history: &dalil_core::HistoryReport) {
        for limitation in &history.limitations {
            writeln!(output, "- Limitation: {}", utils::sanitize_text(limitation))
                .expect("writing to a string cannot fail");
        }
    }

    fn history_observation(output: &mut String, observation: &dalil_core::HistoryObservation) {
        match observation {
            dalil_core::HistoryObservation::Churn { paths, window_days, caveat } => {
                writeln!(
                    output,
                    "- **Churn:** {} changed over the last {} days. Caveat: {}",
                    Render::path_counts_inline(paths),
                    window_days,
                    utils::sanitize_text(caveat)
                )
                .expect("writing to a string cannot fail");
            }
            dalil_core::HistoryObservation::Contributors { contributor, total_commits, window_days, caveat } => {
                let window = window_days.map_or_else(
                    || "across observed history".to_owned(),
                    |days| format!("in the recent {days}-day window"),
                );
                writeln!(
                    output,
                    "- **Contributor concentration:** {} authored {} of {} non-merge commits ({}%) {}. Caveat: {}",
                    utils::sanitize_text(&contributor.name),
                    contributor.commits,
                    total_commits,
                    contributor.share_percent,
                    window,
                    utils::sanitize_text(caveat)
                )
                .expect("writing to a string cannot fail");
            }
            dalil_core::HistoryObservation::BugOverlap { paths, bug_commits, window_days, caveat } => {
                writeln!(
                    output,
                    "- **Bug/churn overlap:** {} overlapped across {} matching bug commits in the last {} days. Caveat: {}",
                    Render::path_counts_inline(paths),
                    bug_commits,
                    window_days,
                    utils::sanitize_text(caveat)
                )
                .expect("writing to a string cannot fail");
            }
            dalil_core::HistoryObservation::Activity { month, commits, observed_months, observed_commits, caveat } => {
                writeln!(
                    output,
                    "- **Activity:** `{}` was the busiest observed month with {} commits across {} observed commits and {} months. Caveat: {}",
                    utils::escape_inline_code(month),
                    commits,
                    observed_commits,
                    observed_months,
                    utils::sanitize_text(caveat)
                )
                .expect("writing to a string cannot fail");
            }
            dalil_core::HistoryObservation::Firefighting { commits, paths, window_days, caveat } => {
                writeln!(
                    output,
                    "- **Firefighting language:** {} matching commits touched {} over the last {} days. Caveat: {}",
                    commits,
                    Render::path_counts_inline(paths),
                    window_days,
                    utils::sanitize_text(caveat)
                )
                .expect("writing to a string cannot fail");
            }
        }
    }

    fn path_counts_inline(paths: &[dalil_core::PathCount]) -> String {
        paths
            .iter()
            .map(|path| format!("`{}` ({} commits)", utils::escape_inline_code(&path.path), path.commits))
            .collect::<Vec<_>>()
            .join(", ")
    }

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

    pub fn impact_markdown(output: &mut String, impact: &dalil_core::ImpactReport) {
        Render::section_heading(output, "Impact context");
        if let Some(task) = &impact.request.task {
            writeln!(output, "Task: {}", utils::sanitize_text(task)).expect("writing to a string cannot fail");
        }
        writeln!(output, "Change resolution: {}", impact.change_resolution.status.label())
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
                    "- `{}` ({} confidence; {}) — {}",
                    utils::escape_inline_code(&target.path),
                    target.confidence.label(),
                    evidence,
                    utils::sanitize_text(&target.reason),
                )
                .expect("writing to a string cannot fail");
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
                    "- {} `{}` to `{}`{} ({} confidence) — {}{}",
                    relationship.evidence.label(),
                    utils::escape_inline_code(&relationship.source),
                    utils::escape_inline_code(&relationship.target),
                    symbol,
                    relationship.confidence.label(),
                    utils::sanitize_text(&relationship.reason),
                    if relationship.ambiguous { "; ambiguous candidate" } else { "" },
                )
                .expect("writing to a string cannot fail");
            }
        }
        if !impact.likely_tests.is_empty() {
            Render::section_heading(output, "Likely tests");
            for test in &impact.likely_tests {
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

    fn explain_relationship_markdown(output: &mut String, edge: &dalil_core::LexicalEdge, indent: &str) {
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

    fn format_location(location: &dalil_core::SourceLocation) -> String {
        format!(
            "{}:{}-{}:{}",
            location.start.line, location.start.column, location.end.line, location.end.column
        )
    }

    fn churn_markdown(output: &mut String, churn: &dalil_core::ChurnReport) {
        Render::section_heading(output, "Churn hotspots");
        writeln!(output, "Window: {} days", churn.window_days).expect("writing to a string cannot fail");
        if churn.paths.is_empty() {
            writeln!(output, "No in-scope non-merge paths changed in this window.")
                .expect("writing to a string cannot fail");
        } else {
            for path in &churn.paths {
                let normalized = path.commits_per_kib_milli.map_or_else(
                    || {
                        format!(
                            "normalization unavailable ({})",
                            path.size_status.as_deref().unwrap_or("unknown")
                        )
                    },
                    |rate| {
                        format!(
                            "{:.3} commits/KiB ({})",
                            rate as f64 / 1_000.0,
                            path.size_status.as_deref().unwrap_or("text")
                        )
                    },
                );
                writeln!(
                    output,
                    "- `{}` — {} commits; {}",
                    utils::escape_inline_code(&path.path),
                    path.commits,
                    normalized
                )
                .expect("writing to a string cannot fail");
            }
        }
        writeln!(output, "Size basis: {}", utils::sanitize_text(&churn.size_basis))
            .expect("writing to a string cannot fail");
        writeln!(
            output,
            "Rename continuity: {} — {}",
            utils::sanitize_text(&churn.rename_continuity.status),
            utils::sanitize_text(&churn.rename_continuity.detail)
        )
        .expect("writing to a string cannot fail");
        Render::caveats(output, &churn.caveats);
    }

    fn contributors_markdown(output: &mut String, contributors: &dalil_core::ContributorReport) {
        Render::section_heading(output, "Contributor concentration");
        writeln!(output, "Committed .mailmap applied: {}", contributors.mailmap_applied)
            .expect("writing to a string cannot fail");
        if !contributors.identity_mappings.is_empty() {
            writeln!(
                output,
                "Canonicalized identities: {}",
                contributors.identity_mappings.len()
            )
            .expect("writing to a string cannot fail");
        }
        Render::contributors_group(output, "All non-merge commits", &contributors.overall);
        Render::contributors_group(output, "Recent non-merge commits", &contributors.recent);
        Render::caveats(output, &contributors.caveats);
    }

    fn contributors_group(output: &mut String, label: &str, contributors: &[dalil_core::ContributorCount]) {
        writeln!(output, "#### {label}").expect("writing to a string cannot fail");
        if contributors.is_empty() {
            writeln!(output, "No contributors were found.").expect("writing to a string cannot fail");
            return;
        }
        for contributor in contributors {
            let identity = contributor.email.as_ref().map_or_else(
                || utils::sanitize_text(&contributor.name),
                |email| {
                    format!(
                        "{} <{}>",
                        utils::sanitize_text(&contributor.name),
                        utils::sanitize_text(email)
                    )
                },
            );
            writeln!(
                output,
                "- {} — {} commits ({}%)",
                identity, contributor.commits, contributor.share_percent
            )
            .expect("writing to a string cannot fail");
        }
    }

    fn bugs_markdown(output: &mut String, bugs: &dalil_core::BugReport) {
        Render::section_heading(output, "Bug-related clusters");
        writeln!(output, "Window: {} days", bugs.window_days).expect("writing to a string cannot fail");
        writeln!(
            output,
            "Keywords ({} matching): {}",
            bugs.keyword_match.label(),
            utils::inline_code_list(&bugs.keywords)
        )
        .expect("writing to a string cannot fail");
        Render::paths(output, "Bug-related paths", &bugs.paths);
        Render::paths(output, "Churn overlap", &bugs.overlap_paths);
        Render::commits(output, &bugs.commits);
        Render::caveats(output, &bugs.caveats);
    }

    fn activity_markdown(output: &mut String, activity: &dalil_core::ActivityReport) {
        Render::section_heading(output, "Monthly activity");
        if activity.months.is_empty() {
            writeln!(output, "No commits were found.").expect("writing to a string cannot fail");
        } else {
            for month in &activity.months {
                writeln!(output, "- {} — {} commits", month.month, month.commits)
                    .expect("writing to a string cannot fail");
            }
        }
        Render::caveats(output, &activity.caveats);
    }

    fn firefighting_markdown(output: &mut String, firefighting: &dalil_core::FirefightingReport) {
        Render::section_heading(output, "Firefighting commits");
        writeln!(
            output,
            "Window: {} days; keywords ({} matching): {}",
            firefighting.window_days,
            firefighting.keyword_match.label(),
            utils::inline_code_list(&firefighting.keywords)
        )
        .expect("writing to a string cannot fail");
        Render::commits(output, &firefighting.commits);
        Render::caveats(output, &firefighting.caveats);
    }

    fn paths(output: &mut String, label: &str, paths: &[dalil_core::PathCount]) {
        writeln!(output, "#### {label}").expect("writing to a string cannot fail");
        if paths.is_empty() {
            writeln!(output, "No paths were found.").expect("writing to a string cannot fail");
        } else {
            for path in paths {
                writeln!(
                    output,
                    "- `{}` — {} commits",
                    utils::escape_inline_code(&path.path),
                    path.commits
                )
                .expect("writing to a string cannot fail");
            }
        }
    }
}
