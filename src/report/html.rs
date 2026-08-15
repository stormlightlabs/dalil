use std::path::Path;

use minijinja::{Environment, UndefinedBehavior};
use serde::Serialize;

use super::{
    CapabilitiesReport, CollectionSummary, CommandName, DoctorReport, HistoryObservation, HistoryReport, MapReport,
    Report, ReportError, ReportStatus, TOOL_VERSION,
};
use crate::map::CacheControlReport;

const REPORT_TEMPLATE: &str = include_str!("templates/report.html");
const REPORT_STYLESHEET: &str = include_str!("templates/report.css");
const REPORT_SCRIPT: &str = include_str!("templates/report.js");
const REPORT_ICON: &str = include_str!("templates/icon.svg");

#[derive(Serialize)]
struct HtmlPage {
    title: String,
    eyebrow: String,
    summary: String,
    command: String,
    captured_at: String,
    status: String,
    status_tone: &'static str,
    tool_version: String,
    metrics: Vec<Metric>,
    facts: Vec<Fact>,
    recommendations: Vec<Recommendation>,
    cards: Vec<Card>,
    evidence: Vec<Evidence>,
    notices: Vec<String>,
    report_json: String,
    icon: &'static str,
    stylesheet: &'static str,
    script: &'static str,
}

#[derive(Serialize)]
struct Metric {
    value: String,
    label: String,
}

impl Metric {
    fn new(value: impl ToString, label: impl Into<String>) -> Self {
        Self { value: value.to_string(), label: label.into() }
    }
}

#[derive(Serialize)]
struct Fact {
    label: String,
    value: String,
}

impl Fact {
    fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self { label: label.into(), value: value.into() }
    }
}

#[derive(Serialize)]
struct Recommendation {
    ordinal: String,
    purpose: &'static str,
    path: String,
    reason: String,
    confidence: &'static str,
    limitations: Vec<String>,
}

#[derive(Serialize)]
struct Card {
    title: String,
    meta: String,
    headline: Option<String>,
    items: Vec<CardItem>,
    caveat: Option<String>,
}

#[derive(Serialize)]
struct CardItem {
    label: String,
    value: String,
    percent: u8,
}

impl CardItem {
    fn new(label: impl Into<String>, value: impl ToString, percent: u8) -> Self {
        Self { label: label.into(), value: value.to_string(), percent }
    }
}

#[derive(Serialize)]
struct Evidence {
    value: String,
    label: String,
}

impl Evidence {
    fn collection(label: &str, collection: &CollectionSummary) -> Self {
        let value = if collection.total == collection.returned {
            collection.total.to_string()
        } else {
            format!("{} / {}", collection.total, collection.returned)
        };
        Self { value, label: label.to_owned() }
    }
}

