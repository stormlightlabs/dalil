use std::{
    collections::BTreeMap,
    env,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

pub(crate) static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct FixtureRepository {
    pub(crate) root: PathBuf,
    pub(crate) cache: PathBuf,
    pub(crate) temporary_root: PathBuf,
}

impl FixtureRepository {
    pub(crate) fn new() -> Self {
        let suffix = format!(
            "dalil-cli-{}-{}",
            std::process::id(),
            FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let temporary_root = env::temp_dir().join(suffix);
        let root = temporary_root.join("repository");
        let cache = temporary_root.join("xdg-cache");

        fs::create_dir_all(root.join(".git/objects")).expect("create fixture Git objects directory");
        fs::create_dir_all(root.join(".git/refs/heads")).expect("create fixture Git refs directory");
        fs::create_dir_all(&cache).expect("create fixture cache directory");
        write_file(root.join(".git/HEAD"), b"ref: refs/heads/main\n");
        write_file(
            root.join(".git/config"),
            b"[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
        );
        gix::open(&root).expect("open valid fixture repository");

        Self { root, cache, temporary_root }
    }

    pub(crate) fn run(&self, arguments: &[&str]) -> Output {
        self.command(arguments).output().expect("run dalil fixture command")
    }

    pub(crate) fn command(&self, arguments: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dalil"));
        command
            .args(arguments)
            .current_dir(&self.root)
            .env("XDG_CACHE_HOME", &self.cache);
        command
    }
}

impl Drop for FixtureRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temporary_root);
    }
}

pub(crate) struct HistoryFixtureRepository {
    pub(crate) root: PathBuf,
    pub(crate) cache: PathBuf,
    pub(crate) temporary_root: PathBuf,
}

impl HistoryFixtureRepository {
    pub(crate) fn new() -> Self {
        let suffix = format!(
            "dalil-history-{}-{}",
            std::process::id(),
            FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let temporary_root = env::temp_dir().join(suffix);
        let root = temporary_root.join("repository");
        let cache = temporary_root.join("xdg-cache");
        fs::create_dir_all(&root).expect("create history fixture repository");
        fs::create_dir_all(root.join("src")).expect("create history fixture source scope");
        fs::create_dir_all(&cache).expect("create history fixture cache");

        let repository = gix::init(&root).expect("initialize history fixture repository");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after the Unix epoch")
            .as_secs() as i64;
        let day = 86_400;
        let initial_tree = write_tree(&repository, &[("legacy.txt", "legacy")]);
        let first = write_commit(
            &repository,
            initial_tree,
            &[],
            "Alice",
            "alice@example.com",
            now - 400 * day,
            "Initial import",
        );

        let second_tree = write_tree(
            &repository,
            &[("legacy.txt", "legacy"), ("src/lib.rs", "pub fn parse() {}")],
        );
        let second = write_commit(
            &repository,
            second_tree,
            &[first],
            "Robert Alias",
            "ALIAS@example.com",
            now - 200 * day,
            "Implement fixture prefix debug parser",
        );

        let third_tree = write_tree(
            &repository,
            &[("legacy.txt", "legacy"), ("src/lib.rs", "pub fn parse() { 1 }")],
        );
        let third = write_commit(
            &repository,
            third_tree,
            &[second],
            "Alice",
            "alice@example.com",
            now - 20 * day,
            "Fix parser bug",
        );

        let side_tree = write_tree(
            &repository,
            &[
                ("legacy.txt", "legacy"),
                ("src/lib.rs", "pub fn parse() { 1 }"),
                ("src/side.rs", "pub fn side() {}"),
            ],
        );
        let side = write_commit(
            &repository,
            side_tree,
            &[second],
            "Carol",
            "carol@example.com",
            now - 15 * day,
            "Emergency hotfix side work",
        );
        let merge = write_commit(
            &repository,
            third_tree,
            &[third, side],
            "Maintainer",
            "maintainer@example.com",
            now - 5 * day,
            "Merge side work",
        );

        let final_tree = write_tree(
            &repository,
            &[
                (".mailmap", "Bob <bob@example.com> Robert Alias <alias@example.com>\n"),
                ("legacy.txt", "legacy"),
                ("src/binary.rs", "\0binary"),
                ("src/empty.rs", ""),
                ("src/generated.rs", "// generated file\npub fn generated() {}"),
                ("src/lib.rs", "pub fn parse() { 1 }"),
                ("src/main.rs", "fn main() { 1 }"),
            ],
        );
        let final_commit = write_commit(
            &repository,
            final_tree,
            &[merge],
            "Bob",
            "bob@example.com",
            now - 2 * day,
            "Rollback entrypoint",
        );
        drop(repository);
        write_file(root.join(".git/HEAD"), b"ref: refs/heads/main\n");
        write_file(
            root.join(".git/refs/heads/main"),
            format!("{final_commit}\n").as_bytes(),
        );
        gix::open(&root).expect("open history fixture repository");

        Self { root, cache, temporary_root }
    }

    pub(crate) fn run(&self, arguments: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dalil"));
        command
            .args(arguments)
            .current_dir(&self.root)
            .env("XDG_CACHE_HOME", &self.cache);
        command.output().expect("run history fixture command")
    }
}

impl Drop for HistoryFixtureRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temporary_root);
    }
}

