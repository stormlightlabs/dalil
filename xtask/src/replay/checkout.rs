//! Temporary checkouts of a commit's parent, read from the object database.

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use gix::bstr::ByteSlice;
use gix::objs::tree::EntryKind;
use unicode_normalization::UnicodeNormalization;

use super::other;

/// Every case directory starts with this name, so a later run can find leftovers.
pub const TEMP_PREFIX: &str = "dalil-replay-";

/// The branch the temporary repository's `HEAD` points at.
const REPLAY_BRANCH: &str = "refs/heads/replay";

pub type TreeFiles = BTreeMap<String, (EntryKind, gix::ObjectId)>;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn on_signal(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// Record SIGINT and SIGTERM instead of exiting, so the running case can remove its directory.
pub fn install_interrupt_handler() {
    #[cfg(unix)]
    {
        let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        // SAFETY: the handler only stores to an atomic, which is async-signal-safe.
        unsafe {
            libc::signal(libc::SIGINT, handler);
            libc::signal(libc::SIGTERM, handler);
        }
    }
}

pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

pub fn check_interrupted() -> io::Result<()> {
    if interrupted() {
        Err(io::Error::new(io::ErrorKind::Interrupted, "replay interrupted"))
    } else {
        Ok(())
    }
}

/// Why a tree cannot be written safely. Its commit is skipped; the run goes on.
#[derive(Debug)]
pub struct UnsafeTree(String);

impl fmt::Display for UnsafeTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsafe tree: {}", self.0)
    }
}

impl std::error::Error for UnsafeTree {}

fn unsafe_tree(reason: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, UnsafeTree(reason.into()))
}

/// The skip reason, when `error` is an [`UnsafeTree`].
pub fn unsafe_reason(error: &io::Error) -> Option<String> {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<UnsafeTree>())
        .map(ToString::to_string)
}

/// A case's temporary directory, removed when dropped.
pub struct CaseDir {
    path: PathBuf,
}

