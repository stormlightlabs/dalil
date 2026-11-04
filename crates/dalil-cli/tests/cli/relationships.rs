use serde_json::Value;

use super::{MapFixtureRepository, stdout, write_file};

#[test]
fn relationships_return_symbol_and_relationship_ids_with_call_evidence() {
    let fixture = MapFixtureRepository::new();
    std::fs::create_dir_all(fixture.root.join("tests")).expect("create test fixture directory");
    write_file(fixture.root.join("src/target.rs"), b"pub fn target() {}\n");
    write_file(
        fixture.root.join("src/caller.rs"),
        b"use crate::target::target as alias;\npub fn caller() { alias(); }\n",
    );
    write_file(
        fixture.root.join("tests/target_test.rs"),
        b"use crate::target::target;\n#[test]\nfn target_test() { target(); }\n",
    );

    let output = fixture.run(&["relationships", "callers", "target", ".", "--no-cache", "--json"]);
    assert!(
        output.status.success(),
        "relationship query failed: {:?}",
        output.stderr
    );
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_str(&stdout(&output)).expect("relationship JSON");
    let matches = value["matches"].as_array().expect("relationship matches");
    assert!(!matches.is_empty());
    assert!(matches.iter().all(|item| item["relation"] == "caller"));
    assert!(matches.iter().any(|item| item["node"]["symbol"]["name"] == "caller"));

    let relationships = value["relationships"].as_array().expect("relationship edges");
    assert_eq!(relationships.len(), matches.len());
    assert!(relationships.iter().all(|edge| edge["kind"] == "call"));
    assert!(relationships.iter().any(|edge| edge["symbol"] == "alias"));
    assert!(
        relationships
            .iter()
            .all(|edge| edge["id"].as_str().is_some_and(|id| id.starts_with("relationship:")))
    );
    assert!(
        matches
            .iter()
            .all(|item| item["node"]["id"].as_str().is_some_and(|id| id.starts_with("symbol:")))
    );
}

#[test]
fn relationship_operations_cover_definitions_references_imports_dependencies_and_tests() {
    let fixture = MapFixtureRepository::new();
    std::fs::create_dir_all(fixture.root.join("tests")).expect("create test fixture directory");
    write_file(fixture.root.join("src/target.rs"), b"pub fn target() {}\n");
    write_file(
        fixture.root.join("src/caller.rs"),
        b"use crate::target::target as alias;\npub fn caller() { alias(); }\n",
    );
    write_file(
        fixture.root.join("tests/target_test.rs"),
        b"use crate::target::target;\n#[test]\nfn target_test() { target(); }\n",
    );

    for (operation, target, expected_relation) in [
        ("definitions", "target", "definition"),
        ("references", "target", "reference"),
        ("imports", "src/caller.rs", "import"),
        ("dependencies", "src/caller.rs", "dependency"),
        ("reverse-dependencies", "src/target.rs", "reverse_dependency"),
        ("tests", "src/target.rs", "test"),
    ] {
        let output = fixture.run(&["relationships", operation, target, ".", "--no-cache", "--json"]);
        assert!(output.status.success(), "{operation} query failed: {:?}", output.stderr);
        assert!(output.stderr.is_empty());
        let value: Value = serde_json::from_str(&stdout(&output)).expect("relationship JSON");
        assert!(
            value["matches"]
                .as_array()
                .is_some_and(|items| { items.iter().any(|item| item["relation"] == expected_relation) })
        );
        assert!(value["bounds"]["total"].as_u64().unwrap_or_default() > 0);
    }

    let duplicate = fixture.run(&["relationships", "definitions", "duplicate", ".", "--no-cache", "--json"]);
    let duplicate_json: Value = serde_json::from_str(&stdout(&duplicate)).expect("duplicate definitions JSON");
    assert_eq!(duplicate_json["bounds"]["total"], 2);
}

#[test]
fn callers_do_not_borrow_an_owner_of_the_same_name() {
    let fixture = MapFixtureRepository::new();
    write_file(
        fixture.root.join("src/app.js"),
        b"function b() {}\nclass A {\n  run() {}\n}\nconst run = () => {\n  b();\n};\n",
    );

    let output = fixture.run(&["relationships", "callers", "b", ".", "--no-cache", "--json"]);
    assert!(output.status.success(), "callers query failed: {:?}", output.stderr);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("relationship JSON");
    let matches = value["matches"].as_array().expect("relationship matches");
    assert!(
        matches.iter().all(|item| item["node"]["symbol"]["kind"] != "method"),
        "the arrow `run` is not the method `A.run`: {matches:?}"
    );
}

