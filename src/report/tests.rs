use super::*;

use crate::report::{CommandDescriptor, Finding, Limitation, ReportScope, ReportStatus, SCHEMA_VERSION};
use std::path::PathBuf;

#[test]
fn markdown_escapes_report_content_that_could_add_control_sequences() {
    let report = Report {
        schema_version: SCHEMA_VERSION,
        profile: AnalysisProfile::Compact,
        limits: ReportLimits::for_profile(AnalysisProfile::Compact),
        command: CommandDescriptor::map(PathBuf::from("unsafe\u{1b}[31m-path")),
        scope: ReportScope { selected_path: "unsafe\u{1b}[31m-path".to_owned() },
        status: ReportStatus::Foundation,
        summary: "A\u{1b}[31m summary".to_owned(),
        provenance: ReportProvenance::default(),
        quality: ReportQuality::default(),
        findings: vec![Finding { title: "title*".to_owned(), detail: "detail\u{7}".to_owned() }],
        limitations: vec![Limitation { detail: "limitation\u{1b}[0m".to_owned() }],
        reading_plan: None,
        orientation: None,
        history: None,
        map: None,
        explain: None,
        context: None,
        impact: None,
        search: None,
    };

    let markdown = report.render(OutputFormat::Markdown).expect("markdown renders");
    assert!(!markdown.contains('\u{1b}'));
    assert!(!markdown.contains('\u{7}'));
    assert!(markdown.contains("title\\*"));
}

#[test]
fn compact_markdown_applies_the_map_token_budget_to_the_whole_report() {
    let mut provenance = ReportProvenance::default();
    provenance.effective_options.map.map_tokens = 100;
    let report = Report {
        schema_version: SCHEMA_VERSION,
        profile: AnalysisProfile::Compact,
        limits: ReportLimits::for_profile(AnalysisProfile::Compact),
        command: CommandDescriptor::map(PathBuf::from(".")),
        scope: ReportScope { selected_path: ".".to_owned() },
        status: ReportStatus::Analyzed,
        summary: "A concise summary that must remain available.".to_owned(),
        provenance,
        quality: ReportQuality::default(),
        findings: (0..40)
            .map(|index| Finding {
                title: format!("finding {index}"),
                detail: "Detailed evidence that belongs in the complete typed report.".repeat(4),
            })
            .collect(),
        limitations: vec![],
        reading_plan: None,
        orientation: None,
        history: None,
        map: None,
        explain: None,
        context: None,
        impact: None,
        search: None,
    };

    let markdown = report.render(OutputFormat::Markdown).expect("markdown renders");

    assert!(crate::utils::token_count(&markdown) <= 100);
    assert!(markdown.starts_with("# Dalil map\n"));
    assert!(markdown.contains("A concise summary that must remain available."));
    assert!(markdown.contains("Report truncated at the compact Markdown token budget"));

    let json = report.render(OutputFormat::Json).expect("JSON renders");
    assert!(json.contains("finding 39"));
    assert!(!json.contains("Report truncated at the compact Markdown token budget"));
}

#[test]
fn evidence_markdown_is_not_projected_to_the_compact_token_budget() {
    let mut provenance = ReportProvenance::default();
    provenance.effective_options.map.map_tokens = 100;
    let report = Report {
        schema_version: SCHEMA_VERSION,
        profile: AnalysisProfile::Evidence,
        limits: ReportLimits::for_profile(AnalysisProfile::Evidence),
        command: CommandDescriptor::map(PathBuf::from(".")),
        scope: ReportScope { selected_path: ".".to_owned() },
        status: ReportStatus::Analyzed,
        summary: "Evidence summary.".to_owned(),
        provenance,
        quality: ReportQuality::default(),
        findings: (0..40)
            .map(|index| Finding {
                title: format!("finding {index}"),
                detail: "Detailed evidence remains available in evidence Markdown.".repeat(4),
            })
            .collect(),
        limitations: vec![],
        reading_plan: None,
        orientation: None,
        history: None,
        map: None,
        explain: None,
        context: None,
        impact: None,
        search: None,
    };

    let markdown = report.render(OutputFormat::Markdown).expect("markdown renders");

    assert!(crate::utils::token_count(&markdown) > 100);
    assert!(!markdown.contains("Report truncated at the compact Markdown token budget"));
    assert!(markdown.contains("finding 39"));
}

