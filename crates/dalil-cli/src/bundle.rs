use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use dalil_core::{
    EvidenceMap, LandmarkKind, OmissionReason, OrientationReport, SymbolKind, SymbolRole, SymbolVisibility,
};
use sha2::{Digest, Sha256};

use crate::render::Render;

static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);
const MAX_TASK_SLUG_CHARS: usize = 40;
const MAX_TASK_COLLISION_ATTEMPTS: u32 = 64;

#[derive(Debug, thiserror::Error)]
pub(crate) enum BundleError {
    #[error("the repository root is not a directory")]
    RepositoryRoot,
    #[error("refusing to write through symlink or reparse-point path `{0}`")]
    UnsafePath(PathBuf),
    #[error("`.dalil` exists but is not a directory")]
    DestinationCollision,
    #[error("`.dalil/{0}` exists but is not a regular file")]
    FileCollision(&'static str),
    #[error("could not reserve a unique task record filename after repeated collisions")]
    TaskFilename,
    #[error("could not publish the repository bundle: {0}")]
    Io(#[from] io::Error),
    #[error("could not serialize the repository evidence map: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) struct PublishedBundle {
    pub(crate) directory: PathBuf,
    pub(crate) snapshot_id: String,
}

pub(crate) struct PublishedReview {
    pub(crate) directory: PathBuf,
}

pub(crate) struct PublishedTask {
    pub(crate) filename: String,
    pub(crate) task_id: String,
    pub(crate) created_at: String,
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

pub(crate) fn publish_review(map: &EvidenceMap) -> Result<PublishedReview, BundleError> {
    let root = PathBuf::from(&map.repository.canonical_root);
    let directory = private_bundle_directory(&root)?;
    let review = render_review(map);
    atomic_write(&directory, "review.md", review.as_bytes())?;
    Ok(PublishedReview { directory })
}

pub(crate) fn review_is_current(map: &EvidenceMap) -> Result<bool, BundleError> {
    let root = PathBuf::from(&map.repository.canonical_root);
    let Some(directory) = existing_bundle_directory(&root)? else {
        return Ok(false);
    };
    let path = directory.join("review.md");
    match fs::symlink_metadata(&path) {
        Ok(metadata) if is_reparse_or_symlink(&metadata) => return Err(BundleError::UnsafePath(path)),
        Ok(metadata) if !metadata.is_file() => return Err(BundleError::FileCollision("review.md")),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    }
    Ok(fs::read(path)? == render_review(map).as_bytes())
}

/// Append one task record under `.dalil/tasks/` for an explicit task export.
/// The map snapshot and its orientation must already be complete before this
/// function is called. The record is created with exclusive file creation so an
/// earlier record is never overwritten.
pub(crate) fn publish_task(
    map: &EvidenceMap, task: &str, orientation: &OrientationReport,
) -> Result<PublishedTask, BundleError> {
    let root = PathBuf::from(&map.repository.canonical_root);
    let bundle = private_bundle_directory(&root)?;
    let directory = task_records_directory(&bundle)?;
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let task_id = task_id(task);
    let record = render_task_record(map, task, orientation, &task_id, &utc_rfc3339(seconds));
    let base = format!("{}-{}-{}", utc_compact_timestamp(seconds), task_slug(task), task_id);
    let mut name = format!("{base}.md");
    let mut attempt = 0u32;
    loop {
        match create_new_private_platform(&directory, &name, record.as_bytes()) {
            Ok(()) => break,
            Err(BundleError::Io(error)) if error.kind() == io::ErrorKind::AlreadyExists => {
                attempt += 1;
                if attempt >= MAX_TASK_COLLISION_ATTEMPTS {
                    return Err(BundleError::TaskFilename);
                }
                name = format!("{base}-{}.md", attempt + 1);
            }
            Err(error) => return Err(error),
        }
    }
    Ok(PublishedTask { filename: name, task_id, created_at: utc_rfc3339(seconds) })
}

fn task_records_directory(bundle: &Path) -> Result<PathBuf, BundleError> {
    let directory = bundle.join("tasks");
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if is_reparse_or_symlink(&metadata) => return Err(BundleError::UnsafePath(directory)),
        Ok(metadata) if !metadata.is_dir() => return Err(BundleError::FileCollision("tasks")),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(&directory)?,
        Err(error) => return Err(error.into()),
    }
    Ok(directory)
}

/// Derive a stable, content-derived task identifier from the exact task text.
fn task_id(task: &str) -> String {
    let digest = Sha256::digest(task.as_bytes());
    let mut id = String::with_capacity(10);
    for byte in digest.iter().take(5) {
        write!(id, "{byte:02x}").expect("writing to a string cannot fail");
    }
    id
}

/// Project arbitrary task text into a filesystem-safe filename slug.
fn task_slug(task: &str) -> String {
    let mut slug = String::new();
    for character in task.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug = slug.trim_matches('-').to_owned();
    if slug.len() > MAX_TASK_SLUG_CHARS {
        slug.truncate(MAX_TASK_SLUG_CHARS);
        while slug.ends_with('-') {
            slug.pop();
        }
    }
    if slug.is_empty() {
        slug.push_str("task");
    }
    slug
}

/// Select a fenced code block marker longer than any backtick run in the task,
/// so the original task text is preserved exactly inside one valid fence.
fn task_fence(task: &str) -> String {
    let mut longest_run = 0usize;
    let mut current_run = 0usize;
    for character in task.chars() {
        if character == '`' {
            current_run += 1;
            longest_run = longest_run.max(current_run);
        } else {
            current_run = 0;
        }
    }
    "`".repeat((longest_run + 1).max(3))
}

fn render_task_record(
    map: &EvidenceMap, task: &str, orientation: &OrientationReport, task_id: &str, created_at: &str,
) -> String {
    let fence = task_fence(task);
    let mut output = String::new();
    writeln!(output, "# Dalil task record").expect("writing to a string cannot fail");
    writeln!(output).expect("writing to a string cannot fail");
    writeln!(
        output,
        "Generated by `dalil export --task`. Task records are repository files and may contain sensitive task input."
    )
    .expect("writing to a string cannot fail");
    writeln!(output).expect("writing to a string cannot fail");
    writeln!(output, "- Task ID: `{task_id}`").expect("writing to a string cannot fail");
    writeln!(output, "- Created (UTC): `{created_at}`").expect("writing to a string cannot fail");
    writeln!(output, "- Dalil version: {}", map.producer_version).expect("writing to a string cannot fail");
    writeln!(output, "- Map snapshot: `{}`", map.snapshot_id).expect("writing to a string cannot fail");
    writeln!(
        output,
        "- Repository: `{}`",
        crate::utils::escape_inline_code(&map.repository.canonical_root)
    )
    .expect("writing to a string cannot fail");
    writeln!(output, "- Scope: `{}`", crate::utils::escape_inline_code(&map.scope))
        .expect("writing to a string cannot fail");
    writeln!(
        output,
        "- Revision: `{}`",
        map.revision.oid.as_deref().unwrap_or("unborn or unavailable")
    )
    .expect("writing to a string cannot fail");
    writeln!(output, "- Worktree fingerprint: `{}`", map.worktree_fingerprint)
        .expect("writing to a string cannot fail");

    section(&mut output, "Task");
    writeln!(output, "{fence}").expect("writing to a string cannot fail");
    output.push_str(task);
    if !task.ends_with('\n') {
        output.push('\n');
    }
    writeln!(output, "{fence}").expect("writing to a string cannot fail");

    section(&mut output, "Orientation");
    Render::orientation_markdown(&mut output, orientation);

    section(&mut output, "Quality");
    writeln!(
        output,
        "- stale={}, truncated={}, resource_limited={}, incomplete={}, unsafe_paths={}, unsupported={}, partial={}",
        map.quality.stale,
        map.quality.truncated,
        map.quality.resource_limited,
        map.quality.incomplete,
        map.quality.unsafe_paths,
        map.quality.unsupported,
        map.quality.partial
    )
    .expect("writing to a string cannot fail");
    if map.quality.strict_issues.is_empty() {
        writeln!(output, "- No strict issues were recorded.").expect("writing to a string cannot fail");
    } else {
        let issues = map
            .quality
            .strict_issues
            .iter()
            .map(|issue| issue.label())
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "- Strict issues: {issues}").expect("writing to a string cannot fail");
    }

    section(&mut output, "Limitations");
    if map.limitations.is_empty() {
        writeln!(output, "No additional limitations were recorded.").expect("writing to a string cannot fail");
    } else {
        for limitation in &map.limitations {
            writeln!(output, "- {limitation}").expect("writing to a string cannot fail");
        }
    }

    writeln!(output).expect("writing to a string cannot fail");
    writeln!(
        output,
        "This record matches the `.dalil/map.json` snapshot above; compare its revision and worktree fingerprint before reuse."
    )
    .expect("writing to a string cannot fail");
    output
}

fn utc_compact_timestamp(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_date_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = seconds_of_day.rem_euclid(3_600) / 60;
    let second = seconds_of_day.rem_euclid(60);
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}

fn utc_rfc3339(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_date_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = seconds_of_day.rem_euclid(3_600) / 60;
    let second = seconds_of_day.rem_euclid(60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Gregorian calendar conversion based on the civil-from-days algorithm.
fn civil_date_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year, month, day)
}

fn create_new_private_platform(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), BundleError> {
    #[cfg(unix)]
    {
        create_new_private_unix(directory, name, bytes).map_err(BundleError::Io)
    }
    #[cfg(not(unix))]
    {
        let destination = directory.join(name);
        let mut file = OpenOptions::new().write(true).create_new(true).open(&destination)?;
        let result = (|| -> io::Result<()> {
            file.write_all(bytes)?;
            file.sync_all()
        })();
        if result.is_err() {
            drop(file);
            let _ = fs::remove_file(&destination);
        }
        result.map_err(BundleError::Io)
    }
}

#[cfg(unix)]
fn create_new_private_unix(directory: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
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
    let name_c =
        CString::new(name.as_bytes()).map_err(|_| io::Error::other("task record filename contains a NUL byte"))?;
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name_c.as_ptr(),
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
        if unsafe { libc::fsync(directory.as_raw_fd()) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    })();
    if result.is_err() {
        unsafe {
            libc::unlinkat(directory.as_raw_fd(), name_c.as_ptr(), 0);
        }
    }
    result
}

