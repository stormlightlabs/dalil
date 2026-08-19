use serde_json::Value;

use super::{MapFixtureRepository, stdout, write_file};

fn add_cycle(fixture: &MapFixtureRepository) {
    write_file(
        fixture.root.join("src/one.rs"),
        b"use crate::two::two;\npub fn one() { two(); }\n",
    );
    write_file(
        fixture.root.join("src/two.rs"),
        b"use crate::tracked_ignored::tracked;\npub fn two() { tracked(); }\n",
    );
    write_file(
        fixture.root.join("src/tracked_ignored.rs"),
        b"use crate::one::one;\npub fn tracked() { one(); }\n",
    );
}

#[test]
fn neighbors_are_depth_limited_deterministic_and_cycle_safe() {
    let fixture = MapFixtureRepository::new();
    add_cycle(&fixture);

    let arguments = [
        "traverse",
        "neighbors",
        "src/one.rs",
        ".",
        "--direction",
        "outgoing",
        "--kind",
        "import",
        "--depth",
        "8",
        "--no-cache",
        "--json",
    ];
    let first = fixture.run(&arguments);
    let second = fixture.run(&arguments);
    assert!(first.status.success(), "neighbor traversal failed: {:?}", first.stderr);
    assert!(
        second.status.success(),
        "repeated neighbor traversal failed: {:?}",
        second.stderr
    );
    assert_eq!(stdout(&first), stdout(&second));

    let value: Value = serde_json::from_str(&stdout(&first)).expect("neighbor JSON");
    let nodes = value["nodes"].as_array().expect("neighbor nodes");
    assert_eq!(nodes.len(), 2, "traversal output: {value}");
    assert_eq!(
        nodes.iter().filter(|node| node["node"]["path"] == "src/one.rs").count(),
        0
    );
    assert!(nodes.iter().all(|node| node["depth"].as_u64().unwrap_or_default() <= 2));
    assert_eq!(value["bounds"]["visited_nodes"], 3);
    assert_eq!(value["bounds"]["work_limited"], false);
}

#[test]
fn path_and_reverse_dependencies_follow_direction_and_bounds() {
    let fixture = MapFixtureRepository::new();
    add_cycle(&fixture);

    let path = fixture.run(&[
        "traverse",
        "path",
        "src/one.rs",
        "src/tracked_ignored.rs",
        ".",
        "--kind",
        "import",
        "--direction",
        "outgoing",
        "--depth",
        "2",
        "--no-cache",
        "--json",
    ]);
    assert!(path.status.success(), "path traversal failed: {:?}", path.stderr);
    let path_value: Value = serde_json::from_str(&stdout(&path)).expect("path JSON");
    let paths = path_value["paths"].as_array().expect("paths");
    assert_eq!(paths.len(), 1, "path output: {path_value}");
    assert_eq!(paths[0]["depth"], 2, "path output: {path_value}");
    assert_eq!(paths[0]["nodes"].as_array().expect("path nodes").len(), 3);
    assert_eq!(
        paths[0]["relationships"].as_array().expect("path relationships").len(),
        2
    );

    let reverse = fixture.run(&[
        "traverse",
        "reverse-dependencies",
        "src/tracked_ignored.rs",
        ".",
        "--depth",
        "2",
        "--no-cache",
        "--json",
    ]);
    assert!(
        reverse.status.success(),
        "reverse traversal failed: {:?}",
        reverse.stderr
    );
    let reverse_value: Value = serde_json::from_str(&stdout(&reverse)).expect("reverse dependency JSON");
    let reverse_nodes = reverse_value["nodes"].as_array().expect("reverse nodes");
    assert!(reverse_nodes.iter().any(|node| node["node"]["path"] == "src/two.rs"));
    assert!(reverse_nodes.iter().any(|node| node["node"]["path"] == "src/one.rs"));
    assert_eq!(reverse_value["request"]["direction"], "incoming");
}

#[test]
fn traversal_reports_work_and_depth_limits() {
    let fixture = MapFixtureRepository::new();
    add_cycle(&fixture);

    let work_limited = fixture.run(&[
        "traverse",
        "neighbors",
        "src/one.rs",
        ".",
        "--direction",
        "outgoing",
        "--kind",
        "import",
        "--depth",
        "8",
        "--work-limit",
        "1",
        "--no-cache",
        "--json",
    ]);
    assert!(work_limited.status.success());
    let work_value: Value = serde_json::from_str(&stdout(&work_limited)).expect("work-limited JSON");
    assert_eq!(work_value["bounds"]["work_limited"], true, "work output: {work_value}");
    assert!(
        work_value["omissions"]
            .as_array()
            .is_some_and(|items| { items.iter().any(|item| item["reason"] == "work_limit") })
    );

    let depth_limited = fixture.run(&[
        "traverse",
        "neighbors",
        "src/one.rs",
        ".",
        "--direction",
        "outgoing",
        "--kind",
        "import",
        "--depth",
        "1",
        "--no-cache",
        "--json",
    ]);
    assert!(depth_limited.status.success());
    let depth_value: Value = serde_json::from_str(&stdout(&depth_limited)).expect("depth-limited JSON");
    assert_eq!(depth_value["bounds"]["depth_limited"], true);
    assert_eq!(depth_value["nodes"].as_array().expect("depth nodes").len(), 1);
}
