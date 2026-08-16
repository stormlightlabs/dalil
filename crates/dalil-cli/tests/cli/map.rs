use super::*;

#[test]
fn map_inventory_and_rust_findings_are_reported_semantically() {
    let fixture = MapFixtureRepository::new();
    let output = fixture.run(&["map", "--no-cache", "--json"]);
    let json = stdout(&output);
    let value: Value = serde_json::from_str(&json).expect("valid map JSON");

    assert!(
        output.status.success(),
        "map failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(value["reading_plan"].is_null());
    assert!(output.stderr.is_empty());
    assert_plain_report(&json);
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["status"], "analyzed");
    assert_eq!(value["command"]["name"], "map");
    assert_eq!(value["map"]["query_pack"], "rust-v1");
    assert_eq!(value["map"]["inventory"]["tracked"], 8);
    assert_eq!(value["map"]["inventory"]["modified"], 1);
    assert_eq!(value["map"]["inventory"]["untracked"], 1);
    assert_eq!(value["map"]["inventory"]["analyzed"], 7);
    assert_eq!(value["map"]["inventory"]["omitted"], 3);

    let files = value["map"]["files"].as_array().expect("map files");
    assert_eq!(
        files
            .iter()
            .find(|file| file["path"] == "src/lib.rs")
            .expect("modified Rust file")["worktree_state"],
        "modified"
    );
    assert_eq!(
        files
            .iter()
            .find(|file| file["path"] == "src/tracked_ignored.rs")
            .expect("tracked ignored Rust file")["worktree_state"],
        "tracked"
    );
    assert_eq!(
        files
            .iter()
            .find(|file| file["path"] == "src/untracked.rs")
            .expect("untracked Rust file")["worktree_state"],
        "untracked"
    );
    assert_eq!(
        files
            .iter()
            .find(|file| file["path"] == "src/broken.rs")
            .expect("malformed Rust file")["status"],
        "partial"
    );
    assert!(
        files
            .iter()
            .find(|file| file["path"] == "src/lib.rs")
            .expect("parsed Rust file")["symbols"]
            .as_array()
            .expect("symbols")
            .iter()
            .any(|symbol| symbol["name"] == "parse" && symbol["role"] == "definition")
    );

    let omissions = value["map"]["omissions"].as_array().expect("map omissions");
    assert!(
        omissions
            .iter()
            .any(|omission| { omission["path"] == "src/ignored.rs" && omission["reason"] == "ignored_untracked" })
    );
    assert!(
        omissions
            .iter()
            .any(|omission| { omission["path"] == "README.md" && omission["reason"] == "non_source" })
    );

    let findings = value["map"]["findings"].as_array().expect("map findings");
    assert!(findings.iter().any(|finding| finding["kind"] == "parse_error"));
    assert!(
        !findings
            .iter()
            .any(|finding| { finding["kind"] == "ambiguous_reference" && finding["path"] == "src/use.rs" })
    );
}

