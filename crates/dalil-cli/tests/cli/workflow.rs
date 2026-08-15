use super::*;

#[test]
fn explain_reports_bounded_reading_guidance_in_json_and_markdown() {
    let fixture = MapFixtureRepository::new();
    let output = fixture.run(&["explain", "duplicate", "--focus", "duplicate", "--no-cache", "--json"]);
    assert!(
        output.status.success(),
        "explain failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_str(&stdout(&output)).expect("valid explain JSON");
    assert_eq!(json["command"]["name"], "explain");
    assert_eq!(json["command"]["target"], "duplicate");
    assert_eq!(json["explain"]["target_kind"], "symbol");
    assert!(json["explain"]["matched_paths"].as_array().unwrap().len() >= 2);
    assert_eq!(json["explain"]["provenance"]["profile"], "compact");
    assert!(json["explain"]["provenance"]["source_files_analyzed"].as_u64().unwrap() >= 2);
    assert!(json["explain"]["guidance"].as_array().unwrap().len() >= 2);
    assert!(
        json["explain"]["guidance"]
            .as_array()
            .unwrap()
            .iter()
            .all(|guidance| guidance["ranking"]["contributions"].is_object())
    );
    assert!(
        json["explain"]["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|limitation| {
                limitation
                    .as_str()
                    .unwrap_or_default()
                    .contains("not a semantic call graph")
            })
    );

    let markdown = fixture.run(&["explain", "duplicate", "--focus", "duplicate", "--no-cache"]);
    assert!(markdown.status.success());
    assert!(markdown.stderr.is_empty());
    let markdown = stdout(&markdown);
    assert!(markdown.chars().count().div_ceil(4) <= 1_000);
    assert!(markdown.contains("Target: `duplicate` (symbol)"));
    assert!(markdown.contains("Provenance:"));
    assert!(markdown.contains("Reading guidance:"));
    assert!(markdown.contains("ranking: score"));
    assert!(markdown.contains("Next read:"));
    assert!(!markdown.contains("## History analysis"));
    assert!(!markdown.contains("## Source map"));
}

