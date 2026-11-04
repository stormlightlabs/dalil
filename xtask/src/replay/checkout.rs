//! Temporary checkouts of a commit's parent, read from the object database.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use gix::bstr::ByteSlice;
use gix::objs::tree::EntryKind;

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

/// Remove case directories left by replay runs that are no longer running.
pub fn remove_leftovers(temp_root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    let entries = match fs::read_dir(temp_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(removed),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(rest) = name.to_str().and_then(|name| name.strip_prefix(TEMP_PREFIX)) else {
            continue;
        };
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let owner = rest.split('-').next().and_then(|pid| pid.parse::<u32>().ok());
        if owner.is_some_and(|pid| pid != std::process::id() && process_alive(pid)) {
            continue;
        }
        fs::remove_dir_all(entry.path())?;
        removed.push(entry.path());
    }
    removed.sort();
    Ok(removed)
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
        entries.insert(name, (entry.mode.kind(), entry.oid.to_owned()));
    }
    Ok(entries)
}

/// Reject tree entry names that could escape the checkout or touch its `.git`.
fn validate_component(bytes: &[u8]) -> io::Result<String> {
    let name = std::str::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "tree entry name is not UTF-8"))?;
    let unsafe_name = name.is_empty()
        || name == "."
        || name == ".."
        || name.eq_ignore_ascii_case(".git")
        || name.contains(['/', '\\', '\0']);
    if unsafe_name {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsafe tree entry name {name:?}"),
        ));
    }
    Ok(name.to_owned())
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() { name.to_owned() } else { format!("{prefix}/{name}") }
}

/// Every non-tree entry under `tree`, keyed by its slash-separated path.
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

    for (path, (kind, oid)) in files {
        check_interrupted()?;
        let target = root.join(path);
        if let Some(directory) = target.parent() {
            fs::create_dir_all(directory)?;
        }
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
            EntryKind::Commit => fs::create_dir_all(&target)?,
            EntryKind::Tree => {}
        }
    }
    gix::open(root).map_err(other)?;
    Ok(())
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
        let unparsable = root.join(format!("{TEMP_PREFIX}junk"));
        let own = root.join(format!("{TEMP_PREFIX}{}-3", std::process::id()));
        let unrelated = root.join("other-tool-1");
        let plain_file = root.join(format!("{TEMP_PREFIX}file"));
        for directory in [&dead, &unparsable, &own, &unrelated] {
            fs::create_dir_all(directory.join("nested")).unwrap();
        }
        fs::write(&plain_file, "x").unwrap();

        let mut expected = vec![dead.clone(), unparsable.clone(), own.clone()];
        expected.sort();
        assert_eq!(remove_leftovers(&root).unwrap(), expected);
        assert!(unrelated.exists());
        assert!(plain_file.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn directories_of_live_runs_are_kept() {
        let root = scratch("live");
        // pid 1 always exists.
        let live = root.join(format!("{TEMP_PREFIX}1-0"));
        fs::create_dir_all(&live).unwrap();
        assert!(remove_leftovers(&root).unwrap().is_empty());
        assert!(live.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsafe_entry_names_are_rejected() {
        for name in ["", ".", "..", ".git", ".GIT", "a/b", "a\\b"] {
            assert!(validate_component(name.as_bytes()).is_err(), "{name:?}");
        }
        assert_eq!(validate_component(b".github").unwrap(), ".github");
    }
}