pub(crate) fn write_tree(repository: &gix::Repository, files: &[(&str, &str)]) -> gix::ObjectId {
    #[derive(Default)]
    struct TreeNode {
        files: Vec<(String, gix::ObjectId)>,
        directories: BTreeMap<String, TreeNode>,
    }

    fn insert_file(node: &mut TreeNode, path: &str, blob: gix::ObjectId) {
        let mut components = path.split('/');
        let Some(first) = components.next() else {
            return;
        };
        let rest = components.collect::<Vec<_>>();
        if rest.is_empty() {
            node.files.push((first.to_owned(), blob));
        } else {
            insert_file(
                node.directories.entry(first.to_owned()).or_default(),
                &rest.join("/"),
                blob,
            );
        }
    }

    fn write_node(repository: &gix::Repository, node: TreeNode) -> gix::ObjectId {
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
            .expect("write fixture tree")
            .detach()
    }

    let mut root = TreeNode::default();
    for (path, contents) in files {
        let blob = repository
            .write_object(gix::objs::Blob { data: contents.as_bytes().to_vec() })
            .expect("write fixture blob")
            .detach();
        insert_file(&mut root, path, blob);
    }
    write_node(repository, root)
}

pub(crate) fn write_commit(
    repository: &gix::Repository, tree: gix::ObjectId, parents: &[gix::ObjectId], name: &str, email: &str,
    seconds: i64, message: &str,
) -> gix::ObjectId {
    let timestamp = format!("{seconds} +0000");
    let signature = gix::actor::SignatureRef { name: name.into(), email: email.into(), time: &timestamp };
    repository
        .new_commit_as(signature, signature, message, tree, parents.iter().copied())
        .expect("write fixture commit")
        .id
}

pub(crate) fn write_file(path: impl AsRef<Path>, contents: &[u8]) {
    let mut file = File::create(path).expect("create fixture file");
    file.write_all(contents).expect("write fixture file");
}

pub(crate) fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

pub(crate) fn orientation_recommendations(value: &Value) -> Vec<&Value> {
    ["starting_points", "runtime_entry_points", "tests", "next_reads"]
        .into_iter()
        .flat_map(|section| value["orientation"][section].as_array().into_iter().flatten())
        .collect()
}

pub(crate) fn cache_json_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "json") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

pub(crate) fn assert_plain_report(output: &str) {
    assert!(
        !output.contains('\u{1b}'),
        "report stdout must not include ANSI escape sequences: {output:?}"
    );
    assert!(
        output
            .bytes()
            .all(|byte| { byte == b'\n' || byte == b'\r' || byte >= 0x20 })
    );
}
