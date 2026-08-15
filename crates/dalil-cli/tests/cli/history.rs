use super::*;

#[test]
fn history_without_commits_uses_the_analysis_exit_category() {
    let fixture = FixtureRepository::new();
    let output = fixture.run(&["history", "--json"]);

    assert_eq!(output.status.code(), Some(5));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("history analysis failed"));
}

#[test]
fn json_rendering_is_versioned_semantic_and_plain() {
    let fixture = MixedMapFixtureRepository::new();
    let output = fixture.run(&["--no-cache", "--json"]);
    let json = stdout(&output);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_plain_report(&json);

    let value: Value = serde_json::from_str(&json).expect("valid JSON report");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["command"]["name"], "orient");
    assert_eq!(value["status"], "analyzed");
    assert!(value["orientation"].is_object());
    assert!(value.get("history").is_none());
    assert!(value.get("map").is_none());
}

#[test]
fn machine_report_provenance_is_typed_and_repeated_runs_are_comparable() {
    let fixture = MapFixtureRepository::new();
    let first = fixture.run(&["map", "--no-cache", "--json"]);
    let second = fixture.run(&["map", "--no-cache", "--json"]);
    let first_json = stdout(&first);
    let value: Value = serde_json::from_str(&first_json).expect("valid provenance report");

    assert!(first.status.success());
    assert!(second.status.success());
    assert_eq!(first_json, stdout(&second));
    assert_eq!(value["provenance"]["tool_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["provenance"]["effective_options"]["format"], "json");
    assert_eq!(value["provenance"]["repository"]["object_format"], "sha1");
    assert!(
        value["provenance"]["repository"]["stable_id"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(value["provenance"]["head"]["reference"], "refs/heads/main");
    assert!(value["provenance"]["head"]["oid"].as_str().unwrap().len() >= 40);
    assert!(value["provenance"]["captured_at"].as_str().unwrap().contains('T'));
    assert_eq!(value["provenance"]["cache"]["status"], "disabled");
    assert!(value["provenance"]["languages"]["rust"]["grammar_version"].is_string());
    assert_eq!(value["provenance"]["worktree"]["state"], "mixed");
}

#[test]
fn capabilities_are_available_without_repository_analysis() {
    let fixture = FixtureRepository::new();
    let output = fixture.run(&["capabilities", "--json"]);
    let value: Value = serde_json::from_slice(&output.stdout).expect("valid capabilities JSON");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["report_kind"], "capabilities");
    assert_eq!(value["query_packs_valid"], true);
    assert_eq!(value["limits"]["compact"]["max_files"], 4_096);
    assert!(
        value["languages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|language| language["language"] == "java")
    );
    let go = value["languages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|language| language["language"] == "go")
        .expect("Go capability");
    assert_eq!(go["extensions"], serde_json::json!(["go"]));
    assert_eq!(go["grammar"], "tree-sitter-go");
    assert_eq!(go["grammar_version"], "0.25.0");
    assert_eq!(go["query_pack"], "go-v1");
    assert_eq!(go["definitions"], true);
    assert_eq!(go["references"], true);
    let lua = value["languages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|language| language["language"] == "lua")
        .expect("Lua capability");
    assert_eq!(lua["extensions"], serde_json::json!(["lua", "rockspec"]));
    assert_eq!(lua["grammar"], "tree-sitter-lua");
    assert_eq!(lua["grammar_version"], "0.5.0");
    assert_eq!(lua["query_pack"], "lua-v1");
    assert_eq!(lua["definitions"], true);
    assert_eq!(lua["references"], true);
    let zig = value["languages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|language| language["language"] == "zig")
        .expect("Zig capability");
    assert_eq!(zig["extensions"], serde_json::json!(["zig"]));
    assert_eq!(zig["grammar"], "tree-sitter-zig");
    assert_eq!(zig["grammar_version"], "1.1.2");
    assert_eq!(zig["query_pack"], "zig-v1");
    assert_eq!(zig["definitions"], true);
    assert_eq!(zig["references"], true);
}

#[test]
fn doctor_reports_support_health_without_source_evidence_or_repository_mutation() {
    let fixture = FixtureRepository::new();
    let before = fs::read(fixture.root.join(".git/HEAD")).expect("read HEAD before doctor");
    let output = fixture.run(&["doctor", "--json"]);
    let value: Value = serde_json::from_slice(&output.stdout).expect("valid doctor JSON");

    assert!(
        output.status.success(),
        "doctor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(value["report_kind"], "doctor");
    assert_eq!(value["source_evidence_collected"], false);
    assert_eq!(value["repository_state_changed"], false);
    assert!(
        value["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["name"] == "path_safety")
    );
    assert!(
        value["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["name"] == "query_packs")
    );
    assert_eq!(
        fs::read(fixture.root.join(".git/HEAD")).expect("read HEAD after doctor"),
        before
    );
    assert!(!stdout(&output).contains("pub fn"));
}

#[test]
fn strict_policy_renders_typed_partial_report_then_returns_analysis_failure() {
    let fixture = MapFixtureRepository::new();
    let output = fixture.run(&["map", "--strict", "--no-cache", "--json"]);
    let value: Value = serde_json::from_slice(&output.stdout).expect("strict mode still emits JSON");

    assert_eq!(output.status.code(), Some(5));
    assert_eq!(value["quality"]["partial"], true);
    assert!(
        value["quality"]["strict_issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue == "partial")
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("strict report policy rejected"));
}

#[test]
fn strict_quality_uses_complete_counts_when_compact_samples_are_truncated() {
    let fixture = FixtureRepository::new();
    for index in 0..8 {
        write_file(fixture.root.join(format!("a{index}.rs")), b"\0binary");
    }
    write_file(fixture.root.join("z.dart"), b"void unsupported() {}\n");

    let output = fixture.run(&["map", "--strict", "--no-cache", "--json"]);
    let value: Value = serde_json::from_slice(&output.stdout).expect("strict compact report is valid JSON");

    assert_eq!(output.status.code(), Some(5));
    assert_eq!(value["map"]["availability"]["unsupported_paths"], 1);
    assert_eq!(value["quality"]["unsupported"], true);
    assert!(
        !value["map"]["omissions"]
            .as_array()
            .expect("bounded omission sample")
            .iter()
            .any(|omission| omission["reason"] == "unsupported_language")
    );
}

#[test]
fn compact_projection_is_reported_without_becoming_actionable_quality() {
    let fixture = FixtureRepository::new();
    fs::create_dir_all(fixture.root.join("src")).expect("create projection source directory");
    for index in 0..40 {
        write_file(
            fixture.root.join(format!("src/file{index}.rs")),
            format!("pub fn file{index}() {{}}\n").as_bytes(),
        );
    }

    let output = fixture.run(&["map", "--strict", "--no-cache", "--json"]);
    let value: Value = serde_json::from_slice(&output.stdout).expect("compact projection JSON");

    assert!(
        output.status.success(),
        "projection should not fail strict policy: {:?}",
        output.stderr
    );
    assert!(output.stderr.is_empty());
    assert_eq!(value["quality"]["projection"], true);
    assert_eq!(value["quality"]["truncated"], true);
    assert_eq!(value["quality"]["resource_limited"], false);
    assert_eq!(value["quality"]["strict_issues"].as_array().unwrap().len(), 0);
    assert_eq!(value["map"]["collections"]["files"]["reason"], "profile_projection");
}

#[test]
fn irrelevant_unsupported_source_does_not_poison_orientation_but_focus_does() {
    let fixture = ClassificationFixtureRepository::new();
    write_file(fixture.root.join("src/unsupported.dart"), b"void unsupported() {}\n");

    let orientation = fixture.run(&["--strict", "--no-cache", "--json"]);
    let orientation_value: Value = serde_json::from_slice(&orientation.stdout).expect("orientation JSON");
    assert!(
        orientation.status.success(),
        "irrelevant unsupported source: {:?}",
        orientation.stderr
    );
    assert_eq!(orientation_value["quality"]["unsupported"], false);
    assert!(
        orientation_value["orientation"]["uncertainty"]
            .as_array()
            .unwrap()
            .iter()
            .any(|uncertainty| uncertainty["kind"] == "unsupported_source")
    );

    let focused = fixture.run(&[
        "map",
        "--strict",
        "--focus-path",
        "src/unsupported.dart",
        "--no-cache",
        "--json",
    ]);
    let focused_value: Value = serde_json::from_slice(&focused.stdout).expect("focused JSON");
    assert_eq!(focused.status.code(), Some(5));
    assert_eq!(focused_value["quality"]["unsupported"], true);
    assert!(
        focused_value["quality"]["strict_issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue == "unsupported")
    );
}

#[test]
fn history_completeness_marks_shallow_and_missing_objects_and_strict_rejects_them() {
    let shallow = HistoryFixtureRepository::new();
    let head = {
        let repository = gix::open(&shallow.root).expect("open shallow fixture");
        repository.head_id().expect("resolve shallow HEAD").to_string()
    };
    write_file(shallow.root.join(".git/shallow"), format!("{head}\n").as_bytes());

    let shallow_output = shallow.run(&["history", "--json"]);
    let shallow_value: Value = serde_json::from_slice(&shallow_output.stdout).expect("valid shallow JSON");
    assert!(shallow_output.status.success());
    assert_eq!(
        shallow_value["provenance"]["history"]["completeness"]["status"],
        "shallow"
    );
    assert_eq!(shallow_value["quality"]["incomplete"], true);

    let missing = HistoryFixtureRepository::new();
    let repository = gix::open(&missing.root).expect("open missing-object fixture");
    let head = repository.head_id().expect("resolve missing-object HEAD");
    let parent = repository
        .find_commit(head)
        .expect("read missing-object HEAD")
        .parent_ids()
        .next()
        .expect("fixture HEAD has a parent");
    let parent_text = parent.to_string();
    let parent_path = missing
        .root
        .join(".git/objects")
        .join(&parent_text[..2])
        .join(&parent_text[2..]);
    drop(repository);
    assert!(parent_path.is_file(), "fixture parent should be a loose object");
    fs::remove_file(parent_path).expect("remove one reachable Git object");

    let missing_output = missing.run(&["history", "--strict", "--json"]);
    let missing_value: Value = serde_json::from_slice(&missing_output.stdout).expect("valid missing-object JSON");
    assert_eq!(missing_output.status.code(), Some(5));
    assert_eq!(
        missing_value["provenance"]["history"]["completeness"]["status"],
        "missing_objects"
    );
    assert_eq!(missing_value["quality"]["incomplete"], true);
    assert!(String::from_utf8_lossy(&missing_output.stderr).contains("strict report policy rejected"));
}

#[test]
fn selected_paths_and_history_operations_are_preserved_in_the_typed_report() {
    let fixture = HistoryFixtureRepository::new();
    let output = fixture.run(&["history", "contributors", "src", "--include-emails", "--json"]);
    let json = stdout(&output);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let value: Value = serde_json::from_str(&json).expect("valid JSON report");
    assert_eq!(value["command"]["name"], "history");
    assert_eq!(value["command"]["operation"], "contributors");
    assert_eq!(value["scope"]["selected_path"], "src");
    assert_eq!(
        value["history"]["contributors"]["overall"]
            .as_array()
            .expect("scoped contributors")
            .iter()
            .find(|contributor| contributor["email"] == "alice@example.com")
            .expect("scoped Alice contributor")["commits"],
        1
    );
}

#[test]
fn history_json_contains_all_signals_evidence_and_required_caveats() {
    let fixture = HistoryFixtureRepository::new();
    let output = fixture.run(&["history", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid history JSON");

    assert!(
        output.status.success(),
        "history failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_plain_report(&stdout(&output));
    assert_eq!(value["status"], "analyzed");
    assert_eq!(value["history"]["commits_seen"], 6);
    assert_eq!(value["history"]["non_merge_commits_seen"], 5);
    assert_eq!(value["history"]["settings"]["window_days"], 365);
    assert_eq!(value["history"]["settings"]["recent_window_days"], 180);

    let churn_paths: BTreeMap<_, _> = value["history"]["churn"]["paths"]
        .as_array()
        .expect("churn paths")
        .iter()
        .map(|path| {
            (
                path["path"].as_str().expect("path name").to_owned(),
                path["commits"].as_u64().expect("count"),
            )
        })
        .collect();
    assert_eq!(churn_paths.get("src/lib.rs"), Some(&3));
    assert_eq!(churn_paths.get("src/side.rs"), Some(&1));
    assert_eq!(churn_paths.get("src"), None);
    assert!(
        value["history"]["bugs"]["overlap_paths"]
            .as_array()
            .expect("bug overlap")
            .iter()
            .any(|path| path["path"] == "src/lib.rs")
    );
    assert_eq!(
        value["history"]["bugs"]["commits"]
            .as_array()
            .expect("bug evidence")
            .len(),
        1
    );
    assert_eq!(
        value["history"]["firefighting"]["commits"]
            .as_array()
            .expect("firefighting evidence")
            .len(),
        2
    );
    assert_eq!(
        value["history"]["activity"]["months"]
            .as_array()
            .expect("activity months")
            .iter()
            .map(|month| month["commits"].as_u64().unwrap())
            .sum::<u64>(),
        6
    );

    let caveats = value["history"]["bugs"]["caveats"].as_array().expect("bug caveats");
    assert!(
        caveats
            .iter()
            .any(|caveat| caveat.as_str().expect("caveat").contains("commit-message discipline"))
    );
    let contributor_caveats = value["history"]["contributors"]["caveats"]
        .as_array()
        .expect("contributor caveats");
    assert!(
        contributor_caveats
            .iter()
            .any(|caveat| caveat.as_str().expect("caveat").contains("Squash merges"))
    );
}

#[test]
fn focused_history_commands_support_scopes_and_explicit_overrides() {
    let fixture = HistoryFixtureRepository::new();
    let bugs = fixture.run(&[
        "history",
        "bugs",
        "--window-days",
        "30",
        "--bug-keyword",
        "parser",
        "--json",
    ]);
    let bugs_json: Value = serde_json::from_str(&stdout(&bugs)).expect("valid focused bug JSON");

    assert!(bugs.status.success());
    assert_eq!(bugs_json["history"]["settings"]["window_days"], 30);
    assert_eq!(bugs_json["history"]["settings"]["bug_keywords"][0], "parser");
    assert_eq!(bugs_json["history"]["bugs"]["keywords"][0], "parser");
    assert_eq!(bugs_json["history"]["bugs"]["paths"][0]["path"], "src/lib.rs");
    assert_eq!(bugs_json["history"]["bugs"]["overlap_paths"][0]["path"], "src/lib.rs");
    assert!(bugs_json["history"]["churn"].is_null());

    let keyword_miss = fixture.run(&["history", "bugs", "--bug-keyword", "not-a-keyword", "--json"]);
    let keyword_miss_json: Value = serde_json::from_str(&stdout(&keyword_miss)).expect("valid keyword-miss JSON");
    assert!(keyword_miss.status.success());
    assert!(
        keyword_miss_json["history"]["bugs"]["commits"]
            .as_array()
            .expect("keyword-miss commits")
            .is_empty()
    );
    assert!(
        keyword_miss_json["history"]["bugs"]["caveats"]
            .as_array()
            .expect("keyword-miss caveats")
            .iter()
            .any(|caveat| caveat
                .as_str()
                .expect("caveat")
                .contains("No bug-related commits matched"))
    );

    let scoped = fixture.run(&["history", "churn", "src", "--json"]);
    let scoped_json: Value = serde_json::from_str(&stdout(&scoped)).expect("valid scoped churn JSON");
    assert!(scoped.status.success());
    assert_eq!(scoped_json["history"]["scope_path"], "src");
    assert!(
        scoped_json["history"]["churn"]["paths"]
            .as_array()
            .expect("scoped paths")
            .iter()
            .all(|path| path["path"].as_str().expect("path").starts_with("src/"))
    );

    let activity = fixture.run(&["history", "activity"]);
    let activity_markdown = stdout(&activity);
    assert!(activity.status.success());
    assert!(activity_markdown.contains("Monthly activity"));
    assert!(!activity_markdown.contains("Churn hotspots"));
}

#[test]
fn scoped_history_activity_and_envelope_counts_only_include_affected_commits() {
    let fixture = HistoryFixtureRepository::new();
    let output = fixture.run(&["history", "activity", "src", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid scoped activity JSON");

    assert!(output.status.success());
    assert_eq!(value["history"]["scope_path"], "src");
    assert_eq!(value["history"]["commits_seen"], 4);
    assert_eq!(value["history"]["non_merge_commits_seen"], 4);
    assert_eq!(
        value["history"]["activity"]["months"]
            .as_array()
            .expect("scoped activity months")
            .iter()
            .map(|month| month["commits"].as_u64().expect("monthly commit count"))
            .sum::<u64>(),
        4
    );
}

#[test]
fn contributors_apply_committed_mailmap_and_hide_emails_by_default() {
    let fixture = HistoryFixtureRepository::new();
    let compact = fixture.run(&["history", "contributors", "--json"]);
    let compact: Value = serde_json::from_str(&stdout(&compact)).expect("valid compact contributor JSON");
    let contributors = &compact["history"]["contributors"];

    assert_eq!(contributors["mailmap_applied"], true);
    let bob = contributors["overall"]
        .as_array()
        .expect("overall contributors")
        .iter()
        .find(|contributor| contributor["name"] == "Bob")
        .expect("mailmap canonicalized Bob");
    assert_eq!(bob["commits"], 2);
    assert!(bob.get("email").is_none());
    let mapping = &contributors["identity_mappings"][0];
    assert_eq!(mapping["raw_name"], "Robert Alias");
    assert_eq!(mapping["canonical_name"], "Bob");
    assert!(mapping.get("raw_email").is_none());

    let disclosed = fixture.run(&["history", "contributors", "--include-emails", "--json"]);
    let disclosed: Value = serde_json::from_str(&stdout(&disclosed)).expect("valid disclosed contributor JSON");
    let mapping = &disclosed["history"]["contributors"]["identity_mappings"][0];
    assert_eq!(mapping["raw_email"], "ALIAS@example.com");
    assert_eq!(mapping["canonical_email"], "bob@example.com");
}

#[test]
fn history_keywords_are_word_aware_and_record_matches_with_substring_compatibility() {
    let fixture = HistoryFixtureRepository::new();
    let word = fixture.run(&["history", "bugs", "--json"]);
    let word: Value = serde_json::from_str(&stdout(&word)).expect("valid word-aware bug JSON");
    let commits = word["history"]["bugs"]["commits"]
        .as_array()
        .expect("word-aware commits");
    assert_eq!(word["history"]["bugs"]["keyword_match"], "word");
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0]["matched_terms"], serde_json::json!(["bug", "fix"]));
    assert!(
        commits
            .iter()
            .all(|commit| commit["subject"] != "Implement fixture prefix debug parser")
    );
    assert!(
        commits
            .iter()
            .all(|commit| commit["subject"] != "Emergency hotfix side work")
    );

    let substring = fixture.run(&["history", "bugs", "--keyword-match", "substring", "--json"]);
    let substring: Value = serde_json::from_str(&stdout(&substring)).expect("valid substring bug JSON");
    let commits = substring["history"]["bugs"]["commits"]
        .as_array()
        .expect("substring commits");
    assert_eq!(commits.len(), 3);
    assert!(commits.iter().any(|commit| {
        commit["subject"] == "Implement fixture prefix debug parser"
            && commit["matched_terms"] == serde_json::json!(["bug", "fix"])
    }));
}

#[test]
fn churn_reports_normalization_edge_cases_and_rename_unavailability() {
    let fixture = HistoryFixtureRepository::new();
    let output = fixture.run(&["history", "churn", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid churn JSON");
    let churn = &value["history"]["churn"];
    assert_eq!(churn["size_basis"], "current_head_blob_bytes");
    assert_eq!(churn["rename_continuity"]["status"], "unavailable");

    let paths = churn["paths"].as_array().expect("churn paths");
    let path = |name: &str| {
        paths
            .iter()
            .find(|path| path["path"] == name)
            .unwrap_or_else(|| panic!("missing churn path {name}"))
    };
    assert_eq!(path("src/lib.rs")["size_status"], "text");
    assert!(path("src/lib.rs")["commits_per_kib_milli"].is_number());
    assert_eq!(path("src/generated.rs")["size_status"], "generated");
    assert!(path("src/generated.rs")["commits_per_kib_milli"].is_number());
    assert_eq!(path("src/empty.rs")["size_status"], "empty");
    assert!(path("src/empty.rs").get("commits_per_kib_milli").is_none());
    assert_eq!(path("src/binary.rs")["size_status"], "binary");
    assert!(path("src/binary.rs").get("commits_per_kib_milli").is_none());
    assert_eq!(path("src/side.rs")["size_status"], "missing_at_head");
}

#[test]
fn every_history_operation_renders_in_markdown_and_json() {
    let fixture = HistoryFixtureRepository::new();
    for operation in ["history", "churn", "contributors", "bugs", "activity", "firefighting"] {
        let operation_arguments: Vec<&str> =
            if operation == "history" { vec!["history"] } else { vec!["history", operation] };
        let markdown = fixture.run(&operation_arguments);
        assert!(markdown.status.success(), "{operation} Markdown failed");
        assert!(markdown.stderr.is_empty());
        assert!(stdout(&markdown).contains("Status: Analyzed"));
        assert_plain_report(&stdout(&markdown));

        let mut json_arguments = operation_arguments;
        json_arguments.push("--json");
        let json = fixture.run(&json_arguments);
        assert!(json.status.success(), "{operation} JSON failed");
        assert!(json.stderr.is_empty());
        let value: Value = serde_json::from_str(&stdout(&json)).expect("valid operation JSON");
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["status"], "analyzed");
    }
}
