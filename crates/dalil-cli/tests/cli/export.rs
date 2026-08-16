use std::fs;

use super::*;

#[test]
fn export_writes_a_shared_portable_snapshot_and_preserves_unknown_bundle_files() {
    let fixture = MapFixtureRepository::new();
    fs::create_dir(fixture.root.join(".dalil")).expect("create existing bundle directory");
    write_file(fixture.root.join(".dalil/notes.md"), b"keep this file\n");

    let first = fixture.run(&["export", "--no-cache", "--json"]);
    assert!(
        first.status.success(),
        "export failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let result: Value = serde_json::from_str(&stdout(&first)).expect("valid export result");
    let json_path = fixture.root.join(".dalil/map.json");
    let markdown_path = fixture.root.join(".dalil/map.md");
    assert!(
        !fixture.root.join(".dalil/tasks").exists(),
        "exports without an explicit task must not create task records"
    );
    let json = fs::read(&json_path).expect("read exported JSON");
    let map: Value = serde_json::from_slice(&json).expect("valid map JSON");
    let markdown = fs::read_to_string(&markdown_path).expect("read exported Markdown");

    let schema: Value =
        serde_json::from_str(include_str!("../../../../schema/export/v1/map.json")).expect("valid evidence-map schema");
    let required = schema["required"].as_array().expect("schema required fields");
    assert_eq!(map["schema_version"], schema["properties"]["schema_version"]["const"]);
    for field in required {
        assert!(
            map.get(field.as_str().expect("required field name")).is_some(),
            "missing {field}"
        );
    }
    assert_eq!(map["snapshot_id"], result["snapshot_id"]);
    assert!(markdown.contains(map["snapshot_id"].as_str().expect("snapshot id")));
    assert!(markdown.contains("## Symbols"));
    assert!(markdown.contains("## History"));
    assert!(map["projects"].is_array());
    assert!(map["files"].is_array());
    assert!(map["symbols"].is_array());
    assert_eq!(json.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert!(!map["symbols"].as_array().expect("top-level symbols").is_empty());
    assert!(
        map["files"]
            .as_array()
            .expect("files")
            .iter()
            .all(|file| file["symbols"].as_array().is_some_and(Vec::is_empty))
    );
    assert!(map["relationships"].is_array());
    assert!(map["landmarks"].is_array());
    assert!(map["tests"].is_array());
    assert!(map["history"].is_object());
    assert!(map["quality"].is_object());
    assert!(map["provenance"].is_object());
    assert!(map["collections"].is_object());
    assert_eq!(
        fs::read(fixture.root.join(".dalil/notes.md")).expect("unknown bundle file"),
        b"keep this file\n"
    );

    let second = fixture.run(&["export", "--no-cache", "--json"]);
    assert!(
        second.status.success(),
        "refresh failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let refreshed: Value =
        serde_json::from_slice(&fs::read(&json_path).expect("read refreshed JSON")).expect("valid map JSON");
    assert_eq!(map["snapshot_id"], refreshed["snapshot_id"]);
    assert_eq!(map["symbols"], refreshed["symbols"]);
    assert_eq!(map["relationships"], refreshed["relationships"]);

    write_file(fixture.root.join("README.md"), b"changed non-source evidence\n");
    let changed = fixture.run(&["export", "--no-cache", "--json"]);
    assert!(
        changed.status.success(),
        "changed export failed: {}",
        String::from_utf8_lossy(&changed.stderr)
    );
    let changed_map: Value =
        serde_json::from_slice(&fs::read(&json_path).expect("read changed JSON")).expect("valid changed map JSON");
    assert_ne!(refreshed["snapshot_id"], changed_map["snapshot_id"]);
    assert_ne!(refreshed["worktree_fingerprint"], changed_map["worktree_fingerprint"]);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(fixture.root.join(".dalil")).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(fs::metadata(json_path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(markdown_path).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn review_export_and_check_track_only_reviewable_facts() {
    let fixture = MapFixtureRepository::new();
    let review_path = fixture.root.join(".dalil/review.md");

    let missing = fixture.run(&["export", "--review", "--check", "--no-cache"]);
    assert_eq!(missing.status.code(), Some(5));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("dalil export --review"));
    assert!(
        !fixture.root.join(".dalil").exists(),
        "check mode must not create a bundle directory"
    );

    let written = fixture.run(&["export", "--review", "--no-cache", "--json"]);
    assert!(
        written.status.success(),
        "review export failed: {}",
        String::from_utf8_lossy(&written.stderr)
    );
    let result: Value = serde_json::from_str(&stdout(&written)).expect("valid review export result");
    assert_eq!(result["files"], serde_json::json!(["review.md"]));
    assert_eq!(result["status"], "written");
    assert!(review_path.exists());
    assert!(!fixture.root.join(".dalil/map.json").exists());
    assert!(!fixture.root.join(".dalil/map.md").exists());

    let first = fs::read_to_string(&review_path).expect("read review snapshot");
    assert!(first.contains("<!-- Generated by `dalil export --review`; do not edit. -->"));
    assert!(first.contains("public function `parse` in `src/lib.rs`"));
    assert!(!first.contains("Captured:"));
    assert!(!first.contains("Worktree fingerprint:"));

    let current = fixture.run(&["export", "--review", "--check", "--no-cache", "--json"]);
    assert!(
        current.status.success(),
        "review check failed: {}",
        String::from_utf8_lossy(&current.stderr)
    );
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&current)).expect("valid review check result")["status"],
        "current"
    );
    assert_eq!(
        fs::read_to_string(&review_path).expect("read unchanged review snapshot"),
        first
    );

    write_file(fixture.root.join(".gitignore"), b"src/ignored.rs\nignored/\n");
    fs::create_dir_all(fixture.root.join("ignored")).expect("create ignored directory");
    write_file(fixture.root.join("ignored/volatile.rs"), b"pub fn volatile() {}\n");
    let ignored_change = fixture.run(&["export", "--review", "--check", "--no-cache"]);
    assert!(
        ignored_change.status.success(),
        "ignored churn made review stale: {}",
        String::from_utf8_lossy(&ignored_change.stderr)
    );

    write_file(
        fixture.root.join("src/lib.rs"),
        b"pub fn parse() { let value = 2; let _ = value; }\nfn private_helper() {}\n",
    );
    let private_change = fixture.run(&["export", "--review", "--check", "--no-cache"]);
    assert!(
        private_change.status.success(),
        "private-only change made review stale: {}",
        String::from_utf8_lossy(&private_change.stderr)
    );
    assert_eq!(
        fs::read_to_string(&review_path).expect("read private-change review snapshot"),
        first
    );

    write_file(fixture.root.join("src/lib.rs"), b"pub fn parse_changed() {}\n");
    let stale = fixture.run(&["export", "--review", "--check", "--no-cache"]);
    assert_eq!(stale.status.code(), Some(5));
    assert_eq!(
        fs::read_to_string(&review_path).expect("check must not rewrite stale review snapshot"),
        first
    );

    let refreshed = fixture.run(&["export", "--review", "--no-cache"]);
    assert!(refreshed.status.success());
    let refreshed_review = fs::read_to_string(&review_path).expect("read refreshed review snapshot");
    assert!(refreshed_review.contains("parse_changed"));
    assert_ne!(refreshed_review, first);
}

#[test]
fn review_export_has_deterministic_overflow_notices_and_size_limits() {
    let fixture = MapFixtureRepository::new();
    for (name, start) in [("overflow_a.rs", 0usize), ("overflow_b.rs", 1_100usize)] {
        let source = (start..start + 1_100)
            .map(|index| format!("pub fn api_{index:04}() {{}}\n"))
            .collect::<String>();
        write_file(fixture.root.join("src").join(name), source.as_bytes());
    }

    let first = fixture.run(&["export", "--review", "--no-cache"]);
    assert!(
        first.status.success(),
        "overflow export failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let review_path = fixture.root.join(".dalil/review.md");
    let review = fs::read_to_string(&review_path).expect("read overflow review snapshot");
    assert!(
        review.lines().count() <= 2_000,
        "too many review lines: {}",
        review.lines().count()
    );
    assert!(
        review.len() <= 200 * 1024,
        "review exceeds byte limit: {}",
        review.len()
    );
    assert!(review.contains("Omitted "));
    assert!(review.contains("review snapshot limit"));

    let second = fixture.run(&["export", "--review", "--no-cache"]);
    assert!(second.status.success());
    assert_eq!(
        fs::read_to_string(&review_path).expect("read repeated review snapshot"),
        review
    );
}

#[test]
fn task_export_preserves_verbatim_input_and_appends_linked_orientations() {
    let fixture = MapFixtureRepository::new();
    write_file(fixture.root.join("src/needle.rs"), b"pub fn needle() {}\n");
    let task = "# Repair the needle\n\nCheck Unicode: \u{1f6a2}\n\n````rust\nneedle();\n````";

    let first = fixture.run(&["export", "--task", task, "--no-cache", "--json"]);
    assert!(
        first.status.success(),
        "task export failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_result: Value = serde_json::from_str(&stdout(&first)).expect("valid task export result");
    let first_filename = first_result["task"]["filename"].as_str().expect("task filename");
    assert!(first_filename.ends_with(".md"));
    assert!(first_filename.contains("repair-the-needle"));
    assert_eq!(first_result["files"][2], format!("tasks/{first_filename}"));

    let record_path = fixture.root.join(".dalil/tasks").join(first_filename);
    let record = fs::read_to_string(&record_path).expect("read task record");
    let map: Value = serde_json::from_slice(&fs::read(fixture.root.join(".dalil/map.json")).expect("read map"))
        .expect("valid map JSON");
    assert!(record.contains(task), "task text must round-trip exactly");
    assert!(
        record.contains("`````\n"),
        "outer fence must exceed the task's four-backtick fence"
    );
    assert!(record.contains(&format!(
        "- Map snapshot: `{}`",
        map["snapshot_id"].as_str().expect("snapshot ID")
    )));
    assert!(record.contains(&format!(
        "- Revision: `{}`",
        map["revision"]["oid"].as_str().expect("revision ID")
    )));
    assert!(record.contains(&format!(
        "- Worktree fingerprint: `{}`",
        map["worktree_fingerprint"].as_str().expect("worktree fingerprint")
    )));
    assert!(
        record.contains("src/needle.rs"),
        "orientation must use the task as a ranking seed"
    );
    assert!(record.contains("## Quality"));
    assert!(record.contains("## Limitations"));

    let repeated = fixture.run(&["export", "--task", task, "--no-cache", "--json"]);
    assert!(
        repeated.status.success(),
        "repeated task export failed: {}",
        String::from_utf8_lossy(&repeated.stderr)
    );
    let repeated_result: Value = serde_json::from_str(&stdout(&repeated)).expect("valid repeated task result");
    let repeated_filename = repeated_result["task"]["filename"]
        .as_str()
        .expect("repeated task filename");
    assert_ne!(
        first_filename, repeated_filename,
        "repeated tasks must append, never overwrite"
    );
    assert_eq!(first_result["task"]["id"], repeated_result["task"]["id"]);
    assert_eq!(
        fs::read_to_string(&record_path).expect("read preserved task record"),
        record
    );

    let empty = fixture.run(&["export", "--task", "", "--no-cache", "--json"]);
    assert!(
        empty.status.success(),
        "empty task export failed: {}",
        String::from_utf8_lossy(&empty.stderr)
    );
    let empty_result: Value = serde_json::from_str(&stdout(&empty)).expect("valid empty task result");
    assert!(
        empty_result["task"]["filename"]
            .as_str()
            .expect("empty task filename")
            .contains("-task-"),
        "an empty normalized slug must use the safe fallback"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(fixture.root.join(".dalil/tasks"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(fs::metadata(record_path).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn task_export_failure_leaves_no_partial_task_record() {
    let fixture = MapFixtureRepository::new();
    fs::create_dir(fixture.root.join(".dalil")).expect("create bundle directory");
    write_file(fixture.root.join(".dalil/tasks"), b"not a task directory\n");

    let output = fixture.run(&["export", "--task", "write a task record", "--no-cache"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("tasks"));
    assert_eq!(
        fs::read(fixture.root.join(".dalil/tasks")).expect("preserve collision"),
        b"not a task directory\n"
    );
    assert!(
        fixture.root.join(".dalil/map.json").exists(),
        "the completed map snapshot remains usable"
    );
    assert!(
        fixture.root.join(".dalil/map.md").exists(),
        "the completed map projection remains usable"
    );
    assert!(
        fs::read_dir(fixture.root.join(".dalil"))
            .expect("read bundle directory")
            .all(|entry| !entry
                .expect("bundle entry")
                .file_name()
                .to_string_lossy()
                .contains(".tmp-")),
        "failed publication must clean temporary files"
    );
}

#[cfg(unix)]
#[test]
fn export_refuses_a_symlink_bundle_directory() {
    let fixture = MapFixtureRepository::new();
    let outside = fixture.temporary_root.join("outside");
    fs::create_dir(&outside).expect("create outside directory");
    std::os::unix::fs::symlink(&outside, fixture.root.join(".dalil")).expect("create bundle symlink");

    let output = fixture.run(&["export", "--no-cache"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    assert!(
        fs::read_dir(&outside)
            .expect("outside remains readable")
            .next()
            .is_none()
    );
}