pub(super) fn render_report(report: &Report) -> Result<String, ReportError> {
    let repository_name = Path::new(&report.provenance.repository.canonical_root)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("repository")
        .to_owned();
    let command_label = command_label(report);
    let mut metrics = Vec::new();
    let mut evidence = Vec::new();
    let mut cards = Vec::new();
    if let Some(map) = &report.map {
        metrics.push(Metric::new(map.inventory.analyzed, "source files"));
        metrics.push(Metric::new(map.collections.symbols.total, "symbols observed"));
        metrics.push(Metric::new(map.collections.edges.total, "lexical edges"));
        evidence.extend(map_evidence(map));
        cards.push(map_files_card(map));
    }
    if let Some(history) = &report.history {
        metrics.push(Metric::new(history.commits_seen, "reachable commits"));
        cards.extend(history_cards(history));
    }
    if metrics.is_empty() {
        metrics.push(Metric::new(report.findings.len(), "findings"));
        metrics.push(Metric::new(report.limitations.len(), "limitations"));
    }
    metrics.truncate(4);

    if let Some(explain) = &report.explain {
        let max_score = explain.ranking.iter().map(|item| item.score).max().unwrap_or(1);
        cards.push(Card {
            title: "Explanation ranking".to_owned(),
            meta: format!("{command_label} target"),
            headline: Some(explain.target.clone()),
            items: explain
                .ranking
                .iter()
                .map(|item| {
                    CardItem::new(
                        &item.path,
                        item.score,
                        ((item.score.saturating_mul(100) / max_score).min(100)) as u8,
                    )
                })
                .collect(),
            caveat: explain.limitations.first().cloned(),
        });
    }

    let recommendations = report
        .reading_plan
        .as_ref()
        .map(|plan| {
            plan.recommendations
                .iter()
                .map(|recommendation| Recommendation {
                    ordinal: format!("{:02}", recommendation.ordinal),
                    purpose: reading_purpose(recommendation.purpose),
                    path: recommendation.path.clone(),
                    reason: recommendation.reason.clone(),
                    confidence: recommendation.confidence.label(),
                    limitations: recommendation.limitations.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let languages = if report.provenance.languages.is_empty() {
        "none detected".to_owned()
    } else {
        report
            .provenance
            .languages
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut notices: Vec<String> = report
        .findings
        .iter()
        .map(|finding| format!("{}: {}", finding.title, finding.detail))
        .collect();
    notices.extend(report.limitations.iter().map(|limitation| limitation.detail.clone()));
    if report.quality.projection {
        notices.push(
            "This report is a bounded projection. JSON retains collection totals and truncation reasons.".to_owned(),
        );
    }
    if let Some(map) = &report.map
        && map.availability.partial_files > 0
    {
        notices.push(format!(
            "{} analyzed file(s) contain bounded or incomplete structural evidence.",
            map.availability.partial_files
        ));
    }

    render_page(HtmlPage {
        title: repository_name,
        eyebrow: format!("Repository / {command_label}"),
        summary: report.summary.clone(),
        command: command_text(report),
        captured_at: display_date(&report.provenance.captured_at),
        status: match report.status {
            ReportStatus::Foundation => "Foundation",
            ReportStatus::Analyzed => "Analyzed",
        }
        .to_owned(),
        status_tone: if report.quality.strict_issues.is_empty() { "success" } else { "warning" },
        tool_version: report.provenance.tool_version.clone(),
        metrics,
        facts: vec![
            Fact::new(
                "Profile",
                match report.profile {
                    super::AnalysisProfile::Compact => "compact",
                    super::AnalysisProfile::Evidence => "evidence",
                },
            ),
            Fact::new("Worktree", report.provenance.worktree.state.label()),
            Fact::new(
                "Branch",
                report.provenance.head.reference.as_deref().unwrap_or("not resolved"),
            ),
            Fact::new(
                "Revision",
                report
                    .provenance
                    .head
                    .oid
                    .as_deref()
                    .map(|oid| oid.chars().take(7).collect())
                    .unwrap_or_else(|| "not resolved".to_owned()),
            ),
            Fact::new("Languages", languages),
            Fact::new("Scope", report.scope.selected_path.clone()),
            Fact::new("Schema", format!("v{}", report.schema_version)),
        ],
        recommendations,
        cards,
        evidence,
        notices,
        report_json: serde_json::to_string_pretty(report)?,
        icon: REPORT_ICON,
        stylesheet: REPORT_STYLESHEET,
        script: REPORT_SCRIPT,
    })
}

pub(super) fn render_capabilities(report: &CapabilitiesReport) -> Result<String, ReportError> {
    let cards = report
        .languages
        .iter()
        .map(|language| Card {
            title: language.language.display_label().to_owned(),
            meta: language.extensions.join(", "),
            headline: None,
            items: vec![
                CardItem::new(
                    "Grammar",
                    format!("{} {}", language.grammar, language.grammar_version),
                    100,
                ),
                CardItem::new(
                    "Query pack",
                    format!("{} {}", language.query_pack, language.query_pack_version),
                    100,
                ),
            ],
            caveat: None,
        })
        .collect();
    render_page(HtmlPage {
        title: "Capabilities".to_owned(),
        eyebrow: "Dalil / support".to_owned(),
        summary: "Installed language grammars, query packs, and active report limits.".to_owned(),
        command: "dalil capabilities --format html".to_owned(),
        captured_at: "Current installation".to_owned(),
        status: if report.query_packs_valid { "Ready" } else { "Degraded" }.to_owned(),
        status_tone: if report.query_packs_valid { "success" } else { "warning" },
        tool_version: report.tool_version.clone(),
        metrics: vec![
            Metric::new(report.languages.len(), "languages"),
            Metric::new(report.limits.len(), "analysis profiles"),
            Metric::new(
                report.languages.iter().filter(|language| language.definitions).count(),
                "definition packs",
            ),
            Metric::new(
                report.languages.iter().filter(|language| language.references).count(),
                "reference packs",
            ),
        ],
        facts: vec![
            Fact::new("Schema", format!("v{}", report.schema_version)),
            Fact::new(
                "Query packs",
                if report.query_packs_valid { "valid" } else { "invalid" },
            ),
        ],
        recommendations: Vec::new(),
        cards,
        evidence: Vec::new(),
        notices: Vec::new(),
        report_json: serde_json::to_string_pretty(report)?,
        icon: REPORT_ICON,
        stylesheet: REPORT_STYLESHEET,
        script: REPORT_SCRIPT,
    })
}

pub(super) fn render_doctor(report: &DoctorReport) -> Result<String, ReportError> {
    let passing = report
        .checks
        .iter()
        .filter(|check| check.status == super::DoctorCheckStatus::Pass)
        .count();
    let warnings = report
        .checks
        .iter()
        .filter(|check| check.status == super::DoctorCheckStatus::Warn)
        .count();
    let failing = report
        .checks
        .iter()
        .filter(|check| check.status == super::DoctorCheckStatus::Fail)
        .count();
    render_page(HtmlPage {
        title: "Doctor".to_owned(),
        eyebrow: "Dalil / support".to_owned(),
        summary: "Repository discovery, cache, schema, query-pack, and safety checks.".to_owned(),
        command: "dalil doctor --format html".to_owned(),
        captured_at: "Current installation".to_owned(),
        status: if report.is_ok() { "Healthy" } else { "Needs attention" }.to_owned(),
        status_tone: if report.is_ok() { "success" } else { "warning" },
        tool_version: report.tool_version.clone(),
        metrics: vec![
            Metric::new(report.checks.len(), "checks"),
            Metric::new(passing, "passing"),
            Metric::new(warnings, "warnings"),
            Metric::new(failing, "failing"),
        ],
        facts: vec![
            Fact::new("Path", report.requested_path.clone()),
            Fact::new("Schema", format!("v{}", report.schema_version)),
            Fact::new("Source evidence", report.source_evidence_collected.to_string()),
            Fact::new("State changed", report.repository_state_changed.to_string()),
        ],
        recommendations: Vec::new(),
        cards: vec![Card {
            title: "Support checks".to_owned(),
            meta: format!("{} checks", report.checks.len()),
            headline: None,
            items: report
                .checks
                .iter()
                .map(|check| CardItem::new(&check.name, format!("{:?}", check.status), 100))
                .collect(),
            caveat: None,
        }],
        evidence: Vec::new(),
        notices: report
            .checks
            .iter()
            .filter(|check| check.status != super::DoctorCheckStatus::Pass)
            .map(|check| check.detail.clone())
            .collect(),
        report_json: serde_json::to_string_pretty(report)?,
        icon: REPORT_ICON,
        stylesheet: REPORT_STYLESHEET,
        script: REPORT_SCRIPT,
    })
}

pub(crate) fn render_cache(report: &CacheControlReport) -> Result<String, ReportError> {
    render_page(HtmlPage {
        title: "Cache".to_owned(),
        eyebrow: format!("Dalil / {}", report.operation),
        summary: "Retained source-analysis cache metadata and configured limits.".to_owned(),
        command: format!("dalil cache {} --format html", report.operation),
        captured_at: "Current installation".to_owned(),
        status: if report.exists { "Available" } else { "Not configured" }.to_owned(),
        status_tone: if report.exists { "success" } else { "warning" },
        tool_version: TOOL_VERSION.to_owned(),
        metrics: vec![
            Metric::new(report.repositories, "repositories"),
            Metric::new(report.records, "records"),
            Metric::new(format_bytes(report.bytes), "stored"),
            Metric::new(report.removed_records, "records removed"),
        ],
        facts: vec![
            Fact::new(
                "Path",
                report.path.clone().unwrap_or_else(|| "not configured".to_owned()),
            ),
            Fact::new("Record limit", report.max_records_per_repository.to_string()),
            Fact::new("Byte limit", format_bytes(report.max_bytes_per_repository)),
            Fact::new("Maximum age", format!("{} seconds", report.max_age_seconds)),
        ],
        recommendations: Vec::new(),
        cards: Vec::new(),
        evidence: Vec::new(),
        notices: Vec::new(),
        report_json: serde_json::to_string_pretty(report)?,
        icon: REPORT_ICON,
        stylesheet: REPORT_STYLESHEET,
        script: REPORT_SCRIPT,
    })
}

fn render_page(page: HtmlPage) -> Result<String, ReportError> {
    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    environment.add_template("report.html", REPORT_TEMPLATE)?;
    let mut output = environment.get_template("report.html")?.render(page)?;
    output.push('\n');
    Ok(output)
}

fn map_evidence(map: &MapReport) -> Vec<Evidence> {
    vec![
        Evidence::collection("source files", &map.collections.files),
        Evidence::collection("symbols", &map.collections.symbols),
        Evidence::collection("lexical edges", &map.collections.edges),
        Evidence::collection("repository landmarks", &map.collections.landmarks),
        Evidence::collection("project roots", &map.collections.project_roots),
        Evidence::collection("omitted paths", &map.collections.omissions),
    ]
}

fn map_files_card(map: &MapReport) -> Card {
    Card {
        title: "Source map".to_owned(),
        meta: format!("{} analyzed files", map.inventory.analyzed),
        headline: None,
        items: map
            .files
            .iter()
            .map(|file| CardItem::new(&file.path, file.language.display_label(), 100))
            .collect(),
        caveat: map.limitations.first().cloned(),
    }
}

fn history_cards(history: &HistoryReport) -> Vec<Card> {
    if !history.observations.is_empty() {
        return history.observations.iter().map(observation_card).collect();
    }
    let mut cards = Vec::new();
    if let Some(churn) = &history.churn {
        cards.push(counts_card(
            "Churn hotspots",
            format!("{}-day window", churn.window_days),
            &churn.paths,
            churn.caveats.first().cloned(),
        ));
    }
    if let Some(contributors) = &history.contributors {
        let source = if contributors.recent.is_empty() { &contributors.overall } else { &contributors.recent };
        cards.push(Card {
            title: "Contributors".to_owned(),
            meta: format!("{}-day recent window", contributors.recent_window_days),
            headline: None,
            items: source
                .iter()
                .map(|contributor| CardItem::new(&contributor.name, contributor.commits, contributor.share_percent))
                .collect(),
            caveat: contributors.caveats.first().cloned(),
        });
    }
    if let Some(bugs) = &history.bugs {
        let source = if bugs.overlap_paths.is_empty() { &bugs.paths } else { &bugs.overlap_paths };
        cards.push(counts_card(
            "Bug-history overlap",
            format!("{} matching commits", bugs.commits.len()),
            source,
            bugs.caveats.first().cloned(),
        ));
    }
    if let Some(activity) = &history.activity {
        let max = activity.months.iter().map(|month| month.commits).max().unwrap_or(1);
        cards.push(Card {
            title: "Monthly activity".to_owned(),
            meta: format!("{} observed months", activity.months.len()),
            headline: None,
            items: activity
                .months
                .iter()
                .map(|month| {
                    CardItem::new(
                        &month.month,
                        month.commits,
                        ((month.commits.saturating_mul(100) / max).min(100)) as u8,
                    )
                })
                .collect(),
            caveat: activity.caveats.first().cloned(),
        });
    }
    if let Some(firefighting) = &history.firefighting {
        cards.push(Card {
            title: "Firefighting language".to_owned(),
            meta: format!("{}-day window", firefighting.window_days),
            headline: None,
            items: firefighting
                .commits
                .iter()
                .map(|commit| CardItem::new(&commit.subject, &commit.id, 100))
                .collect(),
            caveat: firefighting.caveats.first().cloned(),
        });
    }
    cards
}

fn observation_card(observation: &HistoryObservation) -> Card {
    match observation {
        HistoryObservation::Churn { paths, window_days, caveat } => counts_card(
            "Frequently changed paths",
            format!("{window_days}-day window"),
            paths,
            Some(caveat.clone()),
        ),
        HistoryObservation::Contributors { contributor, total_commits, window_days, caveat } => Card {
            title: "Contributor concentration".to_owned(),
            meta: window_days.map_or_else(|| "observed history".to_owned(), |days| format!("{days}-day window")),
            headline: Some(format!(
                "{} authored {} of {} commits ({}%)",
                contributor.name, contributor.commits, total_commits, contributor.share_percent
            )),
            items: Vec::new(),
            caveat: Some(caveat.clone()),
        },
        HistoryObservation::BugOverlap { paths, bug_commits, window_days, caveat } => counts_card(
            "Bug-history overlap",
            format!("{bug_commits} matching commit(s) / {window_days} days"),
            paths,
            Some(caveat.clone()),
        ),
        HistoryObservation::Activity { month, commits, observed_months, caveat, .. } => Card {
            title: "Recent activity".to_owned(),
            meta: format!("{observed_months} observed months"),
            headline: Some(format!("{commits} commits in {month}")),
            items: Vec::new(),
            caveat: Some(caveat.clone()),
        },
        HistoryObservation::Firefighting { commits, paths, window_days, caveat } => counts_card(
            "Firefighting language",
            format!("{commits} matching commit(s) / {window_days} days"),
            paths,
            Some(caveat.clone()),
        ),
    }
}

fn counts_card(title: &str, meta: String, paths: &[super::PathCount], caveat: Option<String>) -> Card {
    let max = paths.iter().map(|path| path.commits).max().unwrap_or(1);
    Card {
        title: title.to_owned(),
        meta,
        headline: None,
        items: paths
            .iter()
            .map(|path| {
                CardItem::new(
                    &path.path,
                    path.commits,
                    ((path.commits.saturating_mul(100) / max).min(100)) as u8,
                )
            })
            .collect(),
        caveat,
    }
}

fn command_label(report: &Report) -> String {
    report.command.operation.map_or_else(
        || report.command.name.label().to_owned(),
        |operation| format!("{} / {}", report.command.name.label(), operation.label()),
    )
}

fn command_text(report: &Report) -> String {
    let path = &report.scope.selected_path;
    match report.command.name {
        CommandName::Briefing => format!("dalil --format html {path}"),
        CommandName::Map => format!("dalil map --format html {path}"),
        CommandName::History => report.command.operation.map_or_else(
            || format!("dalil history --format html {path}"),
            |operation| format!("dalil history {} --format html {path}", operation.label()),
        ),
        CommandName::Explain => format!(
            "dalil explain {} --format html {path}",
            report.command.target.as_deref().unwrap_or("target")
        ),
        CommandName::Context => format!("dalil context --format html {path}"),
        CommandName::Impact => format!("dalil impact --format html {path}"),
    }
}

fn reading_purpose(purpose: super::ReadingPurpose) -> &'static str {
    match purpose {
        super::ReadingPurpose::StartHere => "Start here",
        super::ReadingPurpose::Architecture => "Architecture",
        super::ReadingPurpose::Runtime => "Runtime",
        super::ReadingPurpose::Tests => "Tests",
        super::ReadingPurpose::SupportingContext => "Supporting context",
    }
}

fn display_date(value: &str) -> String {
    value
        .get(..10)
        .filter(|date| date.len() == 10)
        .unwrap_or("Not captured")
        .to_owned()
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
