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
