//! Replays a small generated repository end to end with a debug Dalil build.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn write_tree(repository: &gix::Repository, files: &[(String, String)]) -> gix::ObjectId {
    #[derive(Default)]
    struct Node {
        files: Vec<(String, gix::ObjectId)>,
        directories: BTreeMap<String, Node>,
    }

    fn write_node(repository: &gix::Repository, node: Node) -> gix::ObjectId {
        let mut entries = node
            .files
            .into_iter()
            .map(|(filename, oid)| gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: filename.into(),
                oid,
            })
            .collect::<Vec<_>>();
        for (filename, child) in node.directories {
            entries.push(gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Tree.into(),
                filename: filename.into(),
                oid: write_node(repository, child),
            });
        }
        entries.sort();
        repository
            .write_object(gix::objs::Tree { entries })
            .expect("write tree")
            .detach()
    }

    let mut root = Node::default();
    for (path, contents) in files {
        let blob = repository
            .write_object(gix::objs::Blob { data: contents.as_bytes().to_vec() })
            .expect("write blob")
            .detach();
        let mut node = &mut root;
        let mut components = path.split('/').collect::<Vec<_>>();
        let filename = components.pop().expect("non-empty path");
        for component in components {
            node = node.directories.entry(component.to_owned()).or_default();
        }
        node.files.push((filename.to_owned(), blob));
    }
    write_node(repository, root)
}

fn commit(
    repository: &gix::Repository, files: &[(String, String)], parents: &[gix::ObjectId], seconds: i64, message: &str,
) -> gix::ObjectId {
    let tree = write_tree(repository, files);
    let time = format!("{seconds} +0000");
    let signature = gix::actor::SignatureRef { name: "Ada".into(), email: "ada@example.com".into(), time: &time };
    repository
        .new_commit_as(signature, signature, message, tree, parents.iter().copied())
        .expect("write commit")
        .id
}

fn files(entries: &[(&str, &str)]) -> Vec<(String, String)> {
    entries
        .iter()
        .map(|(path, contents)| ((*path).to_owned(), (*contents).to_owned()))
        .collect()
}

/// Build the history the replay walks, newest first:
/// a two-file change, a 51-file change, a merge, a one-file change, an added file, and the root.
fn build_repository(root: &Path) {
    let repository = gix::init(root).expect("init repository");
    let base = [
        ("README.md", "# demo\n"),
        ("src/lib.rs", "pub mod parser;\n"),
        ("src/parser.rs", "pub fn parse() {}\n"),
    ];
    let first = commit(&repository, &files(&base), &[], 1_000, "Initial import");

    let mut with_cache = files(&base);
    with_cache[1].1 = "pub mod cache;\npub mod parser;\n".to_owned();
    with_cache.push(("src/cache.rs".to_owned(), "pub struct Cache;\n".to_owned()));
    let second = commit(&repository, &with_cache, &[first], 2_000, "Add a cache module");

    let mut bounds = with_cache.clone();
    bounds[2].1 = "pub fn parse() { check_bounds() }\nfn check_bounds() {}\n".to_owned();
    let third = commit(
        &repository,
        &bounds,
        &[second],
        3_000,
        "fix(parser): check parser bounds",
    );

    let merge = commit(&repository, &bounds, &[third, second], 4_000, "Merge branch 'cache'");

    let mut generated = bounds.clone();
    generated.extend((0..51).map(|index| (format!("gen/file{index}.txt"), format!("{index}\n"))));
    let big = commit(&repository, &generated, &[merge], 5_000, "Generate fixtures");

    let mut faster = generated.clone();
    faster[2].1 = "pub fn parse() { check_bounds() }\nfn check_bounds() { cache() }\n".to_owned();
    faster[3].1 = "pub struct Cache;\npub fn cache() {}\n".to_owned();
    let head = commit(&repository, &faster, &[big], 6_000, "Speed up the parser cache");
    fs::write(root.join(".git/refs/heads/main"), format!("{head}\n")).expect("point main at the newest commit");
}