impl CaseDir {
    pub fn create(temp_root: &Path, case: usize) -> io::Result<Self> {
        let path = temp_root.join(format!("{TEMP_PREFIX}{}-{case}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CaseDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// The outcome of [`remove_leftovers`].
#[derive(Debug, Default)]
pub struct Leftovers {
    pub removed: Vec<PathBuf>,
    /// Directories that matched but could not be removed; the run goes on without them.
    pub failed: Vec<(PathBuf, io::Error)>,
}

/// Remove case directories left by replay runs of this user that are no longer running.
pub fn remove_leftovers(temp_root: &Path) -> io::Result<Leftovers> {
    remove_leftovers_with(temp_root, |path| fs::remove_dir_all(path))
}

fn remove_leftovers_with(temp_root: &Path, remove: impl Fn(&Path) -> io::Result<()>) -> io::Result<Leftovers> {
    let mut leftovers = Leftovers::default();
    let entries = match fs::read_dir(temp_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(leftovers),
        Err(error) => return Err(error),
    };
    // An entry that cannot be read is not one this run can judge, so it is left alone.
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(case_dir_pid) else {
            continue;
        };
        // `DirEntry::metadata` does not follow links, so a link named like a case directory is skipped.
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_dir() || !owned_by_current_user(&metadata) {
            continue;
        }
        if pid != std::process::id() && process_alive(pid) {
            continue;
        }
        let path = entry.path();
        match remove(&path) {
            Ok(()) => leftovers.removed.push(path),
            Err(error) => leftovers.failed.push((path, error)),
        }
    }
    leftovers.removed.sort();
    Ok(leftovers)
}

/// The pid in a name [`CaseDir::create`] makes, `dalil-replay-<pid>-<case>`.
fn case_dir_pid(name: &str) -> Option<u32> {
    let (pid, case) = name.strip_prefix(TEMP_PREFIX)?.split_once('-')?;
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(case) || !digits(pid) {
        return None;
    }
    pid.parse().ok()
}

#[cfg(unix)]
fn owned_by_current_user(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no preconditions and cannot fail.
    metadata.uid() == unsafe { libc::geteuid() }
}

#[cfg(not(unix))]
fn owned_by_current_user(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 checks that the process exists and sends nothing.
    let result = unsafe { libc::kill(pid, 0) };
    let exists = result == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
    exists && !zombie(pid)
}

/// A killed run stays visible to `kill` until its parent reaps it.
#[cfg(target_os = "linux")]
fn zombie(pid: libc::pid_t) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit_once(')')
                .map(|(_, rest)| rest.trim_start().starts_with('Z'))
        })
        .unwrap_or(false)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn zombie(_pid: libc::pid_t) -> bool {
    false
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    false
}

fn tree_entries(repository: &gix::Repository, id: Option<gix::ObjectId>) -> io::Result<TreeFiles> {
    let mut entries = BTreeMap::new();
    let Some(id) = id else {
        return Ok(entries);
    };
    let tree = repository.find_tree(id).map_err(other)?;
    let decoded = gix::objs::TreeRef::from_bytes(&tree.data, id.kind()).map_err(other)?;
    for entry in decoded.entries {
        let name = validate_component(entry.filename.as_bytes())?;
        if entries
            .insert(name.clone(), (entry.mode.kind(), entry.oid.to_owned()))
            .is_some()
        {
            return Err(unsafe_tree(format!("entry {name:?} appears twice in one tree")));
        }
    }
    Ok(entries)
}

/// Reject tree entry names that could escape the checkout or touch its `.git`.
fn validate_component(bytes: &[u8]) -> io::Result<String> {
    let name = std::str::from_utf8(bytes).map_err(|_| unsafe_tree("an entry name is not UTF-8"))?;
    let unsafe_name =
        name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', '\0']) || is_git_alias(name);
    if unsafe_name {
        return Err(unsafe_tree(format!("entry name {name:?}")));
    }
    Ok(name.to_owned())
}

/// Code points HFS+ ignores in names, as git's `is_hfs_dotgit` lists them.
fn hfs_ignorable(c: char) -> bool {
    matches!(c, '\u{200c}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{206a}'..='\u{206f}' | '\u{feff}')
}

/// Whether HFS+ or NTFS would open `name` as `.git`. HFS+ ignores some code points and case; NTFS ignores case and
/// trailing dots and spaces, reads `name:stream` as `name`, and gives `.git` a short name such as `GIT~1`.
fn is_git_alias(name: &str) -> bool {
    let visible: String = name.chars().filter(|c| !hfs_ignorable(*c)).collect();
    let name = visible.trim_end_matches([' ', '.']).to_lowercase();
    if name == ".git" || name.starts_with(".git:") {
        return true;
    }
    name.strip_prefix("git~")
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
}

/// `path` as a case-insensitive, normalization-insensitive filesystem compares it.
fn fold(path: &str) -> String {
    path.chars()
        .filter(|c| !hfs_ignorable(*c))
        .collect::<String>()
        .to_lowercase()
        .nfc()
        .collect()
}

/// Reject a tree in which two paths, files or the directories above them, fold to the same name. On such a
/// filesystem the second would be written into or through the first.
fn check_collisions(files: &TreeFiles) -> io::Result<()> {
    let mut folded: HashMap<String, &str> = HashMap::new();
    let mut checked: HashSet<&str> = HashSet::new();
    for path in files.keys() {
        let prefixes = path
            .match_indices('/')
            .map(|(end, _)| &path[..end])
            .chain(std::iter::once(path.as_str()));
        for prefix in prefixes {
            if !checked.insert(prefix) {
                continue;
            }
            match folded.entry(fold(prefix)) {
                Entry::Occupied(first) => {
                    return Err(unsafe_tree(format!(
                        "{:?} and {prefix:?} name the same path when case and normalization are ignored",
                        first.get()
                    )));
                }
                Entry::Vacant(slot) => {
                    slot.insert(prefix);
                }
            }
        }
    }
    Ok(())
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() { name.to_owned() } else { format!("{prefix}/{name}") }
}

/// Every non-tree entry under `tree`, keyed by its slash-separated path. Fails with [`UnsafeTree`] on a name that
/// could escape the checkout or two paths that would collide.
pub fn flatten_tree(repository: &gix::Repository, tree: gix::ObjectId) -> io::Result<TreeFiles> {
    let mut files = BTreeMap::new();
    let mut stack = vec![(tree, String::new())];
    while let Some((id, prefix)) = stack.pop() {
        for (name, (kind, oid)) in tree_entries(repository, Some(id))? {
            let path = join(&prefix, &name);
            if kind == EntryKind::Tree {
                stack.push((oid, path));
            } else {
                files.insert(path, (kind, oid));
            }
        }
    }
    check_collisions(&files)?;
    Ok(files)
}

/// Paths whose entry differs between two trees. Stops once more than `limit` paths differ.
pub fn changed_paths(
    repository: &gix::Repository, old: gix::ObjectId, new: gix::ObjectId, limit: usize,
) -> io::Result<BTreeSet<String>> {
    let mut changed = BTreeSet::new();
    diff_trees(repository, Some(old), Some(new), "", limit, &mut changed)?;
    Ok(changed)
}

fn diff_trees(
    repository: &gix::Repository, old: Option<gix::ObjectId>, new: Option<gix::ObjectId>, prefix: &str, limit: usize,
    changed: &mut BTreeSet<String>,
) -> io::Result<()> {
    if old == new {
        return Ok(());
    }
    let old = tree_entries(repository, old)?;
    let new = tree_entries(repository, new)?;
    let names: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    for name in names {
        if changed.len() > limit {
            return Ok(());
        }
        let (before, after) = (old.get(name), new.get(name));
        if before == after {
            continue;
        }
        let path = join(prefix, name);
        let subtree = |entry: Option<&(EntryKind, gix::ObjectId)>| {
            entry.filter(|(kind, _)| *kind == EntryKind::Tree).map(|(_, oid)| *oid)
        };
        let is_file =
            |entry: Option<&(EntryKind, gix::ObjectId)>| entry.is_some_and(|(kind, _)| *kind != EntryKind::Tree);
        if subtree(before).is_some() || subtree(after).is_some() {
            diff_trees(repository, subtree(before), subtree(after), &path, limit, changed)?;
        }
        if is_file(before) || is_file(after) {
            changed.insert(path);
        }
    }
    Ok(())
}

/// Write a repository at `root` whose `HEAD` is `parent`, and the files of `parent`'s tree.
///
/// Objects come from `source` through `objects/info/alternates`, so history stays readable. Blobs are written
/// directly: no `git` process, checkout, hooks, filters or attributes run. Symbolic links are written as links on
/// Unix and skipped elsewhere; submodules become empty directories.
pub fn materialize(source: &gix::Repository, parent: gix::ObjectId, files: &TreeFiles, root: &Path) -> io::Result<()> {
    let git = root.join(".git");
    fs::create_dir_all(git.join("objects/info"))?;
    fs::create_dir_all(git.join("refs/heads"))?;
    let source_objects = fs::canonicalize(source.common_dir().join("objects"))?;
    let alternates = source_objects
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "source object path is not UTF-8"))?;
    fs::write(git.join("objects/info/alternates"), format!("{alternates}\n"))?;
    let shallow = source.common_dir().join("shallow");
    if shallow.is_file() {
        fs::copy(&shallow, git.join("shallow"))?;
    }
    fs::write(
        git.join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
    )?;
    fs::write(git.join("HEAD"), format!("ref: {REPLAY_BRANCH}\n"))?;
    fs::write(git.join(REPLAY_BRANCH), format!("{parent}\n"))?;

    // Links go last, so no file or directory of the tree is written through a link the tree made.
    let (links, others): (Vec<_>, Vec<_>) = files.iter().partition(|(_, (kind, _))| *kind == EntryKind::Link);
    for (path, (kind, oid)) in others.into_iter().chain(links) {
        check_interrupted()?;
        let target = prepare_target(root, path)?;
        match kind {
            EntryKind::Blob | EntryKind::BlobExecutable => {
                let blob = source.find_blob(*oid).map_err(other)?;
                fs::write(&target, &blob.data)?;
                #[cfg(unix)]
                if *kind == EntryKind::BlobExecutable {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
                }
            }
            EntryKind::Link => {
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStrExt;
                    let blob = source.find_blob(*oid).map_err(other)?;
                    std::os::unix::fs::symlink(std::ffi::OsStr::from_bytes(&blob.data), &target)?;
                }
            }
            EntryKind::Commit => fs::create_dir(&target)?,
            EntryKind::Tree => {}
        }
    }
    gix::open(root).map_err(other)?;
    Ok(())
}

