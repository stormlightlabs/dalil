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