#[test]
fn compact_classification_excludes_low_value_source_without_poisoning_quality() {
    let fixture = ClassificationFixtureRepository::new();
    let output = fixture.run(&["map", "--no-cache", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid classification JSON");

    assert!(
        output.status.success(),
        "classification map failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(value["quality"]["partial"], false);
    assert_eq!(value["quality"]["unsupported"], false);
    assert_eq!(value["quality"]["resource_limited"], false);
    assert_eq!(value["quality"]["unsafe_paths"], false);
    assert!(
        value["map"]["classifications"]["total"].as_u64().unwrap() >= 7,
        "classifications: {}",
        value["map"]["classifications"]
    );
    assert!(value["map"]["classifications"]["generated"].as_u64().unwrap() >= 2);
    assert!(value["map"]["classifications"]["vendor"].as_u64().unwrap() >= 2);
    assert!(value["map"]["classifications"]["minified"].as_u64().unwrap() >= 2);
    assert!(value["map"]["classifications"]["source_map"].as_u64().unwrap() >= 1);
    assert!(
        value["map"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "src/generated_parser.rs")
    );
    assert!(
        !value["map"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "src/large.generated.rs")
    );
    assert!(
        value["map"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|omission| omission["reason"] == "classified")
    );
    assert!(value["map"]["inventory"]["omitted"].as_u64().unwrap() < 32);

    let repeat = fixture.run(&["map", "--no-cache", "--json"]);
    let repeat_value: Value = serde_json::from_str(&stdout(&repeat)).expect("valid repeated classification JSON");
    assert_eq!(value["map"]["classifications"], repeat_value["map"]["classifications"]);

    let markdown = fixture.run(&["map", "--no-cache"]);
    assert!(markdown.status.success());
    assert!(stdout(&markdown).contains("excluded before parsing"));
    assert!(stdout(&markdown).contains("bounded_minification_heuristic"));
}

#[test]
fn classification_overrides_include_safe_text_and_keep_hard_limits() {
    let fixture = ClassificationFixtureRepository::new();

    let focused = fixture.run(&["map", "--no-cache", "--focus-path", "src/vendor/tracked.rs", "--json"]);
    let focused_value: Value = serde_json::from_str(&stdout(&focused)).expect("valid focused classification JSON");
    assert!(focused.status.success());
    let focused_file = focused_value["map"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "src/vendor/tracked.rs")
        .expect("focused vendored file");
    assert_eq!(focused_file["classification_overridden"], true);
    assert_eq!(focused_file["classifications"][0]["kind"], "vendor");

    let evidence = fixture.run(&["map", "--profile", "evidence", "--no-cache", "--json"]);
    let evidence_value: Value = serde_json::from_str(&stdout(&evidence)).expect("valid evidence classification JSON");
    assert!(evidence.status.success());
    assert!(
        !evidence_value["map"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "src/generated_marker.rs" && file["classification_overridden"] == true)
    );
    assert!(
        !evidence_value["map"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "vendor/untracked.rs" && file["classification_overridden"] == true)
    );

    let focused_untracked = fixture.run(&[
        "map",
        "--profile",
        "evidence",
        "--no-cache",
        "--focus-path",
        "vendor/untracked.rs",
        "--json",
    ]);
    let focused_untracked_value: Value =
        serde_json::from_str(&stdout(&focused_untracked)).expect("valid focused untracked classification JSON");
    assert!(focused_untracked.status.success());
    assert!(
        focused_untracked_value["map"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "vendor/untracked.rs" && file["classification_overridden"] == true),
        "focused map: {}",
        focused_untracked_value["map"]
    );

    let hard_limit = fixture.run(&["map", "--no-cache", "--focus-path", "src/large.generated.rs", "--json"]);
    let hard_limit_value: Value = serde_json::from_str(&stdout(&hard_limit)).expect("valid hard-limit JSON");
    assert!(hard_limit.status.success());
    assert!(
        hard_limit_value["map"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|omission| {
                omission["path"] == "src/large.generated.rs"
                    && omission["reason"] == "oversized"
                    && omission["classification_overridden"] == true
            })
    );
}

#[test]
fn hidden_untracked_sources_are_included_but_hidden_ignored_sources_are_recorded() {
    let fixture = MapFixtureRepository::new();
    write_file(
        fixture.root.join(".gitignore"),
        b"src/ignored.rs\nsrc/.ignored-hidden.rs\n",
    );
    write_file(fixture.root.join("src/.hidden.rs"), b"pub fn hidden() {}\n");
    write_file(
        fixture.root.join("src/.ignored-hidden.rs"),
        b"pub fn ignored_hidden() {}\n",
    );

    let output = fixture.run(&["map", "--no-cache", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid hidden-file map JSON");

    assert!(output.status.success());
    assert!(
        value["map"]["files"]
            .as_array()
            .expect("map files")
            .iter()
            .any(|file| file["path"] == "src/.hidden.rs" && file["worktree_state"] == "untracked")
    );
    assert!(
        value["map"]["omissions"]
            .as_array()
            .expect("map omissions")
            .iter()
            .any(|omission| {
                omission["path"] == "src/.ignored-hidden.rs" && omission["reason"] == "ignored_untracked"
            })
    );
}

#[test]
fn ignored_directories_are_not_traversed_to_inventory_child_files() {
    let fixture = MapFixtureRepository::new();
    write_file(fixture.root.join(".gitignore"), b"src/ignored.rs\n.sandbox/\n");
    let ignored_path = ".sandbox/2026/07/14/2026_07_14_status_ticket_3_rust_map.md";
    fs::create_dir_all(fixture.root.join(".sandbox/2026/07/14")).expect("create ignored sandbox directory");
    write_file(
        fixture.root.join(ignored_path),
        b"# Status\n\nThis is Markdown, not Rust.\n",
    );

    let output = fixture.run(&["map", "--no-cache", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("valid map JSON");

    assert!(
        output.status.success(),
        "map failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        value["map"]["omissions"]
            .as_array()
            .expect("map omissions")
            .iter()
            .all(|omission| omission["path"] != ignored_path),
        "ignored directory child was inventoried: {}",
        value["map"]["omissions"]
    );
    assert!(
        value["map"]["omissions"]
            .as_array()
            .expect("map omissions")
            .iter()
            .any(|omission| {
                omission["path"] == "src/ignored.rs"
                    && omission["reason"] == "ignored_untracked"
                    && omission["detail"] == "The ignored untracked file was omitted by the ignore traversal policy."
            }),
        "ignored files in visible directories remain reported: {}",
        value["map"]["omissions"]
    );
}

#[test]
fn map_scope_exclusions_and_markdown_limitations_are_preserved() {
    let fixture = MapFixtureRepository::new();
    let json_output = fixture.run(&["map", "src", "--exclude", "src/two.rs", "--json"]);
    let json: Value = serde_json::from_str(&stdout(&json_output)).expect("valid scoped map JSON");

    assert!(json_output.status.success());
    assert!(json_output.stderr.is_empty());
    assert_eq!(json["scope"]["selected_path"], "src");
    assert_eq!(json["map"]["scope_path"], "src");
    assert_eq!(json["map"]["exclusions"][0], "src/two.rs");
    assert!(
        json["map"]["files"]
            .as_array()
            .expect("scoped files")
            .iter()
            .all(|file| file["path"].as_str().expect("file path").starts_with("src/"))
    );
    assert!(
        json["map"]["omissions"]
            .as_array()
            .expect("scoped omissions")
            .iter()
            .any(|omission| { omission["path"] == "src/two.rs" && omission["reason"] == "explicit_exclusion" })
    );

    let markdown_output = fixture.run(&["map", "src"]);
    let markdown = stdout(&markdown_output);
    assert!(markdown_output.status.success());
    assert!(markdown_output.stderr.is_empty());
    assert_plain_report(&markdown);
    assert!(markdown.contains("Map scope: `src`"));
    assert!(markdown.contains("Inventory:"));
    assert!(markdown.contains("Map findings"));
    assert!(markdown.contains("Map limitations"));
    assert!(markdown.contains("lexically"));
}

#[test]
fn map_rejects_unqualified_cross_file_edges_and_applies_focus_and_token_budget() {
    let fixture = MapFixtureRepository::new();
    let output = fixture.run(&[
        "map",
        "--focus",
        "duplicate",
        "--focus-path",
        "src/one.rs",
        "--budget",
        "40",
        "--no-cache",
        "--json",
    ]);
    let json: Value = serde_json::from_str(&stdout(&output)).expect("valid focused map JSON");

    assert!(
        output.status.success(),
        "focused map failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(json["map"]["selection"]["token_budget"], 40);
    assert!(json["map"]["selection"]["estimated_tokens"].as_u64().unwrap() <= 40);
    assert_eq!(json["map"]["ranking"][0]["path"], "src/one.rs");
    assert_eq!(json["map"]["cache"]["status"], "disabled");
    assert!(
        !json["map"]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| { edge["source"] == "src/use.rs" && edge["symbol"] == "duplicate" })
    );

    let elided = fixture.run(&["map", "--focus", "duplicate", "--budget", "14", "--no-cache", "--json"]);
    let elided_json: Value = serde_json::from_str(&stdout(&elided)).expect("valid elided map JSON");
    assert!(
        elided_json["map"]["selection"]["snippets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|snippet| snippet["truncated"] == true && snippet["symbol"]["location"]["start"]["line"] == 1)
    );
}
