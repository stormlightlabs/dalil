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
        history: None,
        map: None,
        explain: None,
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
        history: None,
        map: None,
        explain: None,
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
        history: None,
        map: None,
        explain: None,
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
        history: None,
        map: None,
        explain: None,
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

    let analysis = [
        include_str!("../../schema/v1/golden/briefing.json"),
        include_str!("../../schema/v1/golden/map.json"),
        include_str!("../../schema/v1/golden/history.json"),
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