fn private_bundle_directory(root: &Path) -> Result<PathBuf, BundleError> {
    validate_repository_root(root)?;
    let directory = root.join(".dalil");
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if is_reparse_or_symlink(&metadata) => return Err(BundleError::UnsafePath(directory)),
        Ok(metadata) if !metadata.is_dir() => return Err(BundleError::DestinationCollision),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(&directory)?,
        Err(error) => return Err(error.into()),
    }
    validate_bundle_directory(&directory)?;
    set_private_directory(&directory)?;
    Ok(directory)
}

fn existing_bundle_directory(root: &Path) -> Result<Option<PathBuf>, BundleError> {
    validate_repository_root(root)?;
    let directory = root.join(".dalil");
    match fs::symlink_metadata(&directory) {
        Ok(_) => {
            validate_bundle_directory(&directory)?;
            Ok(Some(directory))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn validate_repository_root(root: &Path) -> Result<(), BundleError> {
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() || is_reparse_or_symlink(&metadata) {
        return Err(BundleError::RepositoryRoot);
    }
    Ok(())
}

fn validate_bundle_directory(directory: &Path) -> Result<(), BundleError> {
    let metadata = fs::symlink_metadata(directory)?;
    if is_reparse_or_symlink(&metadata) {
        return Err(BundleError::UnsafePath(directory.to_owned()));
    }
    if !metadata.is_dir() {
        return Err(BundleError::DestinationCollision);
    }
    Ok(())
}

fn atomic_write(directory: &Path, name: &'static str, bytes: &[u8]) -> Result<(), BundleError> {
    let destination = directory.join(name);
    match fs::symlink_metadata(&destination) {
        Ok(metadata) if is_reparse_or_symlink(&metadata) => return Err(BundleError::UnsafePath(destination)),
        Ok(metadata) if !metadata.is_file() => return Err(BundleError::FileCollision(name)),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    atomic_write_platform(directory, name, bytes)
}

#[cfg(unix)]
fn atomic_write_platform(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), BundleError> {
    atomic_write_unix(directory, name, bytes).map_err(BundleError::Io)
}

#[cfg(not(unix))]
fn atomic_write_platform(directory: &Path, name: &str, bytes: &[u8]) -> Result<(), BundleError> {
    let destination = directory.join(name);
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

fn render_review(map: &EvidenceMap) -> String {
    const MAX_LINES: usize = 2_000;
    const MAX_BYTES: usize = 200 * 1024;
    const MAX_OMISSION_LINE_BYTES: usize = 160;

    let mut sections = vec![
        ReviewSection::new("Project roots", project_root_facts(map)),
        ReviewSection::new("Public symbols", public_symbol_facts(map)),
        ReviewSection::new("Library exports", library_export_facts(map)),
        ReviewSection::new("Cross-project dependencies", cross_project_dependency_facts(map)),
        ReviewSection::new("Runtime entry points", runtime_entry_point_facts(map)),
        ReviewSection::new("Test roots", test_root_facts(map)),
        ReviewSection::new("Coverage and omissions", coverage_facts(map)),
    ];
    for section in &mut sections {
        section.facts.sort();
        section.facts.dedup();
    }

    let mut output = String::from(
        "# Dalil repository review snapshot\n\n<!-- Generated by `dalil export --review`; do not edit. -->\n",
    );
    let mut lines = output.lines().count();
    for section in &sections {
        writeln!(output, "\n## {}", section.title).expect("writing to a string cannot fail");
        writeln!(output, "- Facts: {}", section.facts.len()).expect("writing to a string cannot fail");
        lines += 3;
    }

    let mut omitted = vec![0usize; sections.len()];
    for (index, section) in sections.iter().enumerate() {
        let marker = format!("\n## {}\n- Facts: {}\n", section.title, section.facts.len());
        let position = output.find(&marker).expect("review section header was rendered") + marker.len();
        let mut insertion = String::new();
        for fact in &section.facts {
            let line = format!("{fact}\n");
            let reserved_lines = sections.len();
            let reserved_bytes = sections.len() * MAX_OMISSION_LINE_BYTES;
            if lines + 1 + reserved_lines > MAX_LINES
                || output.len() + insertion.len() + line.len() + reserved_bytes > MAX_BYTES
            {
                omitted[index] += 1;
                continue;
            }
            insertion.push_str(&line);
            lines += 1;
        }
        output.insert_str(position, &insertion);
    }

    for (section, count) in sections.iter().zip(omitted) {
        if count > 0 {
            writeln!(
                output,
                "- Omitted {count} fact(s) from {} due to the review snapshot limit.",
                section.title
            )
            .expect("writing to a string cannot fail");
        }
    }
    output
}

struct ReviewSection {
    title: &'static str,
    facts: Vec<String>,
}

impl ReviewSection {
    fn new(title: &'static str, facts: Vec<String>) -> Self {
        Self { title, facts }
    }
}

fn project_root_facts(map: &EvidenceMap) -> Vec<String> {
    map.projects
        .iter()
        .map(|project| {
            format!(
                "- Project root `{}` ({})",
                inline(&project.project.path),
                project.project.kind.label()
            )
        })
        .collect()
}

fn public_symbol_facts(map: &EvidenceMap) -> Vec<String> {
    map.symbols
        .iter()
        .filter(|symbol| {
            symbol.symbol.role == SymbolRole::Definition
                && (symbol.symbol.visibility == SymbolVisibility::Public || symbol.symbol.kind == SymbolKind::Export)
        })
        .map(|symbol| {
            let qualified_name = if symbol.symbol.scope.is_empty() {
                symbol.symbol.name.clone()
            } else {
                format!("{}::{}", symbol.symbol.scope.join("::"), symbol.symbol.name)
            };
            let visibility = if symbol.symbol.kind == SymbolKind::Export { "exported" } else { "public" };
            format!(
                "- {visibility} {} `{}` in `{}`",
                symbol.symbol.kind.label(),
                inline(&qualified_name),
                inline(&symbol.path)
            )
        })
        .collect()
}

fn library_export_facts(map: &EvidenceMap) -> Vec<String> {
    manifest_target_facts(map, "library export", |metadata| &metadata.library_exports)
}

fn runtime_entry_point_facts(map: &EvidenceMap) -> Vec<String> {
    manifest_target_facts(map, "runtime entry point", |metadata| &metadata.runtime_entry_points)
}

fn manifest_target_facts(
    map: &EvidenceMap, label: &str, targets: impl Fn(&dalil_core::ManifestMetadata) -> &[dalil_core::ManifestTarget],
) -> Vec<String> {
    let mut facts = Vec::new();
    for project in &map.projects {
        for metadata in &project.project.manifest_metadata {
            for target in targets(metadata) {
                let target_name = target
                    .resolved_path
                    .as_deref()
                    .filter(|path| is_relative_path(path))
                    .or_else(|| target.name.as_deref().filter(|name| is_relative_path(name)))
                    .or_else(|| is_relative_path(&target.declared).then_some(target.declared.as_str()))
                    .unwrap_or("unresolved target");
                facts.push(format!(
                    "- {label} `{}`: `{}`",
                    inline(&project.project.path),
                    inline(target_name)
                ));
            }
        }
    }
    facts
}

fn cross_project_dependency_facts(map: &EvidenceMap) -> Vec<String> {
    let roots = map
        .projects
        .iter()
        .map(|project| project.project.path.as_str())
        .collect::<Vec<_>>();
    let mut dependencies = BTreeMap::<(String, String), usize>::new();
    for relationship in &map.relationships {
        if relationship.relationship.ambiguous {
            continue;
        }
        let Some(source) = project_root_for_path(&relationship.relationship.source, &roots) else {
            continue;
        };
        let Some(target) = project_root_for_path(&relationship.relationship.target, &roots) else {
            continue;
        };
        if source != target {
            *dependencies.entry((source.to_owned(), target.to_owned())).or_default() += 1;
        }
    }
    dependencies
        .into_iter()
        .map(|((source, target), count)| {
            format!(
                "- Dependency `{}` -> `{}` ({count} resolved relationship(s))",
                inline(&source),
                inline(&target)
            )
        })
        .collect()
}

fn project_root_for_path<'a>(path: &str, roots: &[&'a str]) -> Option<&'a str> {
    roots
        .iter()
        .copied()
        .filter(|root| *root == "." || path == *root || path.starts_with(&format!("{root}/")))
        .max_by_key(|root| root.len())
}

fn test_root_facts(map: &EvidenceMap) -> Vec<String> {
    map.tests
        .iter()
        .filter(|test| test.landmark.kind == LandmarkKind::TestRoot)
        .map(|test| format!("- Test root `{}`", inline(&test.landmark.path)))
        .collect()
}

fn coverage_facts(map: &EvidenceMap) -> Vec<String> {
    let mut languages = BTreeMap::<&str, usize>::new();
    let mut statuses = BTreeMap::<&str, usize>::new();
    let mut omissions = BTreeMap::<&str, usize>::new();
    for file in &map.files {
        *languages.entry(file.file.language.label()).or_default() += 1;
        *statuses.entry(file.file.status.label()).or_default() += 1;
    }
    for omission in &map.omissions {
        if omission.reason != OmissionReason::IgnoredUntracked {
            *omissions.entry(omission.reason.label()).or_default() += 1;
        }
    }

    let mut facts = Vec::new();
    facts.extend(
        languages
            .into_iter()
            .map(|(language, count)| format!("- Analyzed {count} {language} source file(s)")),
    );
    facts.extend(
        statuses
            .into_iter()
            .map(|(status, count)| format!("- {count} source file(s) with {status} analysis")),
    );
    facts.extend(
        omissions
            .into_iter()
            .map(|(reason, count)| format!("- Omitted {count} file(s): {reason}")),
    );
    facts
}

fn inline(value: &str) -> String {
    crate::utils::escape_inline_code(value)
}

fn is_relative_path(path: &str) -> bool {
    !path.starts_with('/') && !path.starts_with('\\') && path.as_bytes().get(1).is_none_or(|byte| *byte != b':')
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
