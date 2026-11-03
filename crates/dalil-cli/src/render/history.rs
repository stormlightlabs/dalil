use std::fmt::Write;

use super::Render;
use crate::utils;

impl Render {
    pub fn history_markdown(output: &mut String, history: &dalil_core::HistoryReport) {
        Render::history_header(output, history);

        let mut omitted = 0;
        if let Some(churn) = &history.churn {
            omitted += Render::churn_markdown(output, churn);
        }
        if let Some(contributors) = &history.contributors {
            omitted += Render::contributors_markdown(output, contributors);
        }
        if let Some(bugs) = &history.bugs {
            omitted += Render::bugs_markdown(output, bugs);
        }
        if let Some(activity) = &history.activity {
            omitted += Render::activity_markdown(output, activity);
        }
        if let Some(firefighting) = &history.firefighting {
            omitted += Render::firefighting_markdown(output, firefighting);
        }
        Render::history_limitations(output, history, omitted);
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
        let mut omitted = 0;
        if history.observations.is_empty() {
            writeln!(
                output,
                "No distinct observations were supported by the available history evidence."
            )
            .expect("writing to a string cannot fail");
        } else {
            for observation in &history.observations {
                omitted += usize::from(Render::history_observation(output, observation));
            }
        }
        writeln!(
            output,
            "Detailed history evidence: use `dalil history`, a focused history subcommand, `--profile evidence`, or `--json`."
        )
        .expect("writing to a string cannot fail");
        Render::history_limitations(output, history, omitted);
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

    /// Prints the history limitations, then the method limitations note when
    /// `omitted` caveats were left out.
    pub(super) fn history_limitations(output: &mut String, history: &dalil_core::HistoryReport, omitted: usize) {
        for limitation in &history.limitations {
            writeln!(output, "- Limitation: {}", utils::sanitize_text(limitation))
                .expect("writing to a string cannot fail");
        }
        if omitted > 0 {
            let bullet = if history.limitations.is_empty() { "" } else { "- " };
            writeln!(output, "{bullet}{}", utils::METHOD_LIMITATIONS_NOTE).expect("writing to a string cannot fail");
        }
    }

    /// Prints one observation and returns whether its caveat was a method
    /// limitation left out.
    pub(super) fn history_observation(output: &mut String, observation: &dalil_core::HistoryObservation) -> bool {
        let caveat = match observation {
            dalil_core::HistoryObservation::Churn { caveat, .. }
            | dalil_core::HistoryObservation::Contributors { caveat, .. }
            | dalil_core::HistoryObservation::BugOverlap { caveat, .. }
            | dalil_core::HistoryObservation::Activity { caveat, .. }
            | dalil_core::HistoryObservation::Firefighting { caveat, .. } => caveat,
        };
        match observation {
            dalil_core::HistoryObservation::Churn { paths, window_days, caveat } => {
                writeln!(
                    output,
                    "- **Churn:** {} changed over the last {} days{}",
                    Render::path_counts_inline(paths),
                    window_days,
                    Render::observation_caveat_suffix(caveat)
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
                    "- **Contributor concentration:** {} authored {} of {} non-merge commits ({}%) {}{}",
                    utils::sanitize_text(&contributor.name),
                    contributor.commits,
                    total_commits,
                    contributor.share_percent,
                    window,
                    Render::observation_caveat_suffix(caveat)
                )
                .expect("writing to a string cannot fail");
            }
            dalil_core::HistoryObservation::BugOverlap { paths, bug_commits, window_days, caveat } => {
                writeln!(
                    output,
                    "- **Bug/churn overlap:** {} overlapped across {} matching bug commits in the last {} days{}",
                    Render::path_counts_inline(paths),
                    bug_commits,
                    window_days,
                    Render::observation_caveat_suffix(caveat)
                )
                .expect("writing to a string cannot fail");
            }
            dalil_core::HistoryObservation::Activity { month, commits, observed_months, observed_commits, caveat } => {
                writeln!(
                    output,
                    "- **Activity:** `{}` was the busiest observed month with {} commits across {} observed commits and {} months{}",
                    utils::escape_inline_code(month),
                    commits,
                    observed_commits,
                    observed_months,
                    Render::observation_caveat_suffix(caveat)
                )
                .expect("writing to a string cannot fail");
            }
            dalil_core::HistoryObservation::Firefighting { commits, paths, window_days, caveat } => {
                writeln!(
                    output,
                    "- **Firefighting language:** {} matching commits touched {} over the last {} days{}",
                    commits,
                    Render::path_counts_inline(paths),
                    window_days,
                    Render::observation_caveat_suffix(caveat)
                )
                .expect("writing to a string cannot fail");
            }
        }
        dalil_core::limitations::is_method_limitation(caveat)
    }

    /// `. Caveat: <text>` for a result caveat; a full stop for a method limitation.
    fn observation_caveat_suffix(caveat: &str) -> String {
        Render::observation_caveat(caveat).map_or_else(|| ".".to_owned(), |caveat| format!(". Caveat: {caveat}"))
    }

    /// Prints the caveats a history section reached and returns the number
    /// of method limitations left out.
    fn history_caveats(output: &mut String, caveats: &[String]) -> usize {
        let (kept, omitted) = utils::result_limitations(caveats.iter().map(String::as_str));
        Render::result_list(output, super::ListHeading::Label("Caveats:"), &kept, 0);
        omitted
    }

    fn path_counts_inline(paths: &[dalil_core::PathCount]) -> String {
        paths
            .iter()
            .map(|path| format!("`{}` ({} commits)", utils::escape_inline_code(&path.path), path.commits))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn churn_markdown(output: &mut String, churn: &dalil_core::ChurnReport) -> usize {
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
        Render::history_caveats(output, &churn.caveats)
    }

    fn contributors_markdown(output: &mut String, contributors: &dalil_core::ContributorReport) -> usize {
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
        Render::history_caveats(output, &contributors.caveats)
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

    fn bugs_markdown(output: &mut String, bugs: &dalil_core::BugReport) -> usize {
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
        Render::history_caveats(output, &bugs.caveats)
    }

    fn activity_markdown(output: &mut String, activity: &dalil_core::ActivityReport) -> usize {
        Render::section_heading(output, "Monthly activity");
        if activity.months.is_empty() {
            writeln!(output, "No commits were found.").expect("writing to a string cannot fail");
        } else {
            for month in &activity.months {
                writeln!(output, "- {} — {} commits", month.month, month.commits)
                    .expect("writing to a string cannot fail");
            }
        }
        Render::history_caveats(output, &activity.caveats)
    }

    fn firefighting_markdown(output: &mut String, firefighting: &dalil_core::FirefightingReport) -> usize {
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
        Render::history_caveats(output, &firefighting.caveats)
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