fn build_dalil() -> PathBuf {
    let output = Command::new(env!("CARGO"))
        .args([
            "build",
            "--package",
            "dalil",
            "--bin",
            "dalil",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root"))
        .output()
        .expect("run cargo build");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact" && message["target"]["name"] == "dalil")
        .find_map(|message| message["executable"].as_str().map(PathBuf::from))
        .expect("cargo reports the dalil executable")
}

#[test]
fn replay_reports_each_command_and_cleans_up() {
    let scratch = std::env::temp_dir().join(format!("xtask-replay-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let repository = scratch.join("repository");
    let temp = scratch.join("tmp");
    let out = scratch.join("out");
    fs::create_dir_all(&repository).unwrap();
    // A directory a killed run would leave behind; no process can own this pid.
    let leftover = temp.join("dalil-replay-4000000000-0/checkout");
    fs::create_dir_all(&leftover).unwrap();
    build_repository(&repository);

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("replay")
        .arg("--repo")
        .arg(&repository)
        .args(["--commits", "3", "--dalil"])
        .arg(build_dalil())
        .arg("--out")
        .arg(&out)
        .env("TMPDIR", &temp)
        .output()
        .expect("run xtask replay");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let report: Value = serde_json::from_slice(&fs::read(out.join("results.json")).unwrap()).unwrap();
    let subjects: Vec<&str> = report["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["subject"].as_str().unwrap())
        .collect();
    assert_eq!(
        subjects,
        [
            "Speed up the parser cache",
            "fix(parser): check parser bounds",
            "Add a cache module"
        ]
    );
    let reasons: Vec<&str> = report["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|skip| skip["reason"].as_str().unwrap())
        .collect();
    assert_eq!(reasons, ["more than 50 changed files", "merge commit"]);

    let added = &report["cases"][2]["added"];
    assert_eq!(added, &serde_json::json!(["src/cache.rs"]));
    assert_eq!(
        report["cases"][0]["changed"],
        serde_json::json!(["src/cache.rs", "src/parser.rs"])
    );
    for case in report["cases"].as_array().unwrap() {
        let commands: Vec<&str> = case["runs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|run| run["command"].as_str().unwrap())
            .collect();
        assert_eq!(commands, ["dalil context", "dalil search", "baseline"]);
        for run in case["runs"].as_array().unwrap() {
            assert!(run["error"].is_null(), "{run}");
            assert!(run["recall"]["all_at_5"].is_number(), "{run}");
            assert!(
                run["bytes"].as_u64().unwrap() > 0 || run["command"] == "baseline",
                "{run}"
            );
        }
        assert_eq!(case["runs"][0]["worktree"], "clean");
        assert_eq!(case["runs"][0]["argv"][1], "context");
    }
    // The baseline finds both files of the newest change from its subject.
    assert_eq!(report["cases"][0]["runs"][2]["recall"]["all_at_5"], 1.0);

    let summary = report["summary"].as_array().unwrap();
    assert_eq!(summary.len(), 3);
    for command in summary {
        assert_eq!(command["cases"], 3);
        assert_eq!(command["failed"], 0);
        assert_eq!(command["parent_cases"], 3);
    }
    let markdown = fs::read_to_string(out.join("summary.md")).unwrap();
    assert!(markdown.contains("| dalil context | 3 | 0 |"), "{markdown}");

    let remaining: Vec<_> = fs::read_dir(&temp)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(remaining.is_empty(), "{remaining:?}");
    fs::remove_dir_all(scratch).unwrap();
}

/// A stand-in for Dalil that copies each checkout's `.git` to `$DALIL_PROBE` and reports no paths. With
/// `DALIL_SLEEP` set, `context` touches `$DALIL_PROBE/started` and sleeps instead.
#[cfg(unix)]
fn fake_dalil(directory: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let script = directory.join("fake-dalil");
    fs::write(
        &script,
        r#"#!/bin/sh
case "$1" in
--version) echo "dalil 0.0.0-fake" ;;
context)
    if [ -n "$DALIL_SLEEP" ]; then
        touch "$DALIL_PROBE/started"
        exec sleep 30
    fi
    mkdir -p "$DALIL_PROBE/$$"
    cp -R .git "$DALIL_PROBE/$$/.git"
    echo '{"context":{"files":[]}}' ;;
search) echo '{"search":{"matches":[]}}' ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    script
}

#[cfg(unix)]
fn replay_command(repository: &Path, dalil: &Path, temp: &Path, out: &Path, probe: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command
        .arg("replay")
        .arg("--repo")
        .arg(repository)
        .args(["--commits", "3", "--dalil"])
        .arg(dalil)
        .arg("--out")
        .arg(out)
        .env("TMPDIR", temp)
        .env("DALIL_PROBE", probe);
    command
}

#[cfg(unix)]
fn scratch(name: &str) -> PathBuf {
    let scratch = std::env::temp_dir().join(format!("xtask-replay-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    for directory in ["repository", "tmp", "probe"] {
        fs::create_dir_all(scratch.join(directory)).unwrap();
    }
    scratch
}

#[cfg(unix)]
#[test]
fn each_checkout_has_only_the_parent_reachable() {
    let scratch = scratch("refs");
    let repository = scratch.join("repository");
    let probe = scratch.join("probe");
    build_repository(&repository);
    // A tag on the newest commit must not reach the checkouts.
    let head = fs::read_to_string(repository.join(".git/refs/heads/main")).unwrap();
    fs::create_dir_all(repository.join(".git/refs/tags")).unwrap();
    fs::write(repository.join(".git/refs/tags/v1"), &head).unwrap();
    fs::write(
        repository.join(".git/packed-refs"),
        format!("{} refs/tags/v2\n", head.trim()),
    )
    .unwrap();

    let output = replay_command(
        &repository,
        &fake_dalil(&scratch),
        &scratch.join("tmp"),
        &scratch.join("out"),
        &probe,
    )
    .output()
    .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let report: Value = serde_json::from_slice(&fs::read(scratch.join("out/results.json")).unwrap()).unwrap();
    let mut parents: Vec<String> = report["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["parent"].as_str().unwrap().to_owned())
        .collect();
    parents.sort();
    assert_eq!(parents.len(), 3);

    let mut heads = Vec::new();
    for copy in fs::read_dir(&probe).unwrap() {
        let copy = copy.unwrap().path();
        assert!(!copy.join(".git/packed-refs").exists());
        let checkout = gix::open(&copy).unwrap();
        let refs: Vec<String> = checkout
            .references()
            .unwrap()
            .all()
            .unwrap()
            .map(|reference| reference.unwrap().name().as_bstr().to_string())
            .collect();
        assert_eq!(refs, ["refs/heads/replay"]);
        heads.push(checkout.head_id().unwrap().to_string());
    }
    heads.sort();
    assert_eq!(heads, parents);
    fs::remove_dir_all(scratch).unwrap();
}

#[cfg(unix)]
#[test]
fn an_interrupted_replay_removes_its_checkout() {
    use std::os::unix::process::CommandExt;
    use std::time::{Duration, Instant};

    let scratch = scratch("interrupt");
    let repository = scratch.join("repository");
    let temp = scratch.join("tmp");
    let probe = scratch.join("probe");
    build_repository(&repository);
    let mut child = replay_command(&repository, &fake_dalil(&scratch), &temp, &scratch.join("out"), &probe)
        .env("DALIL_SLEEP", "1")
        .process_group(0)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let pid = libc::pid_t::try_from(child.id()).unwrap();

    let deadline = Instant::now() + Duration::from_secs(20);
    while !probe.join("started").exists() {
        assert!(Instant::now() < deadline, "the first case never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    // As a terminal does on Ctrl-C: the whole process group gets SIGINT.
    // SAFETY: kill only sends a signal to the group the test spawned.
    assert_eq!(unsafe { libc::kill(-pid, libc::SIGINT) }, 0);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("replay did not exit after SIGINT");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(!status.success());
    let remaining: Vec<_> = fs::read_dir(&temp)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(remaining.is_empty(), "{remaining:?}");
    assert!(!scratch.join("out/results.json").exists());
    fs::remove_dir_all(scratch).unwrap();
}

#[cfg(unix)]
#[test]
fn a_tree_that_names_git_is_skipped() {
    let scratch = scratch("unsafe");
    let repository = scratch.join("repository");
    let git = gix::init(&repository).unwrap();
    let first = commit(
        &git,
        &files(&[("README.md", "# demo\n"), ("GIT~1/hooks/post-checkout", "#!/bin/sh\n")]),
        &[],
        1_000,
        "Initial import",
    );
    let second = commit(
        &git,
        &files(&[
            ("README.md", "# demo 2\n"),
            ("GIT~1/hooks/post-checkout", "#!/bin/sh\n"),
        ]),
        &[first],
        2_000,
        "Edit the readme",
    );
    fs::write(repository.join(".git/refs/heads/main"), format!("{second}\n")).unwrap();

    let output = replay_command(
        &repository,
        &fake_dalil(&scratch),
        &scratch.join("tmp"),
        &scratch.join("out"),
        &scratch.join("probe"),
    )
    .output()
    .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let report: Value = serde_json::from_slice(&fs::read(scratch.join("out/results.json")).unwrap()).unwrap();
    assert_eq!(report["cases"], serde_json::json!([]));
    let reasons: Vec<&str> = report["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|skip| skip["reason"].as_str().unwrap())
        .collect();
    assert_eq!(reasons, ["unsafe tree: entry name \"GIT~1\"", "root commit"]);
    fs::remove_dir_all(scratch).unwrap();
}