#[test]
fn callers_are_found_past_output_and_symbol_limits_through_qualified_paths() {
    let fixture = MapFixtureRepository::new();
    for index in 0..40 {
        write_file(
            fixture.root.join(format!("src/filler_{index:02}.rs")),
            format!("pub fn filler_{index}() {{}}\n").as_bytes(),
        );
    }
    let mut registry = String::from("pub fn padding(value: usize) {\n");
    for _ in 0..200 {
        registry.push_str("    let _ = value;\n");
    }
    registry.push_str("}\npub fn late_target() { late_bare; }\n");
    write_file(fixture.root.join("src/registry.rs"), registry.as_bytes());
    // An unanchored `registry::` path resolves only to a module the crate declares.
    write_file(fixture.root.join("src/lib.rs"), b"pub fn parse() {}\nmod registry;\n");
    write_file(
        fixture.root.join("src/zz_caller.rs"),
        b"use super::*;\npub fn caller() { registry::late_target(); }\n",
    );

    let output = fixture.run(&["relationships", "callers", "late_target", ".", "--json"]);
    assert!(output.status.success(), "callers query failed: {:?}", output.stderr);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("relationship JSON");
    let matches = value["matches"].as_array().expect("relationship matches");
    assert!(
        matches.iter().any(|item| item["node"]["symbol"]["name"] == "caller"),
        "callers: {matches:?}"
    );

    // The default run above cached registry.rs under the 128-symbol limit; the
    // evidence profile must parse it again rather than reuse that record.
    let output = fixture.run(&[
        "relationships",
        "references",
        "late_bare",
        ".",
        "--profile",
        "evidence",
        "--json",
    ]);
    assert!(output.status.success(), "references query failed: {:?}", output.stderr);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("relationship JSON");
    assert_eq!(value["bounds"]["total"], 1, "references: {value}");
}

#[test]
fn human_output_leaves_out_method_limitations_that_json_keeps() {
    let fixture = MapFixtureRepository::new();
    let method = "Relationship targets are derived from retained Tree-sitter symbols";
    let note = "Method limitations that hold for every run are left out here";

    let human = stdout(&fixture.run(&["relationships", "callers", "parse", ".", "--no-cache"]));
    assert!(!human.contains(method), "human output: {human}");
    assert!(human.contains(note), "human output: {human}");

    let json = stdout(&fixture.run(&["relationships", "callers", "parse", ".", "--no-cache", "--json"]));
    assert!(json.contains(method));
    assert!(!json.contains(note));

    let explain = stdout(&fixture.run(&["explain", "parse", "--profile", "evidence", "--no-cache"]));
    assert!(explain.contains("Method limitations:"), "explain output: {explain}");
    assert!(explain.contains("references are extracted lexically"));
}

#[test]
fn compact_explain_keeps_method_limitations_inside_the_budget() {
    let fixture = MapFixtureRepository::new();
    let notice = "_Report truncated at the compact Markdown token budget; use `--json` for complete typed collections or `--profile evidence` for verbose Markdown._\n";
    let full = stdout(&fixture.run(&["explain", "parse", "--budget", "100000", "--no-cache"]));
    let section = &full[full.find("\nMethod limitations:").expect("method limitations section")..];

    for budget in [60_usize, 120, 200, 400] {
        let explain = stdout(&fixture.run(&["explain", "parse", "--budget", &budget.to_string(), "--no-cache"]));
        let characters = explain.chars().count();
        assert!(
            characters <= budget * 4,
            "budget {budget}: {characters} characters\n{explain}"
        );
        if characters < full.chars().count() {
            assert!(explain.ends_with(notice), "budget {budget}: {explain}");
        }
        // The section fits beside the notice and a separating newline.
        if section.chars().count() + notice.chars().count() + 3 < budget * 4 {
            assert!(explain.contains(section.trim_end()), "budget {budget}: {explain}");
        }
    }
}

