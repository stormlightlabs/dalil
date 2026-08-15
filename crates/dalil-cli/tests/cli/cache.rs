use super::*;

#[test]
fn map_cache_modes_hit_invalidate_refresh_and_disable_without_project_writes() {
    let fixture = MapFixtureRepository::new();
    let initial_cache_entries = fs::read_dir(&fixture.cache).expect("read empty cache root").count();
    assert_eq!(initial_cache_entries, 0);

    let disabled = fixture.run(&["map", "--no-cache", "--json"]);
    assert!(disabled.status.success());
    assert_eq!(
        fs::read_dir(&fixture.cache).expect("read disabled cache root").count(),
        0
    );

    let first = fixture.run(&["map", "--json"]);
    let first_json: Value = serde_json::from_str(&stdout(&first)).expect("first cached map JSON");
    assert_eq!(first_json["map"]["cache"]["status"], "refreshed");
    assert_eq!(first_json["map"]["cache"]["refreshed"].as_array().unwrap().len(), 7);

    let second = fixture.run(&["map", "--json"]);
    let second_json: Value = serde_json::from_str(&stdout(&second)).expect("cache-hit map JSON");
    assert_eq!(second_json["map"]["cache"]["status"], "hit");
    assert_eq!(second_json["map"]["cache"]["hits"], 7);
    assert_eq!(first_json["map"]["ranking"], second_json["map"]["ranking"]);
    assert_eq!(first_json["map"]["selection"], second_json["map"]["selection"]);

    write_file(
        fixture.root.join("src/lib.rs"),
        b"pub fn refreshed() { let changed = 3; let _ = changed; }\n",
    );
    let auto = fixture.run(&["map", "--json"]);
    let auto_json: Value = serde_json::from_str(&stdout(&auto)).expect("auto-refresh map JSON");
    assert_eq!(auto_json["map"]["cache"]["status"], "refreshed");
    assert_eq!(
        auto_json["map"]["cache"]["refreshed"],
        serde_json::json!(["src/lib.rs"])
    );
    let auto_lib = auto_json["map"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "src/lib.rs")
        .expect("refreshed Rust file");
    assert!(
        auto_lib["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .any(|symbol| { symbol["name"] == "refreshed" && symbol["role"] == "definition" })
    );

    let always = fixture.run(&["map", "--cache", "always", "--json"]);
    let always_json: Value = serde_json::from_str(&stdout(&always)).expect("always-refresh map JSON");
    assert_eq!(always_json["map"]["cache"]["status"], "refreshed");
    assert_eq!(always_json["map"]["cache"]["refreshed"].as_array().unwrap().len(), 7);

    write_file(
        fixture.root.join("src/lib.rs"),
        b"pub fn parse() { let changed = 2; let _ = changed; }\n",
    );
    let manual = fixture.run(&["map", "--cache", "manual", "--json"]);
    let manual_json: Value = serde_json::from_str(&stdout(&manual)).expect("manual stale map JSON");
    assert_eq!(manual_json["map"]["cache"]["status"], "stale");
    assert!(
        manual_json["map"]["cache"]["stale"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == "src/lib.rs")
    );
    assert!(
        manual_json["map"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["path"] == "src/lib.rs")
            .unwrap()["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|limitation| limitation.as_str().unwrap().contains("potentially stale"))
    );

    let files = fixture.run(&["map", "--cache", "files", "--cache-file", "src/lib.rs", "--json"]);
    let files_json: Value = serde_json::from_str(&stdout(&files)).expect("file-refresh map JSON");
    assert_eq!(files_json["map"]["cache"]["status"], "refreshed");
    assert_eq!(
        files_json["map"]["cache"]["refreshed"],
        serde_json::json!(["src/lib.rs"])
    );

    let missing_changed_file = fixture.run(&["map", "--cache", "files", "--json"]);
    assert_eq!(missing_changed_file.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing_changed_file.stderr).contains("requires at least one"));
}

#[test]
fn incremental_index_reuses_unchanged_work_and_matches_cold_analysis() {
    let fixture = MapFixtureRepository::new();
    write_file(fixture.root.join("src/target.rs"), b"pub fn target() {}\n");
    write_file(
        fixture.root.join("src/use.rs"),
        b"use crate::target::target;\nfn use_it() { target(); }\n",
    );
    let initial = fixture.run(&["map", "--json"]);
    assert!(initial.status.success());

    let warm = fixture.run(&["map", "--json"]);
    assert!(warm.status.success());
    let warm_json: Value = serde_json::from_slice(&warm.stdout).expect("valid warm map JSON");
    assert_eq!(warm_json["map"]["cache"]["index_status"], "hit");
    assert_eq!(warm_json["map"]["cache"]["reused"].as_array().unwrap().len(), 8);
    assert!(warm_json["map"]["cache"]["invalidated"].as_array().unwrap().is_empty());

    write_file(
        fixture.root.join("src/target.rs"),
        b"pub fn target(value: usize) { let _ = value; }\n",
    );
    let refreshed = fixture.run(&["map", "--json"]);
    assert!(refreshed.status.success());
    let mut refreshed_json: Value = serde_json::from_slice(&refreshed.stdout).expect("valid refreshed map JSON");
    assert_eq!(refreshed_json["map"]["cache"]["index_status"], "refreshed");
    assert_eq!(
        refreshed_json["map"]["cache"]["invalidated"],
        serde_json::json!(["src/target.rs"])
    );
    assert_eq!(refreshed_json["map"]["cache"]["reused"].as_array().unwrap().len(), 7);

    let cold = fixture.run(&["map", "--no-cache", "--json"]);
    assert!(cold.status.success());
    let mut cold_json: Value = serde_json::from_slice(&cold.stdout).expect("valid cold map JSON");
    refreshed_json["map"].as_object_mut().unwrap().remove("cache");
    cold_json["map"].as_object_mut().unwrap().remove("cache");
    assert_eq!(refreshed_json["map"], cold_json["map"]);

    write_file(fixture.root.join("Cargo.toml"), b"[package]\nname = \"fixture\"\n");
    let manifest = fixture.run(&["map", "--json"]);
    assert!(manifest.status.success());
    let manifest_json: Value = serde_json::from_slice(&manifest.stdout).expect("valid manifest map JSON");
    assert_eq!(manifest_json["map"]["cache"]["index_status"], "refreshed");
    assert!(
        manifest_json["map"]["cache"]["index_detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("manifest content changed"))
    );
}

#[test]
fn files_cache_mode_refreshes_only_exact_requested_paths_and_reports_unavailable_files() {
    let fixture = MapFixtureRepository::new();
    let output = fixture.run(&[
        "map",
        "--cache",
        "files",
        "--cache-file",
        "src/lib.rs",
        "--cache-file",
        "lib.rs",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "files cache failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid files cache JSON");
    let cache = &json["map"]["cache"];
    assert_eq!(cache["matched"], 1);
    assert_eq!(cache["unmatched"], 1);
    assert_eq!(cache["unavailable"], 6);
    assert_eq!(cache["hits"], 0);
    assert_eq!(cache["misses"], 6);
    assert_eq!(cache["refreshed"], serde_json::json!(["src/lib.rs"]));
    assert_eq!(json["map"]["files"].as_array().unwrap().len(), 1);
    assert_eq!(json["map"]["files"][0]["path"], "src/lib.rs");
    assert!(
        json["map"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|omission| { omission["reason"] == "cache_unavailable" && omission["path"] == "src/one.rs" })
    );
}

#[test]
fn files_cache_mode_does_not_match_duplicate_basenames() {
    let fixture = MapFixtureRepository::new();
    fs::create_dir_all(fixture.root.join("src/a")).expect("create first duplicate-basename directory");
    fs::create_dir_all(fixture.root.join("src/b")).expect("create second duplicate-basename directory");
    write_file(fixture.root.join("src/a/shared.rs"), b"pub fn first_shared() {}\n");
    write_file(fixture.root.join("src/b/shared.rs"), b"pub fn second_shared() {}\n");

    let output = fixture.run(&[
        "map",
        "--cache",
        "files",
        "--cache-file",
        "src/a/shared.rs",
        "--cache-file",
        "shared.rs",
        "--json",
    ]);
    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid duplicate-basename cache JSON");
    assert_eq!(json["map"]["cache"]["matched"], 1);
    assert_eq!(json["map"]["cache"]["unmatched"], 1);
    assert_eq!(
        json["map"]["cache"]["refreshed"],
        serde_json::json!(["src/a/shared.rs"])
    );
    assert!(json["map"]["files"].as_array().unwrap().iter().any(|file| {
        file["path"] == "src/a/shared.rs"
            && file["symbols"]
                .as_array()
                .unwrap()
                .iter()
                .any(|symbol| symbol["name"] == "first_shared")
    }));
    assert!(
        !json["map"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| { file["path"] == "src/b/shared.rs" })
    );
}

#[test]
fn cache_records_are_reused_across_report_scopes_and_manual_uses_the_newest_record() {
    let fixture = MapFixtureRepository::new();
    let initial = fixture.run(&["map", "--json"]);
    assert!(initial.status.success());

    let scoped = fixture.run(&["map", "src", "--json"]);
    assert!(
        scoped.status.success(),
        "scoped map failed: {}",
        String::from_utf8_lossy(&scoped.stderr)
    );
    let scoped_json: Value = serde_json::from_slice(&scoped.stdout).expect("valid scoped map JSON");
    assert_eq!(scoped_json["map"]["scope_path"], "src");
    assert_eq!(scoped_json["map"]["cache"]["status"], "hit");
    assert_eq!(scoped_json["map"]["cache"]["hits"], 7);

    write_file(fixture.root.join("src/lib.rs"), b"pub fn newest_cached() {}\n");
    std::thread::sleep(std::time::Duration::from_millis(2));
    let refreshed = fixture.run(&["map", "--json"]);
    assert!(refreshed.status.success());
    write_file(fixture.root.join("src/lib.rs"), b"pub fn current_not_cached() {}\n");
    let manual = fixture.run(&["map", "--cache", "manual", "--json"]);
    assert!(
        manual.status.success(),
        "manual map failed: {}",
        String::from_utf8_lossy(&manual.stderr)
    );
    let manual_json: Value = serde_json::from_slice(&manual.stdout).expect("valid manual map JSON");
    let lib = manual_json["map"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "src/lib.rs")
        .expect("manual cached lib file");
    assert!(
        lib["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .any(|symbol| { symbol["name"] == "newest_cached" && symbol["role"] == "definition" })
    );
    assert!(
        !lib["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .any(|symbol| { symbol["name"] == "current_not_cached" })
    );
    assert_eq!(manual_json["map"]["cache"]["status"], "stale");
}

#[test]
fn corrupt_cache_record_refreshes_and_cache_controls_do_not_touch_the_repository() {
    let fixture = MapFixtureRepository::new();
    let initial = fixture.run(&["map", "--json"]);
    assert!(initial.status.success());
    let records = cache_json_files(&fixture.cache.join("dalil"));
    assert_eq!(records.len(), 7);
    write_file(&records[0], b"not valid JSON\n");

    let refreshed = fixture.run(&["map", "--json"]);
    assert!(
        refreshed.status.success(),
        "refresh failed: {}",
        String::from_utf8_lossy(&refreshed.stderr)
    );
    let refreshed_json: Value = serde_json::from_slice(&refreshed.stdout).expect("valid refresh JSON");
    assert_eq!(refreshed_json["map"]["cache"]["status"], "refreshed");
    assert_eq!(refreshed_json["map"]["cache"]["misses"], 1);
    assert_eq!(refreshed_json["map"]["cache"]["refreshed"].as_array().unwrap().len(), 1);

    let source_before = fs::read(fixture.root.join("src/lib.rs")).expect("read source before cache control");
    let status = fixture.run(&["cache", "status", "--json"]);
    assert!(
        status.status.success(),
        "cache status failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status_json: Value = serde_json::from_slice(&status.stdout).expect("valid cache status JSON");
    assert_eq!(status_json["records"], 8);
    assert_eq!(status_json["repositories"], 1);
    assert!(status_json["path"].as_str().unwrap().ends_with("dalil"));

    let path = fixture.run(&["cache", "path", "--json"]);
    assert!(path.status.success());
    let path_json: Value = serde_json::from_slice(&path.stdout).expect("valid cache path JSON");
    assert_eq!(path_json["operation"], "path");

    let outside_cache_file = fixture.temporary_root.join("outside-cache.json");
    write_file(&outside_cache_file, b"must remain outside the cache root\n");
    let prune = fixture.run(&["cache", "prune", "--json"]);
    assert!(
        prune.status.success(),
        "cache prune failed: {}",
        String::from_utf8_lossy(&prune.stderr)
    );
    assert!(
        outside_cache_file.exists(),
        "cache prune crossed the configured cache root"
    );

    let clear = fixture.run(&["cache", "clear", "--json"]);
    assert!(
        clear.status.success(),
        "cache clear failed: {}",
        String::from_utf8_lossy(&clear.stderr)
    );
    let clear_json: Value = serde_json::from_slice(&clear.stdout).expect("valid cache clear JSON");
    assert_eq!(clear_json["removed_records"], 8);
    assert_eq!(clear_json["records"], 0);
    assert_eq!(
        fs::read(fixture.root.join("src/lib.rs")).expect("read source after cache control"),
        source_before
    );
}

#[cfg(unix)]
#[test]
fn cache_directories_and_records_are_user_private() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = MapFixtureRepository::new();
    let output = fixture.run(&["map", "--json"]);
    assert!(output.status.success());
    let cache_root = fixture.cache.join("dalil");
    assert_eq!(fs::metadata(&cache_root).unwrap().permissions().mode() & 0o777, 0o700);
    for record in cache_json_files(&cache_root) {
        assert_eq!(fs::metadata(record).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

#[test]
fn concurrent_cache_writers_leave_only_complete_json_records() {
    let fixture = MapFixtureRepository::new();
    let children = (0..4)
        .map(|_| {
            fixture
                .command(&["map", "--cache", "always", "--json"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn concurrent cache writer")
        })
        .collect::<Vec<_>>();
    for child in children {
        let output = child.wait_with_output().expect("wait for concurrent cache writer");
        assert!(
            output.status.success(),
            "concurrent cache writer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "concurrent map output is invalid JSON ({error}); stdout={} bytes, stderr={}",
                output.stdout.len(),
                String::from_utf8_lossy(&output.stderr)
            )
        });
    }

    let records = cache_json_files(&fixture.cache.join("dalil"));
    assert_eq!(records.len(), 7);
    for record in records {
        let bytes = fs::read(record).expect("read concurrent cache record");
        serde_json::from_slice::<Value>(&bytes).expect("concurrent cache record is complete JSON");
    }
}

#[cfg(unix)]
#[test]
fn hostile_worktree_symlink_is_omitted_without_reading_or_caching_target_content() {
    use std::os::unix::fs::symlink;

    let fixture = MapFixtureRepository::new();
    let outside = fixture.temporary_root.join("outside.rs");
    write_file(&outside, b"pub fn outside_secret() {}\n");
    symlink(&outside, fixture.root.join("src/outside.rs")).expect("create hostile source symlink");

    let output = fixture.run(&["map", "--no-cache", "--json"]);
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid hostile-worktree JSON");
    assert!(
        output.status.success(),
        "hostile map failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!stdout(&output).contains("outside_secret"));
    assert!(
        json["map"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|file| file["path"] != "src/outside.rs")
    );
    assert!(
        json["map"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|omission| omission["path"] == "src/outside.rs" && omission["reason"] == "symlink")
    );
    assert_eq!(
        fs::read_dir(&fixture.cache).unwrap().count(),
        0,
        "no-cache must not write cache data"
    );
}

#[cfg(unix)]
#[test]
fn worktree_swap_race_never_emits_content_from_a_replaced_directory() {
    use std::os::unix::fs::symlink;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::thread;

    let fixture = MapFixtureRepository::new();
    let outside = fixture.temporary_root.join("race-outside");
    fs::create_dir_all(&outside).expect("create race target directory");
    write_file(outside.join("race.rs"), b"pub fn race_outside_secret() {}\n");
    write_file(fixture.root.join("src/race.rs"), b"pub fn race_inside() {}\n");

    let running = Arc::new(AtomicBool::new(true));
    let attacker_running = Arc::clone(&running);
    let source = fixture.root.join("src");
    let moved = fixture.root.join("src-real");
    let link_target = outside.clone();
    let attacker = thread::spawn(move || {
        while attacker_running.load(Ordering::Acquire) {
            if fs::rename(&source, &moved).is_ok() {
                if symlink(&link_target, &source).is_ok() {
                    thread::yield_now();
                    let _ = fs::remove_file(&source);
                }
                let _ = fs::rename(&moved, &source);
            }
        }
    });

    let output = fixture.run(&["map", "--no-cache", "--json"]);
    running.store(false, Ordering::Release);
    attacker.join().expect("join worktree swap fixture");

    assert!(
        output.status.success(),
        "swap-race map failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("race_outside_secret"));
}

#[test]
fn malformed_tree_path_is_rejected_before_source_read_or_cache_write() {
    let fixture = MapFixtureRepository::new();
    let repository = gix::open(&fixture.root).expect("open malformed-tree fixture repository");
    let blob = repository
        .write_object(gix::objs::Blob { data: b"pub fn outside() {}\n".to_vec() })
        .expect("write malformed-tree blob")
        .detach();
    let tree = repository
        .write_object(gix::objs::Tree {
            entries: vec![gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: "../outside.rs".into(),
                oid: blob,
            }],
        })
        .expect("write malformed tree")
        .detach();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is after the Unix epoch")
        .as_secs() as i64;
    let commit = write_commit(
        &repository,
        tree,
        &[],
        "Malformed Tree Fixture",
        "malformed@example.com",
        now,
        "Malformed path fixture",
    );
    drop(repository);
    write_file(fixture.root.join(".git/HEAD"), b"ref: refs/heads/main\n");
    write_file(
        fixture.root.join(".git/refs/heads/main"),
        format!("{commit}\n").as_bytes(),
    );

    let output = fixture.run(&["map", "--json"]);
    assert_eq!(output.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&output.stderr).contains("safety"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("outside"));
    assert_eq!(fs::read_dir(&fixture.cache).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn cache_root_symlink_into_repository_is_rejected_before_any_write() {
    use std::os::unix::fs::symlink;

    let fixture = MapFixtureRepository::new();
    let cache_target = fixture.root.join("cache-target");
    fs::create_dir_all(&cache_target).expect("create cache target");
    let cache_link = fixture.root.join("cache-link");
    symlink(&cache_target, &cache_link).expect("create cache-root symlink");

    let output = fixture
        .command(&["map", "--json"])
        .env("XDG_CACHE_HOME", &cache_link)
        .output()
        .expect("run cache containment fixture");
    assert_eq!(output.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cache root"));
    assert_eq!(
        fs::read_dir(&cache_target).unwrap().count(),
        0,
        "cache writes must not cross a symlink"
    );
}

#[cfg(unix)]
#[test]
fn repository_filter_configuration_and_attributes_never_execute_a_sentinel() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = MapFixtureRepository::new();
    let marker = fixture.temporary_root.join("filter-ran");
    let sentinel = fixture.temporary_root.join("filter-sentinel.sh");
    write_file(
        &sentinel,
        format!("#!/bin/sh\nprintf ran >> '{}'\n", marker.display()).as_bytes(),
    );
    let mut permissions = fs::metadata(&sentinel).expect("sentinel metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&sentinel, permissions).expect("make sentinel executable");
    write_file(fixture.root.join(".gitattributes"), b"src/*.rs filter=hostile\n");
    write_file(
        fixture.root.join(".git/config"),
        format!(
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n[filter \"hostile\"]\n\tprocess = {}\n\tclean = {}\n\tsmudge = {}\n",
            sentinel.display(),
            sentinel.display(),
            sentinel.display()
        )
        .as_bytes(),
    );

    let output = fixture.run(&["map", "--json"]);
    assert!(
        output.status.success(),
        "filter fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "repository-controlled filter sentinel executed");
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn non_utf8_worktree_paths_are_typed_omissions_and_never_become_lossy_output() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let fixture = MapFixtureRepository::new();
    let invalid_name = OsString::from_vec(b"bad\xff.rs".to_vec());
    write_file(
        fixture.root.join("src").join(invalid_name),
        b"pub fn hidden_outside() {}\n",
    );

    let output = fixture.run(&["map", "--no-cache", "--json"]);
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid non-UTF-8 path JSON");
    assert!(
        output.status.success(),
        "non-UTF-8 fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!stdout(&output).contains("hidden_outside"));
    assert!(
        json["map"]["omissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|omission| omission["reason"] == "unsafe_path")
    );
}