#[test]
fn html_is_embedded_deterministic_and_escapes_report_content() {
    let report = Report {
        schema_version: SCHEMA_VERSION,
        profile: AnalysisProfile::Compact,
        limits: ReportLimits::for_profile(AnalysisProfile::Compact),
        command: CommandDescriptor::map(PathBuf::from(".")),
        scope: ReportScope { selected_path: "\"><img src=x onerror=alert(1)>".to_owned() },
        status: ReportStatus::Foundation,
        summary: "<script>alert('report')</script>".to_owned(),
        provenance: ReportProvenance::default(),
        quality: ReportQuality::default(),
        findings: vec![Finding { title: "<strong>unsafe</strong>".to_owned(), detail: "detail".to_owned() }],
        limitations: vec![],
        reading_plan: None,
        orientation: None,
        history: None,
        map: None,
        explain: None,
        context: None,
        impact: None,
        search: None,
    };

    let first = report.render(OutputFormat::Html).expect("HTML renders");
    let second = report
        .render(OutputFormat::Html)
        .expect("HTML renders deterministically");

    assert_eq!(first, second);
    assert!(first.starts_with("<!doctype html>"));
    assert!(first.contains("family=Google+Sans"));
    assert!(first.contains("family=Google+Sans+Code"));
    assert!(first.contains("family=IBM+Plex+Sans"));
    assert!(first.contains("--font-heading: \"Google Sans\""));
    assert!(first.contains("--font-body: \"IBM Plex Sans\""));
    assert!(first.contains("&lt;script&gt;alert"));
    assert!(first.contains("&lt;img src=x onerror=alert"));
    assert!(!first.contains("<script>alert('report')</script>"));
    assert!(!first.contains("linear-gradient"));
}

#[test]
fn schema_and_golden_v1_corpus_cover_all_report_variants() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../schema/v1/dalil.json")).expect("schema is valid JSON");
    assert_eq!(
        schema["$defs"]["analysis_report"]["properties"]["schema_version"]["const"],
        1
    );
    assert!(
        schema["$defs"]["analysis_report"]["required"]
            .as_array()
            .expect("analysis required fields")
            .iter()
            .any(|field| field == "command")
    );
    assert!(schema["$defs"]["analysis_report"]["properties"]["reading_plan"].is_object());
    assert!(schema["$defs"]["analysis_report"]["properties"]["orientation"].is_object());
    assert!(schema["$defs"]["analysis_report"]["properties"]["context"].is_object());
    assert!(schema["$defs"]["analysis_report"]["properties"]["impact"].is_object());
    assert!(schema["$defs"]["analysis_report"]["properties"]["search"].is_object());
    assert!(
        schema["$defs"]["command"]["properties"]["name"]
            .to_string()
            .contains("orient")
    );
    assert!(
        schema["$defs"]["command"]["properties"]["name"]
            .to_string()
            .contains("context")
    );
    assert!(
        schema["$defs"]["command"]["properties"]["name"]
            .to_string()
            .contains("impact")
    );
    assert!(
        schema["$defs"]["command"]["properties"]["name"]
            .to_string()
            .contains("search")
    );

    let analysis = [
        include_str!("../../schema/v1/golden/briefing.json"),
        include_str!("../../schema/v1/golden/orient.json"),
        include_str!("../../schema/v1/golden/map.json"),
        include_str!("../../schema/v1/golden/history.json"),
        include_str!("../../schema/v1/golden/context.json"),
        include_str!("../../schema/v1/golden/search.json"),
    ];
    for document in analysis {
        let report: Report = serde_json::from_str(document).expect("historical v1 report remains readable");
        assert_eq!(report.schema_version, SCHEMA_VERSION);
    }
    let capabilities: CapabilitiesReport =
        serde_json::from_str(include_str!("../../schema/v1/golden/capabilities.json"))
            .expect("capabilities golden remains readable");
    assert_eq!(capabilities.schema_version, SCHEMA_VERSION);
    let doctor: DoctorReport = serde_json::from_str(include_str!("../../schema/v1/golden/doctor.json"))
        .expect("doctor golden remains readable");
    assert_eq!(doctor.schema_version, SCHEMA_VERSION);
    assert!(!doctor.source_evidence_collected);
    assert!(!doctor.repository_state_changed);
}