#[test]
fn search_returns_path_symbol_concept_and_budget_limited_anchors() {
    let fixture = MapFixtureRepository::new();

    let path = fixture.run(&["search", "src/one.rs", "--no-cache", "--json"]);
    assert!(
        path.status.success(),
        "path search failed: {}",
        String::from_utf8_lossy(&path.stderr)
    );
    let path: Value = serde_json::from_str(&stdout(&path)).expect("valid path search JSON");
    assert_eq!(path["command"]["name"], "search");
    assert_eq!(path["search"]["request"]["mode"], "plain");
    assert!(path["map"].is_null());
    assert!(path["history"].is_null());
    assert!(
        path["search"]["matches"]
            .as_array()
            .is_some_and(|matches| matches.iter().any(|result| result["path"] == "src/one.rs"))
    );
    let path_markdown = fixture.run(&["search", "src/one.rs", "--no-cache"]);
    assert!(path_markdown.status.success());
    assert!(stdout(&path_markdown).contains("`src/one.rs`"));

    let symbol = fixture.run(&["search", "--symbol", "duplicate", "--no-cache", "--json"]);
    assert!(
        symbol.status.success(),
        "symbol search failed: {}",
        String::from_utf8_lossy(&symbol.stderr)
    );
    let symbol: Value = serde_json::from_str(&stdout(&symbol)).expect("valid symbol search JSON");
    assert_eq!(symbol["search"]["request"]["mode"], "symbol");
    let matches = symbol["search"]["matches"].as_array().expect("symbol matches");
    assert!(matches.len() >= 2, "ambiguous symbol matches: {matches:?}");
    let symbol_matches = matches
        .iter()
        .filter(|result| result["target"] == "symbol")
        .collect::<Vec<_>>();
    assert!(symbol_matches.len() >= 2, "symbol matches: {matches:?}");
    assert!(
        symbol_matches
            .iter()
            .all(|result| result["symbol"]["name"] == "duplicate")
    );
    assert!(matches.iter().all(|result| {
        result["reason"].is_string()
            && result["evidence_kinds"]
                .as_array()
                .is_some_and(|evidence| !evidence.is_empty())
            && result["confidence"].is_string()
            && result["limitations"].is_array()
    }));
    let symbol_markdown = fixture.run(&["search", "--symbol", "duplicate", "--no-cache"]);
    assert!(symbol_markdown.status.success());
    assert!(stdout(&symbol_markdown).contains("Query: `duplicate` (symbol)"));

    let concept = fixture.run(&["search", "duplicate", "--no-cache", "--json"]);
    assert!(concept.status.success());
    let concept: Value = serde_json::from_str(&stdout(&concept)).expect("valid concept search JSON");
    assert!(
        concept["search"]["matches"]
            .as_array()
            .is_some_and(|matches| !matches.is_empty())
    );
    let ordinals = concept["search"]["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|result| result["ordinal"].as_u64().expect("ordinal"))
        .collect::<Vec<_>>();
    assert_eq!(ordinals, (1..=ordinals.len() as u64).collect::<Vec<_>>());
    assert!(
        concept["search"]["budget"]["estimated_tokens"]
            .as_u64()
            .is_some_and(|tokens| tokens <= 1_000)
    );

    let missing = fixture.run(&["search", "unfindable-search-fixture", "--no-cache", "--json"]);
    assert!(missing.status.success());
    let missing: Value = serde_json::from_str(&stdout(&missing)).expect("valid missing search JSON");
    assert!(missing["search"]["matches"].as_array().is_some_and(Vec::is_empty));
    assert!(
        missing["search"]["shortfall"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("No strong"))
    );
    let missing_markdown = fixture.run(&["search", "unfindable-search-fixture", "--no-cache"]);
    assert!(missing_markdown.status.success());
    assert!(stdout(&missing_markdown).contains("No strong anchors fit this search."));

    let limited = fixture.run(&["search", "duplicate", "--budget", "1", "--no-cache", "--json"]);
    assert!(limited.status.success());
    let limited: Value = serde_json::from_str(&stdout(&limited)).expect("valid limited search JSON");
    assert!(limited["search"]["matches"].as_array().is_some_and(Vec::is_empty));
    assert_eq!(limited["search"]["budget"]["token_budget"], 1);
    assert_eq!(limited["search"]["budget"]["estimated_tokens"], 1);
    assert!(
        limited["search"]["shortfall"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("token budget"))
    );
    let limited_markdown = fixture.run(&["search", "duplicate", "--budget", "1", "--no-cache"]);
    assert!(limited_markdown.status.success());
    assert!(stdout(&limited_markdown).contains("Shortfall:"));

    let markdown = fixture.run(&["search", "duplicate", "--no-cache"]);
    assert!(markdown.status.success());
    let markdown = stdout(&markdown);
    assert_plain_report(&markdown);
    assert!(markdown.contains("Search results"));
    assert!(markdown.contains("Query: `duplicate` (plain)"));
    assert!(!markdown.contains("Lexical dependency edges"));
}

#[test]
fn packaged_agent_skill_uses_supported_cli_workflows() {
    let skill = include_str!("../../skills/dalil/SKILL.md");

    for command in [
        "dalil orient --json",
        "dalil map --budget 750 --json",
        "dalil search 'cache invalidation' --json",
        "dalil search --symbol CacheStore --json",
        "dalil explain src/map/cache.rs --json",
        "dalil context --task 'fix parser cache invalidation' --changed-path src/map/cache.rs --budget 750 --json",
        "dalil context --task 'understand cache invalidation' --teach --budget 750 --json",
        "dalil impact --dirty-worktree --task 'review cache changes' --budget 750 --json",
        "dalil impact --revision-range 'HEAD~1..HEAD' --json",
    ] {
        assert!(skill.contains(command), "skill must document `{command}`");
    }
    for field in [
        "orientation",
        "context.relevant_tests",
        "context.next_reads",
        "quality",
        "uncertainty",
    ] {
        assert!(skill.contains(field), "skill must explain `{field}`");
    }

    let fixture = MapFixtureRepository::new();
    for (arguments, field) in [
        (&["orient", "--json"][..], "orientation"),
        (&["map", "--budget", "750", "--json"][..], "map"),
        (&["search", "cache invalidation", "--json"][..], "search"),
        (&["search", "--symbol", "CacheStore", "--json"][..], "search"),
        (&["explain", "src/map/cache.rs", "--json"][..], "explain"),
        (
            &[
                "context",
                "--task",
                "fix parser cache invalidation",
                "--changed-path",
                "src/map/cache.rs",
                "--budget",
                "750",
                "--json",
            ][..],
            "context",
        ),
        (
            &[
                "context",
                "--task",
                "understand cache invalidation",
                "--teach",
                "--budget",
                "750",
                "--json",
            ][..],
            "context",
        ),
        (
            &[
                "impact",
                "--dirty-worktree",
                "--task",
                "review cache changes",
                "--budget",
                "750",
                "--json",
            ][..],
            "impact",
        ),
        (&["impact", "--revision-range", "HEAD~1..HEAD", "--json"][..], "impact"),
    ] {
        let output = fixture.run(arguments);
        assert!(
            output.status.success(),
            "skill workflow `{arguments:?}` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_str(&stdout(&output)).expect("skill workflow emits JSON");
        assert!(
            report[field].is_object(),
            "skill workflow `{arguments:?}` returns `{field}`"
        );
    }
}

#[test]
fn context_compiles_one_budgeted_bundle_in_json_and_markdown() {
    let fixture = MapFixtureRepository::new();
    let arguments = [
        "context",
        "--task",
        "inspect duplicate resolution",
        "--symbol",
        "duplicate",
        "--changed-path",
        "src/use.rs",
        "--base",
        "main~1",
        "--head",
        "HEAD",
        "--dirty-worktree",
        "--budget",
        "1000",
        "--no-cache",
        "--json",
    ];
    let output = fixture.run(&arguments);
    assert!(
        output.status.success(),
        "context failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_str(&stdout(&output)).expect("valid context JSON");

    assert_eq!(json["command"]["name"], "context");
    assert!(json["map"].is_null());
    assert!(json["history"].is_null());
    assert_eq!(json["context"]["request"]["task"], "inspect duplicate resolution");
    assert_eq!(json["context"]["request"]["revision_context"]["base"], "main~1");
    assert_eq!(json["context"]["request"]["revision_context"]["head"], "HEAD");
    assert_eq!(json["context"]["request"]["revision_context"]["dirty_worktree"], true);
    assert!(json["context"]["orientation"].is_object());
    assert!(
        json["context"]["files"]
            .as_array()
            .is_some_and(|files| !files.is_empty())
    );
    assert!(
        json["context"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|file| file["recommendation"]["path"].is_string() && file["symbols"].is_array())
    );
    assert!(json["context"]["provenance"]["task_seeds"]["search_terms"].is_array());
    assert!(json["context"]["budget"]["estimated_tokens"].as_u64().unwrap() <= 1000);

    let markdown = fixture.run(&[
        "context",
        "--task",
        "inspect duplicate resolution",
        "--symbol",
        "duplicate",
        "--changed-path",
        "src/use.rs",
        "--budget",
        "1000",
        "--no-cache",
    ]);
    assert!(markdown.status.success());
    assert!(markdown.stderr.is_empty());
    let markdown = stdout(&markdown);
    assert_plain_report(&markdown);
    assert!(markdown.contains("Task context"));
    assert!(markdown.contains("Recommended files"));
    assert!(markdown.contains("Context budget"));
    assert!(!markdown.contains("## History analysis"));
    assert!(!markdown.contains("## Source map"));

    let cached_arguments = [
        "context",
        "--task",
        "inspect duplicate resolution",
        "--symbol",
        "duplicate",
        "--changed-path",
        "src/use.rs",
        "--budget",
        "1000",
        "--json",
    ];
    let cold = fixture.run(&cached_arguments);
    let warm = fixture.run(&cached_arguments);
    assert!(cold.status.success());
    assert!(warm.status.success());
    let mut cold_json: Value = serde_json::from_str(&stdout(&cold)).expect("valid cold context JSON");
    let mut warm_json: Value = serde_json::from_str(&stdout(&warm)).expect("valid warm context JSON");
    assert_eq!(cold_json["context"]["provenance"]["cache"]["status"], "refreshed");
    assert_eq!(warm_json["context"]["provenance"]["cache"]["status"], "hit");
    assert_eq!(cold_json["context"]["provenance"]["cache"]["index_status"], "refreshed");
    assert_eq!(warm_json["context"]["provenance"]["cache"]["index_status"], "hit");
    cold_json["context"]["provenance"]
        .as_object_mut()
        .expect("context provenance")
        .remove("cache");
    warm_json["context"]["provenance"]
        .as_object_mut()
        .expect("context provenance")
        .remove("cache");
    assert_eq!(cold_json["context"], warm_json["context"]);
}

#[test]
fn context_resolves_dirty_worktree_paths_and_changed_symbols() {
    let fixture = MapFixtureRepository::new();
    let output = fixture.run(&["context", "--dirty-worktree", "--no-cache", "--json"]);
    assert!(
        output.status.success(),
        "dirty context failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_str(&stdout(&output)).expect("valid dirty context JSON");
    let resolution = &json["context"]["change_resolution"];
    assert_eq!(resolution["status"], "resolved");
    let changes = resolution["changes"].as_array().expect("resolved changes");
    assert!(
        changes
            .iter()
            .any(|change| change["kind"] == "modified" && change["path"] == "src/lib.rs")
    );
    assert!(
        changes
            .iter()
            .any(|change| change["kind"] == "untracked" && change["path"] == "src/untracked.rs")
    );
    let lib = changes
        .iter()
        .find(|change| change["path"] == "src/lib.rs")
        .expect("modified lib change");
    assert!(lib["changed_lines"].as_array().is_some_and(|ranges| !ranges.is_empty()));
    assert!(lib["symbols"].as_array().is_some_and(|symbols| !symbols.is_empty()));
}

#[test]
fn impact_returns_budgeted_change_evidence_without_breakage_claims() {
    let fixture = MapFixtureRepository::new();
    fs::create_dir_all(fixture.root.join("tests")).expect("create impact test root");
    write_file(
        fixture.root.join("src/one.rs"),
        b"fn helper() {}\npub fn duplicate() { helper(); let changed = true; let _ = changed; }\n",
    );
    write_file(
        fixture.root.join("src/use.rs"),
        b"fn use_it() { duplicate(); let changed = true; let _ = changed; }\n",
    );
    write_file(
        fixture.root.join("tests/duplicate.rs"),
        b"#[test]\nfn duplicate_still_works() { duplicate(); }\n",
    );
    write_file(
        fixture.root.join("Cargo.toml"),
        b"[package]\nname = \"impact-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/one.rs\"\n",
    );
    write_file(fixture.root.join("CODEOWNERS"), b"src/ @maintainers\n");

    let output = fixture.run(&[
        "impact",
        "--dirty-worktree",
        "--task",
        "review duplicate implementation",
        "--focus",
        "duplicate",
        "--budget",
        "8000",
        "--no-cache",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "impact failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let json: Value = serde_json::from_str(&stdout(&output)).expect("valid impact JSON");
    assert_eq!(json["command"]["name"], "impact");
    assert!(json["context"].is_null());
    assert!(json["map"].is_null());
    assert!(json["history"].is_null());
    assert!(
        json["impact"]["change_resolution"]["changes"]
            .as_array()
            .is_some_and(|changes| changes.iter().any(|change| {
                change["path"] == "src/one.rs"
                    && change["symbols"]
                        .as_array()
                        .is_some_and(|symbols| symbols.iter().any(|symbol| symbol["name"] == "duplicate"))
            }))
    );
    assert!(
        json["impact"]["targets"]
            .as_array()
            .is_some_and(|targets| targets.iter().any(|target| target["path"] == "src/one.rs"))
    );
    assert!(json["impact"]["relationships"].as_array().is_some_and(|relationships| {
        relationships
            .iter()
            .any(|relationship| relationship["evidence"] == "lexical")
    }));
    assert!(json["impact"]["relationships"].as_array().is_some_and(|relationships| {
        relationships
            .iter()
            .any(|relationship| relationship["evidence"] == "manifest")
    }));
    assert!(json["impact"]["relationships"].as_array().is_some_and(|relationships| {
        relationships
            .iter()
            .any(|relationship| relationship["evidence"] == "structural")
    }));
    assert!(
        json["impact"]["likely_tests"]
            .as_array()
            .is_some_and(|tests| tests.iter().any(|test| test["path"] == "tests/duplicate.rs"))
    );
    assert!(
        json["impact"]["ownership"]
            .as_array()
            .is_some_and(|signals| signals.iter().any(|signal| signal["path"] == "CODEOWNERS"))
    );
    assert!(
        json["impact"]["budget"]["estimated_tokens"]
            .as_u64()
            .is_some_and(|tokens| tokens <= 8_000)
    );

    let markdown = fixture.run(&["impact", "--dirty-worktree", "--budget", "8000", "--no-cache"]);
    assert!(markdown.status.success());
    let markdown = stdout(&markdown);
    assert!(markdown.contains("Impact context"));
    assert!(markdown.contains("Evidence relationships"));
    assert!(markdown.contains("Impact relationships are bounded lexical"));
}

#[test]
fn impact_identifies_a_test_only_dirty_worktree_change() {
    let fixture = MapFixtureRepository::new();
    write_file(fixture.root.join("src/lib.rs"), b"pub fn parse() {}\n");
    fs::remove_file(fixture.root.join("src/untracked.rs")).expect("remove fixture implementation change");
    fs::create_dir_all(fixture.root.join("tests")).expect("create test-only fixture root");
    write_file(
        fixture.root.join("tests/parser.rs"),
        b"#[test]\nfn parser_handles_empty_input() {}\n",
    );

    let output = fixture.run(&["impact", "--dirty-worktree", "--no-cache", "--json"]);
    assert!(
        output.status.success(),
        "test-only impact failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_str(&stdout(&output)).expect("valid test-only impact JSON");
    assert!(
        json["impact"]["change_resolution"]["changes"]
            .as_array()
            .is_some_and(|changes| changes.iter().any(|change| change["path"] == "tests/parser.rs"))
    );
    assert!(
        json["impact"]["likely_tests"]
            .as_array()
            .is_some_and(|tests| tests.iter().any(|test| test["path"] == "tests/parser.rs"))
    );
}

#[test]
fn impact_keeps_ambiguous_lexical_candidates_explicit() {
    let fixture = MixedMapFixtureRepository::new();
    fs::remove_file(fixture.root.join("src/panel.jsx")).expect("remove fixture worktree change");
    write_file(
        fixture.root.join("src/duplicate_one.go"),
        b"package fixture\nfunc Duplicate() { helper() }\nfunc helper() {}\n",
    );

    let output = fixture.run(&["impact", "--dirty-worktree", "--no-cache", "--json"]);
    assert!(
        output.status.success(),
        "ambiguous impact failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_str(&stdout(&output)).expect("valid ambiguous impact JSON");
    assert!(json["impact"]["relationships"].as_array().is_some_and(|relationships| {
        relationships
            .iter()
            .any(|relationship| relationship["evidence"] == "lexical" && relationship["ambiguous"] == true)
    }));
}

#[test]
fn impact_reports_unsupported_changed_source_as_uncertainty() {
    let fixture = MapFixtureRepository::new();
    write_file(fixture.root.join("src/lib.rs"), b"pub fn parse() {}\n");
    fs::remove_file(fixture.root.join("src/untracked.rs")).expect("remove fixture implementation change");
    write_file(fixture.root.join("src/unsupported.swift"), b"func parse() {}\n");

    let output = fixture.run(&["impact", "--dirty-worktree", "--no-cache", "--json"]);
    assert!(
        output.status.success(),
        "unsupported impact failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_str(&stdout(&output)).expect("valid unsupported impact JSON");
    assert!(json["impact"]["uncertainty"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["kind"] == "change_unsupported_or_unavailable")
    }));
}

#[test]
fn context_teaching_scaffold_is_opt_in_and_cites_selected_evidence() {
    let fixture = MapFixtureRepository::new();
    write_file(
        fixture.root.join("src/main.rs"),
        b"pub struct RequestState;\nfn main() { let _state = RequestState; }\n",
    );
    let output = fixture.run(&[
        "context",
        "--task",
        "inspect the parser entry flow",
        "--teach",
        "--budget",
        "8000",
        "--no-cache",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "context teaching failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_str(&stdout(&output)).expect("valid teaching context JSON");
    let context = &json["context"];
    assert_eq!(context["request"]["teaching"], true);
    assert!(
        context["budget"]["estimated_tokens"]
            .as_u64()
            .is_some_and(|tokens| tokens <= 8_000)
    );
    let steps = context["teaching"]["steps"].as_array().expect("teaching steps");
    assert!(steps.iter().any(|step| step["topic"] == "behavior_start"));
    assert!(steps.iter().all(|step| step["observed"].is_array()));
    assert!(steps.iter().any(|step| step["ordering"] == "inferred"));
    let selected_paths = context["files"]
        .as_array()
        .expect("selected files")
        .iter()
        .filter_map(|file| file["recommendation"]["path"].as_str())
        .collect::<BTreeSet<_>>();
    assert!(
        steps
            .iter()
            .flat_map(|step| step["observed"].as_array().unwrap())
            .all(|evidence| {
                match evidence["kind"].as_str() {
                    Some("file") | Some("symbol") => evidence["path"]
                        .as_str()
                        .is_some_and(|path| selected_paths.contains(path)),
                    Some("relationship") => context["relationships"].as_array().is_some_and(|relationships| {
                        relationships
                            .iter()
                            .any(|relationship| relationship["source"] == evidence["path"])
                    }),
                    Some("test") => context["relevant_tests"]
                        .as_array()
                        .is_some_and(|tests| tests.iter().any(|test| test["path"] == evidence["path"])),
                    Some("next_read") => context["next_reads"]
                        .as_array()
                        .is_some_and(|reads| reads.iter().any(|read| read["path"] == evidence["path"])),
                    _ => false,
                }
            })
    );

    let compact = fixture.run(&[
        "context",
        "--task",
        "inspect the parser entry flow",
        "--teach",
        "--no-cache",
        "--json",
    ]);
    assert!(compact.status.success());
    let compact: Value = serde_json::from_str(&stdout(&compact)).expect("valid compact teaching context JSON");
    assert!(
        compact["context"]["teaching"]["steps"]
            .as_array()
            .is_some_and(|steps| steps.iter().any(|step| step["topic"] == "behavior_start"))
    );
    assert!(
        compact["context"]["budget"]["estimated_tokens"]
            .as_u64()
            .is_some_and(|tokens| tokens <= 1_000)
    );

    let markdown = fixture.run(&[
        "context",
        "--task",
        "inspect the parser entry flow",
        "--teach",
        "--budget",
        "8000",
        "--no-cache",
    ]);
    assert!(markdown.status.success());
    let markdown = stdout(&markdown);
    assert!(markdown.contains("Teaching scaffold"));
    assert!(markdown.contains("Observed file:"));
}