#[test]
fn html_cards_and_notices_leave_out_method_limitations() {
    let fixture = MapFixtureRepository::new();
    let html = stdout(&fixture.run(&["map", "--html", "--no-cache"]));
    let visible = &html[..html.find("Complete report data").expect("raw report section")];

    assert!(!visible.contains("extracted lexically"), "{visible}");
    assert!(!visible.contains("Tree-sitter queries"), "{visible}");
    assert!(visible.contains("Method limitations that hold for every run are left out here"));
    assert!(
        html.contains("extracted lexically"),
        "the raw JSON keeps every limitation"
    );
}

#[test]
fn member_calls_on_unknown_receivers_reach_only_methods_at_low_confidence() {
    let fixture = MapFixtureRepository::new();
    write_file(
        fixture.root.join("src/members.rs"),
        b"fn insert() {}\nstruct T;\nimpl T {\n    fn insert(&self) {}\n    fn go(&self) { self.insert(); }\n}\nfn other(m: HashMap) { m.insert(); }\n",
    );

    let output = fixture.run(&[
        "relationships",
        "callers",
        "insert",
        ".",
        "--budget",
        "100000",
        "--no-cache",
        "--json",
    ]);
    assert!(output.status.success(), "callers query failed: {:?}", output.stderr);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("relationship JSON");
    let free = "symbol:src/members.rs:1:4:1:10:definition:function:insert";
    let method = "symbol:src/members.rs:4:8:4:14:definition:function:insert";
    let mut edges = value["relationships"]
        .as_array()
        .expect("relationship edges")
        .iter()
        .map(|edge| {
            let source = edge["source"].as_str().unwrap_or_default();
            let caller = source.rsplit(':').next().unwrap_or_default().to_owned();
            (
                caller,
                edge["target"].as_str().unwrap_or_default().to_owned(),
                edge["confidence"].as_str().unwrap_or_default().to_owned(),
                edge["resolution_reason"].as_str().unwrap_or_default().to_owned(),
                edge["ambiguous"].as_bool().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    edges.sort();
    let edge = |caller: &str, target: &str, confidence: &str, reason: &str, ambiguous: bool| {
        (
            caller.to_owned(),
            target.to_owned(),
            confidence.to_owned(),
            reason.to_owned(),
            ambiguous,
        )
    };
    // `self.insert()` keeps the same-file rule, which does not tell the two
    // `insert`s apart; `m.insert()` reaches only the method, at low confidence.
    assert_eq!(
        edges,
        [
            edge("go", free, "high", "same_file_explicit", false),
            edge("go", method, "high", "same_file_explicit", false),
            edge("other", method, "low", "member_receiver_unknown", true),
        ],
        "{value:#}"
    );
}

#[test]
fn callers_follow_ruby_constant_paths_inside_the_project_root() {
    let fixture = MapFixtureRepository::new();
    for directory in ["app", "lib/foo", "gems/gem/lib/foo"] {
        std::fs::create_dir_all(fixture.root.join(directory)).expect("create fixture directory");
    }
    write_file(fixture.root.join("Gemfile"), b"source 'https://rubygems.org'\n");
    write_file(fixture.root.join("app/caller.rb"), b"def run\n  Foo::Bar.baz\nend\n");
    write_file(
        fixture.root.join("lib/foo/bar.rb"),
        b"module Foo\n  class Bar\n    def self.baz; end\n  end\nend\n",
    );
    // A gem in the repository is its own project root, so its `foo/bar.rb` is not a target.
    write_file(fixture.root.join("gems/gem/gem.gemspec"), b"Gem::Specification.new\n");
    write_file(
        fixture.root.join("gems/gem/lib/foo/bar.rb"),
        b"module Foo\n  class Bar\n    def self.baz; end\n  end\nend\n",
    );

    let output = fixture.run(&["relationships", "callers", "baz", ".", "--no-cache", "--json"]);
    assert!(output.status.success(), "callers query failed: {:?}", output.stderr);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("relationship JSON");
    let matches = value["matches"].as_array().expect("relationship matches");
    assert_eq!(matches.len(), 1, "callers: {matches:?}");
    assert_eq!(matches[0]["node"]["symbol"]["name"], "run");
    let relationships = value["relationships"].as_array().expect("relationship edges");
    assert!(!relationships.is_empty());
    assert!(
        relationships
            .iter()
            .all(|edge| edge["resolution_reason"] == "qualified_path" && edge["confidence"] == "high"),
        "relationships: {relationships:?}"
    );
    let text = value.to_string();
    assert!(text.contains("lib/foo/bar.rb"), "{text}");
    assert!(!text.contains("gems/gem/lib/foo/bar.rb"), "{text}");
}