#[test]
fn orientation_markdown_and_json_expose_only_the_typed_orientation() {
    let report: Report = serde_json::from_str(include_str!("../../schema/v1/golden/orient.json"))
        .expect("orientation fixture remains readable");

    let markdown = report
        .render(OutputFormat::Markdown)
        .expect("Markdown orientation fixture renders");
    let json: serde_json::Value = serde_json::from_str(
        &report
            .render(OutputFormat::Json)
            .expect("JSON orientation fixture renders"),
    )
    .expect("orientation JSON remains valid");

    assert_eq!(report.command.name, CommandName::Orient);
    assert!(markdown.contains("## Repository overview"));
    assert!(markdown.contains("## Start here"));
    assert!(!markdown.contains("## Source map"));
    assert!(!markdown.contains("## History analysis"));
    assert!(json["orientation"].is_object());
    assert!(json.get("map").is_none());
    assert!(json.get("history").is_none());
    assert!(json.get("reading_plan").is_none());
}

#[test]
fn explain_guidance_retains_full_evidence_and_renders_every_guidance_kind() {
    let symbol = SourceSymbol {
        name: "Controller".to_owned(),
        kind: SymbolKind::Struct,
        role: SymbolRole::Definition,
        scope: Vec::new(),
        location: SourceLocation { start: Position { line: 1, column: 1 }, end: Position { line: 1, column: 18 } },
        context: "pub struct Controller;".to_owned(),
        visibility: SymbolVisibility::Public,
        evidence: SymbolEvidence::Declaration,
    };
    let mut map: MapReport = serde_json::from_value(serde_json::json!({
        "profile": "compact",
        "repository_root": "/fixture",
        "scope_path": ".",
        "query_pack": "rust-v1",
        "exclusions": [],
        "task_seeds": {"task": "inspect controller", "symbols": [], "paths": [], "languages": [], "projects": [], "changes": [], "search_terms": ["controller"]},
        "inventory": {"tracked": 2, "modified": 0, "untracked": 0, "analyzed": 2, "omitted": 0},
        "files": [],
        "omissions": [],
        "findings": [{"kind": "ambiguous_reference", "path": "src/subsystem.rs", "location": null, "detail": "Controller has more than one lexical candidate."}],
        "limitations": [],
        "edges": [],
        "ranking": [],
        "selection": {
            "token_budget": 1,
            "estimated_tokens": 0,
            "snippets": [],
            "omitted_relevant_paths": [{"path": "src/subsystem.rs", "reason": "the token budget retained no snippet for this task-relevant path"}],
            "shortfall": {"target_minimum": 3, "returned": 0, "reason": "fixture budget"}
        },
        "cache": {"mode": "disabled", "status": "disabled", "hits": 0, "misses": 0, "refreshed": [], "stale": []}
    }))
    .expect("valid explain fixture map");
    let edge = LexicalEdge {
        source: "src/main.rs".to_owned(),
        target: "src/subsystem.rs".to_owned(),
        symbol: "Controller".to_owned(),
        ambiguous: false,
        candidates: vec!["src/subsystem.rs".to_owned()],
        candidate_group: "fixture-controller".to_owned(),
        resolution_reason: LexicalResolutionReason::SameModule,
        confidence: ConfidenceTier::High,
        target_visibility: SymbolVisibility::Public,
    };
    map.reading_evidence = ReadingPlanEvidence {
        sources: vec![
            ReadingSourceEvidence { path: "src/main.rs".to_owned(), symbols: Vec::new(), limitations: Vec::new() },
            ReadingSourceEvidence {
                path: "src/subsystem.rs".to_owned(),
                symbols: vec![symbol],
                limitations: vec!["Only partial syntax evidence was retained for this source file.".to_owned()],
            },
        ],
        ranking: vec![
            FileRank {
                path: "src/subsystem.rs".to_owned(),
                score: 6_000_000,
                focus_matches: 0,
                contributions: RankingContributions {
                    centrality: 1_000_000,
                    seed_proximity: 0,
                    lexical_relevance: 4_000_000,
                    history_evidence: 1_000_000,
                    explicit_focus: 0,
                },
                matched_seeds: vec![RankingSeedMatch {
                    kind: RankingSeedKind::TaskTerm,
                    seed: "controller".to_owned(),
                }],
            },
            FileRank {
                path: "src/main.rs".to_owned(),
                score: 1_000_000,
                focus_matches: 0,
                contributions: RankingContributions::default(),
                matched_seeds: Vec::new(),
            },
        ],
        graph: vec![ReadingGraphEvidence {
            source: edge.source.clone(),
            target: edge.target.clone(),
            relationship: edge,
        }],
        omissions: Vec::new(),
        landmarks: Vec::new(),
        project_roots: Vec::new(),
    };
    map.collections.ranking = CollectionSummary::bounded(2, 0, TruncationReason::ProfileProjection);
    map.collections.edges = CollectionSummary::bounded(1, 0, TruncationReason::ProfileProjection);

    let history = HistoryReport {
        repository_root: "/fixture".to_owned(),
        scope_path: ".".to_owned(),
        head: HeadSnapshot::default(),
        provenance: HistoryProvenance::default(),
        settings: HistorySettings::default(),
        commits_seen: 1,
        non_merge_commits_seen: 1,
        collections: HistoryCollections::default(),
        limitations: Vec::new(),
        observations: Vec::new(),
        churn: None,
        contributors: None,
        bugs: Some(BugReport {
            window_days: 365,
            keywords: vec!["fix".to_owned()],
            keyword_match: KeywordMatchMode::Word,
            paths: Vec::new(),
            overlap_paths: vec![PathCount {
                path: "src/subsystem.rs".to_owned(),
                commits: 1,
                size_bytes: None,
                commits_per_kib_milli: None,
                size_status: None,
            }],
            commits: vec![CommitEvidence {
                id: "abc123".to_owned(),
                subject: "fix controller routing".to_owned(),
                paths: vec!["src/subsystem.rs".to_owned()],
                matched_terms: vec!["fix".to_owned()],
            }],
            caveats: Vec::new(),
        }),
        activity: None,
        firefighting: None,
    };

    let explain = crate::report::analysis::explain_report("Controller", &map, &history);
    let guidance = explain.guidance.first().expect("guidance for matched symbol");
    assert_eq!(guidance.path, "src/subsystem.rs");
    assert_eq!(guidance.confidence, ConfidenceTier::High);
    assert_eq!(
        guidance
            .ranking
            .as_ref()
            .map(|ranking| ranking.contributions.lexical_relevance),
        Some(4_000_000)
    );
    assert_eq!(guidance.relationships.len(), 1);
    assert_eq!(guidance.recent_commits.len(), 1);
    assert_eq!(guidance.ambiguity.len(), 1);
    assert!(
        guidance
            .limitations
            .iter()
            .any(|limitation| limitation.contains("partial syntax"))
    );
    assert!(
        guidance
            .truncation
            .iter()
            .any(|truncation| truncation.evidence == "snippet_selection")
    );
    assert_eq!(
        explain
            .next_read
            .as_ref()
            .map(|recommendation| recommendation.path.as_str()),
        Some("src/main.rs")
    );
    assert_eq!(
        explain
            .walkthrough
            .as_ref()
            .map(|walkthrough| walkthrough.paths.as_slice()),
        Some(["src/main.rs".to_owned(), "src/subsystem.rs".to_owned()].as_slice())
    );

    let report = Report {
        schema_version: SCHEMA_VERSION,
        profile: AnalysisProfile::Evidence,
        limits: ReportLimits::for_profile(AnalysisProfile::Evidence),
        command: CommandDescriptor::explain("Controller".to_owned(), PathBuf::from(".")),
        scope: ReportScope { selected_path: ".".to_owned() },
        status: ReportStatus::Analyzed,
        summary: "fixture explain".to_owned(),
        provenance: ReportProvenance::default(),
        quality: ReportQuality::default(),
        findings: Vec::new(),
        limitations: Vec::new(),
        reading_plan: None,
        orientation: None,
        history: None,
        map: None,
        explain: Some(explain),
        context: None,
        impact: None,
        search: None,
    };
    let markdown = report
        .render(OutputFormat::Markdown)
        .expect("Markdown explain fixture renders");
    let json: serde_json::Value =
        serde_json::from_str(&report.render(OutputFormat::Json).expect("JSON explain fixture renders"))
            .expect("JSON explain fixture remains valid");

    for expected in [
        "Reading guidance:",
        "Next read:",
        "Entry-point walkthrough:",
        "ambiguity **ambiguous_reference**",
        "partial syntax evidence",
        "snippet_selection evidence",
        "bug history: `abc123`",
    ] {
        assert!(
            markdown.contains(expected),
            "missing Markdown fixture evidence: {expected}"
        );
    }
    assert_eq!(
        json["explain"]["guidance"][0]["ranking"]["contributions"]["lexical_relevance"],
        4_000_000
    );
    assert_eq!(json["explain"]["next_read"]["path"], "src/main.rs");
    assert_eq!(
        json["explain"]["walkthrough"]["paths"],
        serde_json::json!(["src/main.rs", "src/subsystem.rs"])
    );
}