/// Create the directories above `path` inside `root` and return where `path` goes. Refuses when a component is a
/// link or `path` already exists, so a write cannot leave the checkout.
fn prepare_target(root: &Path, path: &str) -> io::Result<PathBuf> {
    let mut current = root.to_path_buf();
    let mut components = path.split('/').peekable();
    while let Some(component) = components.next() {
        current.push(component);
        let last = components.peek().is_none();
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(unsafe_tree(format!("{path:?} would be written through a link")));
            }
            Ok(_) if last => return Err(unsafe_tree(format!("{path:?} is written twice"))),
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Err(unsafe_tree(format!("{path:?} has a file as a parent"))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if !last {
                    fs::create_dir(&current)?;
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("xtask-checkout-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn case_dir_is_removed_on_drop() {
        let root = scratch("drop");
        let path = {
            let case = CaseDir::create(&root, 7).unwrap();
            fs::write(case.path().join("file"), "x").unwrap();
            case.path().to_owned()
        };
        assert!(!path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn leftovers_from_dead_runs_are_removed() {
        let root = scratch("leftovers");
        // No process can have this pid: it is outside the range of pid_t.
        let dead = root.join(format!("{TEMP_PREFIX}4000000000-0"));
        let own = root.join(format!("{TEMP_PREFIX}{}-3", std::process::id()));
        // Names replay never makes are left alone.
        let kept = [
            format!("{TEMP_PREFIX}junk"),
            format!("{TEMP_PREFIX}cache"),
            format!("{TEMP_PREFIX}4000000000"),
            format!("{TEMP_PREFIX}4000000000-x"),
            format!("{TEMP_PREFIX}4000000000-0-1"),
            format!("{TEMP_PREFIX}-0"),
            "other-tool-1".to_owned(),
        ]
        .map(|name| root.join(name));
        let plain_file = root.join(format!("{TEMP_PREFIX}4000000000-1"));
        for directory in kept.iter().chain([&dead, &own]) {
            fs::create_dir_all(directory.join("nested")).unwrap();
        }
        fs::write(&plain_file, "x").unwrap();

        let leftovers = remove_leftovers(&root).unwrap();
        let mut expected = vec![dead.clone(), own.clone()];
        expected.sort();
        assert_eq!(leftovers.removed, expected);
        assert!(leftovers.failed.is_empty());
        for directory in &kept {
            assert!(directory.exists(), "{}", directory.display());
        }
        assert!(plain_file.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_failed_removal_is_reported_and_the_rest_continue() {
        let root = scratch("failed-removal");
        let stuck = root.join(format!("{TEMP_PREFIX}4000000000-0"));
        let dead = root.join(format!("{TEMP_PREFIX}4000000000-1"));
        fs::create_dir_all(&stuck).unwrap();
        fs::create_dir_all(&dead).unwrap();
        let leftovers = remove_leftovers_with(&root, |path| {
            if path == stuck {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            } else {
                fs::remove_dir_all(path)
            }
        })
        .unwrap();
        assert_eq!(leftovers.removed, std::slice::from_ref(&dead));
        assert_eq!(leftovers.failed.len(), 1);
        assert_eq!(leftovers.failed[0].0, stuck);
        assert!(stuck.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn directories_of_other_users_are_kept() {
        let root = scratch("other-user");
        let theirs = root.join(format!("{TEMP_PREFIX}4000000000-0"));
        fs::create_dir_all(&theirs).unwrap();
        let path = std::ffi::CString::new(theirs.to_str().unwrap()).unwrap();
        // SAFETY: `path` is a valid C string for the duration of the call.
        if unsafe { libc::chown(path.as_ptr(), 54_321, 54_321) } != 0 {
            // Only root can give a directory away; elsewhere there is nothing to test.
            fs::remove_dir_all(root).unwrap();
            return;
        }
        let leftovers = remove_leftovers(&root).unwrap();
        assert!(leftovers.removed.is_empty());
        assert!(theirs.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn directories_of_live_runs_are_kept() {
        let root = scratch("live");
        // pid 1 always exists.
        let live = root.join(format!("{TEMP_PREFIX}1-0"));
        fs::create_dir_all(&live).unwrap();
        assert!(remove_leftovers(&root).unwrap().removed.is_empty());
        assert!(live.exists());
        fs::remove_dir_all(root).unwrap();
    }

    fn is_unsafe(error: &io::Error) -> bool {
        unsafe_reason(error).is_some()
    }

    #[test]
    fn unsafe_entry_names_are_rejected() {
        let names = [
            "",
            ".",
            "..",
            ".git",
            ".GIT",
            "a/b",
            "a\\b",
            ".g\u{200c}it",
            "\u{feff}.git",
            ".gi\u{206f}T",
            ".git.",
            ".git ",
            ".git::$INDEX_ALLOCATION",
            "git~1",
            "GIT~12",
        ];
        for name in names {
            let error = validate_component(name.as_bytes()).unwrap_err();
            assert!(is_unsafe(&error), "{name:?}");
        }
        assert!(is_unsafe(&validate_component(b"\xff").unwrap_err()));
        for name in [".github", "git~", "git~x", "git", ".gitignore"] {
            assert_eq!(validate_component(name.as_bytes()).unwrap(), name);
        }
    }

    fn tree_files(entries: &[(&str, EntryKind)]) -> TreeFiles {
        entries
            .iter()
            .map(|(path, kind)| ((*path).to_owned(), (*kind, gix::ObjectId::null(gix::hash::Kind::Sha1))))
            .collect()
    }

    #[test]
    fn paths_that_fold_together_are_rejected() {
        let colliding = [
            // A link would sort first and the blob would be written through it.
            [("Src", EntryKind::Link), ("src/etc/x", EntryKind::Blob)],
            [("README.md", EntryKind::Blob), ("readme.MD", EntryKind::Blob)],
            [("caf\u{e9}.txt", EntryKind::Blob), ("cafe\u{301}.txt", EntryKind::Blob)],
            [("ab/x", EntryKind::Blob), ("a\u{200d}b/y", EntryKind::Blob)],
        ];
        for entries in colliding {
            let error = check_collisions(&tree_files(&entries)).unwrap_err();
            assert!(is_unsafe(&error), "{entries:?}");
        }
        let distinct = tree_files(&[
            ("src/a.rs", EntryKind::Blob),
            ("src/b.rs", EntryKind::Blob),
            ("srcs", EntryKind::Link),
        ]);
        check_collisions(&distinct).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn writes_never_pass_through_a_link() {
        let root = scratch("through-link");
        let checkout = root.join("checkout");
        let outside = root.join("outside");
        fs::create_dir_all(&checkout).unwrap();
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, checkout.join("src")).unwrap();
        for path in ["src/etc/x", "src"] {
            assert!(is_unsafe(&prepare_target(&checkout, path).unwrap_err()), "{path}");
        }
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn links_are_written_after_files() {
        let root = scratch("links-last");
        let source_path = root.join("source");
        let checkout = root.join("checkout");
        let outside = root.join("outside");
        fs::create_dir_all(&checkout).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let source = gix::init(&source_path).unwrap();
        let link = source
            .write_object(gix::objs::Blob { data: outside.to_str().unwrap().as_bytes().to_vec() })
            .unwrap()
            .detach();
        let blob = source
            .write_object(gix::objs::Blob { data: b"x".to_vec() })
            .unwrap()
            .detach();
        // As a case-insensitive filesystem sees `Src -> outside` and `src/etc/x`: the link sorts first.
        let files: TreeFiles = [
            ("lib".to_owned(), (EntryKind::Link, link)),
            ("lib/etc/x".to_owned(), (EntryKind::Blob, blob)),
            ("other.txt".to_owned(), (EntryKind::Blob, blob)),
        ]
        .into_iter()
        .collect();
        let parent = gix::ObjectId::null(gix::hash::Kind::Sha1);
        let error = materialize(&source, parent, &files, &checkout).unwrap_err();
        assert!(is_unsafe(&error), "{error}");
        // The blob went inside the checkout, and the link that would have redirected it was refused.
        assert_eq!(fs::read(checkout.join("lib/etc/x")).unwrap(), b"x");
        assert!(
            !fs::symlink_metadata(checkout.join("lib"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }
}
