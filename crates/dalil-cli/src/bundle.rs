use std::{
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use dalil_core::EvidenceMap;

static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, thiserror::Error)]
pub(crate) enum BundleError {
    #[error("the repository root is not a directory")]
    RepositoryRoot,
    #[error("refusing to write through symlink or reparse-point path `{0}`")]
    UnsafePath(PathBuf),
    #[error("`.dalil` exists but is not a directory")]
    DestinationCollision,
    #[error("could not publish the repository bundle: {0}")]
    Io(#[from] io::Error),
    #[error("could not serialize the repository evidence map: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) struct PublishedBundle {
    pub(crate) directory: PathBuf,
    pub(crate) snapshot_id: String,
}

pub(crate) fn publish(map: &EvidenceMap) -> Result<PublishedBundle, BundleError> {
    let root = PathBuf::from(&map.repository.canonical_root);
    let directory = private_bundle_directory(&root)?;
    let mut json = serde_json::to_vec(map)?;
    json.push(b'\n');
    let markdown = render_markdown(map);
    atomic_write(&directory, "map.json", &json)?;
    atomic_write(&directory, "map.md", markdown.as_bytes())?;
    Ok(PublishedBundle { directory, snapshot_id: map.snapshot_id.clone() })
}

fn private_bundle_directory(root: &Path) -> Result<PathBuf, BundleError> {
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() || is_reparse_or_symlink(&metadata) {
        return Err(BundleError::RepositoryRoot);
    }
    let directory = root.join(".dalil");
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if is_reparse_or_symlink(&metadata) => return Err(BundleError::UnsafePath(directory)),
        Ok(metadata) if !metadata.is_dir() => return Err(BundleError::DestinationCollision),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(&directory)?,
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(&directory)?;
    if is_reparse_or_symlink(&metadata) {
        return Err(BundleError::UnsafePath(directory));
    }
    if !metadata.is_dir() {
        return Err(BundleError::DestinationCollision);
    }
    set_private_directory(&directory)?;
    Ok(directory)
}

#[cfg(unix)]
fn atomic_write(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), BundleError> {
    atomic_write_unix(directory, name, bytes).map_err(BundleError::Io)
}

#[cfg(not(unix))]
fn atomic_write(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), BundleError> {
    let destination = directory.join(name);
    if fs::symlink_metadata(&destination).is_ok_and(|metadata| is_reparse_or_symlink(&metadata)) {
        return Err(BundleError::UnsafePath(destination));
    }
    let temporary = directory.join(format!(
        ".{name}.tmp-{}-{}",
        std::process::id(),
        TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        match fs::rename(&temporary, &destination) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                fs::remove_file(&destination)?;
                fs::rename(&temporary, &destination)?;
            }
            Err(error) => return Err(error),
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(BundleError::Io)
}

#[cfg(unix)]
fn atomic_write_unix(directory: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::OpenOptionsExt,
    };

    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(directory)?;
    if unsafe { libc::fchmod(directory.as_raw_fd(), 0o700) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let temporary = format!(
        ".{name}.tmp-{}-{}",
        std::process::id(),
        TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let temporary_c = CString::new(temporary.as_bytes()).expect("fixed temporary names contain no NUL");
    let destination_c = CString::new(name).expect("fixed destination names contain no NUL");
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            temporary_c.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut file = unsafe { fs::File::from_raw_fd(fd) };
    let result = (|| -> io::Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if unsafe {
            libc::renameat(
                directory.as_raw_fd(),
                temporary_c.as_ptr(),
                directory.as_raw_fd(),
                destination_c.as_ptr(),
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        if unsafe { libc::fsync(directory.as_raw_fd()) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    })();
    if result.is_err() {
        unsafe {
            libc::unlinkat(directory.as_raw_fd(), temporary_c.as_ptr(), 0);
        }
    }
    result
}

fn render_markdown(map: &EvidenceMap) -> String {
    const MAX_ITEMS: usize = 32;
    let mut output = String::new();
    writeln!(output, "# Dalil repository evidence map").expect("writing to a string cannot fail");
    writeln!(output).expect("writing to a string cannot fail");
    writeln!(output, "Snapshot: `{}`", map.snapshot_id).expect("writing to a string cannot fail");
    writeln!(output, "Schema version: {}", map.schema_version).expect("writing to a string cannot fail");
    writeln!(output, "Producer version: {}", map.producer_version).expect("writing to a string cannot fail");
    writeln!(output, "Captured: {}", map.provenance.captured_at).expect("writing to a string cannot fail");
    writeln!(output, "Repository: `{}`", map.repository.canonical_root).expect("writing to a string cannot fail");
    writeln!(output, "Scope: `{}`", map.scope).expect("writing to a string cannot fail");
    writeln!(
        output,
        "Revision: `{}`",
        map.revision.oid.as_deref().unwrap_or("unborn or unavailable")
    )
    .expect("writing to a string cannot fail");
    writeln!(output, "Worktree fingerprint: `{}`", map.worktree_fingerprint).expect("writing to a string cannot fail");

    section(&mut output, "Collections");
    for (name, summary) in [
        ("Projects", &map.collections.projects),
        ("Files", &map.collections.files),
        ("Source omissions", &map.collections.omissions),
        ("Symbols", &map.collections.symbols),
        ("Relationships", &map.collections.relationships),
        ("Landmarks", &map.collections.landmarks),
        ("Tests", &map.collections.tests),
    ] {
        writeln!(
            output,
            "- {name}: {} returned of {}{}",
            summary.returned,
            summary.total,
            if summary.truncated { " (truncated)" } else { "" }
        )
        .expect("writing to a string cannot fail");
    }
    writeln!(
        output,
        "- History commits: {} returned of {}{}",
        map.collections.history.commits.returned,
        map.collections.history.commits.total,
        if map.collections.history.commits.truncated { " (truncated)" } else { "" }
    )
    .expect("writing to a string cannot fail");

    section(&mut output, "Project roots");
    list_limited(
        &mut output,
        map.projects.iter().map(|project| {
            format!(
                "`{}` ({}) — {}; {}",
                project.project.path,
                project.project.kind.label(),
                project.project.reason,
                project.id
            )
        }),
        map.projects.len(),
        MAX_ITEMS,
    );
    section(&mut output, "Landmarks");
    list_limited(
        &mut output,
        map.landmarks
            .iter()
            .map(|landmark| format!("`{}` — {}", landmark.landmark.path, landmark.landmark.reason)),
        map.landmarks.len(),
        MAX_ITEMS,
    );
    section(&mut output, "Files");
    list_limited(
        &mut output,
        map.files.iter().map(|file| {
            format!(
                "`{}` ({}, {}; {})",
                file.file.path,
                file.file.language.label(),
                file.file.status.label(),
                file.id
            )
        }),
        map.files.len(),
        MAX_ITEMS,
    );
    section(&mut output, "Omissions");
    list_limited(
        &mut output,
        map.omissions
            .iter()
            .map(|omission| format!("`{}` — {}", omission.path, omission.detail)),
        map.omissions.len(),
        MAX_ITEMS,
    );
    section(&mut output, "Symbols");
    list_limited(
        &mut output,
        map.symbols.iter().map(|symbol| {
            format!(
                "`{}` in `{}` ({}, {}; {})",
                symbol.symbol.name,
                symbol.path,
                symbol.symbol.kind.label(),
                symbol.symbol.role.label(),
                symbol.id
            )
        }),
        map.symbols.len(),
        MAX_ITEMS,
    );
    section(&mut output, "Relationships");
    list_limited(
        &mut output,
        map.relationships.iter().map(|edge| {
            format!(
                "`{}` → `{}` (`{}`; {})",
                edge.relationship.source, edge.relationship.target, edge.relationship.symbol, edge.id
            )
        }),
        map.relationships.len(),
        MAX_ITEMS,
    );
    section(&mut output, "Tests");
    list_limited(
        &mut output,
        map.tests
            .iter()
            .map(|test| format!("`{}` — {}", test.landmark.path, test.landmark.reason)),
        map.tests.len(),
        MAX_ITEMS,
    );
    section(&mut output, "History");
    writeln!(
        output,
        "Analyzed {} reachable commit(s), including {} non-merge commit(s).",
        map.history.commits_seen, map.history.non_merge_commits_seen
    )
    .expect("writing to a string cannot fail");
    for observation in &map.history.observations {
        writeln!(output, "- {}", history_observation(observation)).expect("writing to a string cannot fail");
    }
    section(&mut output, "Quality and limitations");
    writeln!(output, "Quality: stale={}, truncated={}, resource_limited={}, incomplete={}, unsafe_paths={}, unsupported={}, partial={}", map.quality.stale, map.quality.truncated, map.quality.resource_limited, map.quality.incomplete, map.quality.unsafe_paths, map.quality.unsupported, map.quality.partial).expect("writing to a string cannot fail");
    if map.limitations.is_empty() {
        writeln!(output, "No additional limitations were recorded.").expect("writing to a string cannot fail");
    } else {
        for limitation in &map.limitations {
            writeln!(output, "- {limitation}").expect("writing to a string cannot fail");
        }
    }
    writeln!(output, "\n`map.json` contains the full portable snapshot. Its snapshot identifier must match this file before using the pair.").expect("writing to a string cannot fail");
    output
}

fn history_observation(observation: &dalil_core::HistoryObservation) -> String {
    match observation {
        dalil_core::HistoryObservation::Churn { paths, window_days, .. } => {
            format!("{} churn path(s) in the last {window_days} day(s)", paths.len())
        }
        dalil_core::HistoryObservation::Contributors { contributor, total_commits, .. } => {
            format!(
                "{} authored {} of {total_commits} observed commit(s)",
                contributor.name, contributor.commits
            )
        }
        dalil_core::HistoryObservation::BugOverlap { paths, bug_commits, .. } => {
            format!("{bug_commits} bug-keyword commit(s) overlap {} path(s)", paths.len())
        }
        dalil_core::HistoryObservation::Activity { month, commits, .. } => {
            format!("{month}: {commits} observed commit(s)")
        }
        dalil_core::HistoryObservation::Firefighting { commits, paths, .. } => {
            format!(
                "{commits} firefighting-language commit(s) touched {} path(s)",
                paths.len()
            )
        }
    }
}

fn section(output: &mut String, title: &str) {
    writeln!(output, "\n## {title}\n").expect("writing to a string cannot fail");
}

fn list_limited(output: &mut String, entries: impl Iterator<Item = String>, total: usize, limit: usize) {
    for entry in entries.take(limit) {
        writeln!(output, "- {entry}").expect("writing to a string cannot fail");
    }
    if total > limit {
        writeln!(output, "- … {} additional item(s) in `map.json`", total - limit)
            .expect("writing to a string cannot fail");
    }
}

fn set_private_directory(_path: &Path) -> io::Result<()> {
    // Unix applies this through the no-follow directory descriptor before each
    // publication. Other platforms use their supported default ACL behavior.
    Ok(())
}

fn is_reparse_or_symlink(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::{FileTypeExt, MetadataExt};
        return metadata.file_type().is_symlink()
            || metadata.file_attributes() & 0x400 != 0
            || metadata.file_type().is_symlink_dir()
            || metadata.file_type().is_symlink_file();
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
