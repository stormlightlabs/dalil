use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use gix::bstr::ByteSlice;

use super::*;

const MAX_CHANGED_PATHS: usize = 4_096;
const MAX_CHANGED_TREE_NODES: usize = 100_000;

#[derive(Clone, Copy)]
struct TreeFile {
    id: gix::ObjectId,
}

/// Resolve local revision and worktree inputs without invoking Git, hooks,
/// filters, repository programs, or network transports.
pub fn resolve_changes(path: &Path, context: &ContextRevisionContext) -> Result<ChangeResolution> {
    if context.base.is_none() && context.head.is_none() && context.range.is_none() && !context.dirty_worktree {
        return Ok(ChangeResolution::default());
    }

    let selected_path = absolute_path(path)?;
    let repository = security::discover_repository(&selected_path)
        .map_err(|source| MapError::Discovery { path: selected_path.clone(), source })?;
    let scope = security::resolve_scope(&repository, &selected_path).map_err(|error| match error {
        security::ScopeError::Input(reason) => MapError::Input { path: selected_path.clone(), reason },
        security::ScopeError::Safety(error) => MapError::safety("resolving the change-analysis scope", error),
    })?;

    let mut resolution = ChangeResolution::default();
    if context.base.is_some() || context.head.is_some() || context.range.is_some() {
        let Some((base_spec, head_spec)) = revision_endpoints(context, &mut resolution) else {
            resolution.status = ChangeResolutionStatus::Unresolved;
            return Ok(resolution);
        };
        let Some(base) = resolve_commit(&repository, &base_spec, "base", &mut resolution) else {
            resolution.status = ChangeResolutionStatus::Unresolved;
            return Ok(resolution);
        };
        let Some(head) = resolve_commit(&repository, &head_spec, "head", &mut resolution) else {
            resolution.status = ChangeResolutionStatus::Unresolved;
            return Ok(resolution);
        };
        resolution.base = Some(base.id.to_string());
        resolution.head = Some(head.id.to_string());
        let base_tree = match base.tree() {
            Ok(tree) => tree.id,
            Err(error) => {
                resolution.uncertainty.push(change_uncertainty(
                    "missing_object",
                    format!("Could not read the base tree: {error}."),
                ));
                resolution.status = ChangeResolutionStatus::Partial;
                return Ok(resolution);
            }
        };
        let head_tree = match head.tree() {
            Ok(tree) => tree.id,
            Err(error) => {
                resolution.uncertainty.push(change_uncertainty(
                    "missing_object",
                    format!("Could not read the head tree: {error}."),
                ));
                resolution.status = ChangeResolutionStatus::Partial;
                return Ok(resolution);
            }
        };
        let before = collect_tree_files(&repository, base_tree, &scope.relative_path, &mut resolution);
        let after = collect_tree_files(&repository, head_tree, &scope.relative_path, &mut resolution);
        let changes = compare_trees(&repository, before, after, &mut resolution);
        resolution.changes.extend(changes);
    }

    if context.dirty_worktree {
        let changes = resolve_dirty_worktree(&repository, &scope, &mut resolution);
        resolution.changes.extend(changes);
    }
    resolution.changes.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.previous_path.cmp(&right.previous_path))
            .then_with(|| change_kind_key(left.kind).cmp(&change_kind_key(right.kind)))
    });
    resolution.changes.dedup_by(|left, right| {
        left.kind == right.kind && left.path == right.path && left.previous_path == right.previous_path
    });
    if resolution.changes.len() > MAX_CHANGED_PATHS {
        resolution.changes.truncate(MAX_CHANGED_PATHS);
        resolution.uncertainty.push(change_uncertainty(
            "truncated",
            format!("Change resolution reached the {MAX_CHANGED_PATHS}-path limit."),
        ));
    }
    resolution.status = if resolution.uncertainty.is_empty() {
        ChangeResolutionStatus::Resolved
    } else {
        ChangeResolutionStatus::Partial
    };
    Ok(resolution)
}

/// Add changed-symbol evidence from the bounded source map. Deleted files have
/// no current source to parse, so the resolver reports that limitation instead.
pub fn enrich_change_symbols(resolution: &mut ChangeResolution, map: &MapReport) {
    if resolution.status == ChangeResolutionStatus::NotRequested {
        return;
    }
    let sources = &map.reading_evidence.sources;
    for change in &mut resolution.changes {
        if change.kind == ChangeKind::Deleted {
            resolution.uncertainty.push(change_uncertainty(
                "deleted_source",
                format!(
                    "`{}` was deleted, so no current syntax symbols were available.",
                    change.path
                ),
            ));
            continue;
        }
        let Some(source) = sources.iter().find(|source| source.path == change.path) else {
            if !map.omissions.iter().any(|omission| omission.path == change.path) {
                resolution.uncertainty.push(change_uncertainty(
                    "unsupported_or_unavailable",
                    format!(
                        "`{}` was changed but was not available to the bounded source parser.",
                        change.path
                    ),
                ));
            }
            continue;
        };
        if change.changed_lines.is_empty() {
            continue;
        }
        change.symbols = source
            .symbols
            .iter()
            .filter(|symbol| line_ranges_overlap(&symbol.location, &change.changed_lines))
            .map(|symbol| ChangedSymbol {
                name: symbol.name.clone(),
                kind: symbol.kind,
                role: symbol.role,
                location: symbol.location.clone(),
            })
            .collect();
    }
    if !resolution.uncertainty.is_empty() && resolution.status == ChangeResolutionStatus::Resolved {
        resolution.status = ChangeResolutionStatus::Partial;
    }
}

fn revision_endpoints(context: &ContextRevisionContext, resolution: &mut ChangeResolution) -> Option<(String, String)> {
    if let Some(range) = &context.range {
        let Some((base, head)) = range.split_once("..") else {
            resolution.uncertainty.push(change_uncertainty(
                "invalid_range",
                "`--revision-range` must use exactly one `base..head` separator.",
            ));
            return None;
        };
        if base.is_empty() || head.is_empty() || head.contains("..") {
            resolution.uncertainty.push(change_uncertainty(
                "invalid_range",
                "`--revision-range` must name non-empty local base and head revisions.",
            ));
            return None;
        }
        return Some((base.to_owned(), head.to_owned()));
    }
    Some((
        context.base.clone().unwrap_or_else(|| "HEAD".to_owned()),
        context.head.clone().unwrap_or_else(|| "HEAD".to_owned()),
    ))
}

fn resolve_commit<'repo>(
    repository: &'repo gix::Repository, revision: &str, role: &str, resolution: &mut ChangeResolution,
) -> Option<gix::Commit<'repo>> {
    let id = match repository.rev_parse_single(revision) {
        Ok(id) => id,
        Err(error) => {
            resolution.uncertainty.push(change_uncertainty(
                "unresolved_revision",
                format!("Could not resolve the {role} revision `{revision}` locally: {error}."),
            ));
            return None;
        }
    };
    let commit = match id.object() {
        Ok(object) => object.peel_to_commit(),
        Err(error) => {
            resolution.uncertainty.push(change_uncertainty(
                "missing_object",
                format!("Could not read the {role} revision `{revision}`: {error}."),
            ));
            return None;
        }
    };
    match commit {
        Ok(commit) => Some(commit),
        Err(error) => {
            resolution.uncertainty.push(change_uncertainty(
                "non_commit_revision",
                format!("The {role} revision `{revision}` does not resolve to a commit: {error}."),
            ));
            None
        }
    }
}

fn collect_tree_files(
    repository: &gix::Repository, tree_id: gix::ObjectId, scope: &str, resolution: &mut ChangeResolution,
) -> BTreeMap<String, TreeFile> {
    let mut files = BTreeMap::new();
    let mut stack = vec![(tree_id, Vec::new(), 0usize)];
    let mut nodes = 0usize;
    while let Some((tree_id, prefix, depth)) = stack.pop() {
        nodes = nodes.saturating_add(1);
        if nodes > MAX_CHANGED_TREE_NODES || depth > ReportLimits::default().max_syntax_depth {
            resolution.uncertainty.push(change_uncertainty(
                "truncated",
                "Revision tree traversal reached its bounded node or depth limit.",
            ));
            break;
        }
        let tree = match repository.find_tree(tree_id) {
            Ok(tree) => tree,
            Err(error) => {
                resolution.uncertainty.push(change_uncertainty(
                    "missing_object",
                    format!("Could not read a revision tree: {error}."),
                ));
                break;
            }
        };
        for entry in tree.iter() {
            let Ok(entry) = entry else {
                resolution.uncertainty.push(change_uncertainty(
                    "partial_tree",
                    "Could not decode a revision tree entry.",
                ));
                continue;
            };
            let mut path = prefix.clone();
            if !path.is_empty() {
                path.push(b'/');
            }
            path.extend_from_slice(entry.filename().as_bytes());
            let path = match security::validate_repository_path(&path) {
                Ok(path) => path,
                Err(error) => {
                    resolution.uncertainty.push(change_uncertainty(
                        "unsafe_path",
                        format!("Ignored an unsafe revision-tree path: {error}."),
                    ));
                    continue;
                }
            };
            if entry.mode().is_tree() {
                stack.push((entry.id().detach(), path.into_bytes(), depth.saturating_add(1)));
            } else if !entry.mode().is_commit() && in_change_scope(&path, scope) {
                files.insert(path, TreeFile { id: entry.id().detach() });
            }
        }
    }
    files
}

fn compare_trees(
    repository: &gix::Repository, before: BTreeMap<String, TreeFile>, after: BTreeMap<String, TreeFile>,
    resolution: &mut ChangeResolution,
) -> Vec<ResolvedChange> {
    let mut changes = Vec::new();
    let mut deleted = Vec::new();
    let mut added = Vec::new();
    let paths = before.keys().chain(after.keys()).cloned().collect::<BTreeSet<_>>();
    for path in paths {
        match (before.get(&path), after.get(&path)) {
            (Some(previous), Some(current)) if previous.id != current.id => changes.push(change_with_lines(
                ChangeKind::Modified,
                path,
                None,
                blob_data(repository, previous.id, resolution),
                blob_data(repository, current.id, resolution),
            )),
            (Some(previous), None) => deleted.push((path, *previous)),
            (None, Some(current)) => added.push((path, *current)),
            _ => {}
        }
    }
    let mut consumed_added = BTreeSet::new();
    for (previous_path, previous) in deleted {
        if let Some((index, (path, current))) = added
            .iter()
            .enumerate()
            .find(|(index, (_, current))| !consumed_added.contains(index) && current.id == previous.id)
        {
            consumed_added.insert(index);
            changes.push(change_with_lines(
                ChangeKind::Renamed,
                path.clone(),
                Some(previous_path),
                blob_data(repository, previous.id, resolution),
                blob_data(repository, current.id, resolution),
            ));
        } else {
            changes.push(change_with_lines(
                ChangeKind::Deleted,
                previous_path,
                None,
                blob_data(repository, previous.id, resolution),
                None,
            ));
        }
    }
    for (index, (path, current)) in added.into_iter().enumerate() {
        if !consumed_added.contains(&index) {
            changes.push(change_with_lines(
                ChangeKind::Added,
                path,
                None,
                None,
                blob_data(repository, current.id, resolution),
            ));
        }
    }
    changes
}

fn resolve_dirty_worktree(
    repository: &gix::Repository, scope: &security::RepositoryScope, resolution: &mut ChangeResolution,
) -> Vec<ResolvedChange> {
    let index = match repository.index_or_load_from_head_or_empty() {
        Ok(index) => index,
        Err(error) => {
            resolution.uncertainty.push(change_uncertainty(
                "index_unavailable",
                format!("Could not read the worktree index: {error}."),
            ));
            return Vec::new();
        }
    };
    let mut indexed = BTreeMap::new();
    for (path, id) in index.entries_with_paths_by_filter_map(|_, entry| Some(entry.id)) {
        match security::validate_repository_path(path.as_bytes()) {
            Ok(path) if in_change_scope(&path, &scope.relative_path) => {
                indexed.insert(path, id);
            }
            Ok(_) => {}
            Err(error) => resolution.uncertainty.push(change_uncertainty(
                "unsafe_path",
                format!("Ignored an unsafe index path: {error}."),
            )),
        }
    }
    let (visible, issues, _) = walk_files(
        &scope.selected_path,
        &scope.repository_root,
        false,
        MAX_CHANGED_PATHS,
        false,
        true,
        &[],
    );
    for issue in issues {
        let detail = match issue {
            WalkIssue::Traversal(detail) | WalkIssue::Safety(detail) => detail,
        };
        resolution
            .uncertainty
            .push(change_uncertainty("worktree_traversal", detail));
    }
    let mut changes = Vec::new();
    for (path, id) in &indexed {
        let previous = blob_data(repository, *id, resolution);
        match security::read_worktree_file_limited(
            &scope.repository_root,
            &scope.selected_path,
            path,
            ReportLimits::default().max_file_bytes,
        ) {
            Ok(current) if previous.as_deref() != Some(current.as_slice()) => changes.push(change_with_lines(
                ChangeKind::Modified,
                path.to_owned(),
                None,
                previous,
                Some(current),
            )),
            Ok(_) => {}
            Err(security::ReadError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => changes.push(
                change_with_lines(ChangeKind::Deleted, path.to_owned(), None, previous, None),
            ),
            Err(error) => resolution.uncertainty.push(change_uncertainty(
                "worktree_read",
                format!("Could not read `{path}` from the worktree: {error}."),
            )),
        }
    }
    for (path, symlink) in visible {
        if indexed.contains_key(&path) || path == ".git" || path.starts_with(".git/") {
            continue;
        }
        if symlink {
            resolution.uncertainty.push(change_uncertainty(
                "unsafe_path",
                format!("Ignored untracked symlink `{path}`."),
            ));
            continue;
        }
        match security::read_worktree_file_limited(
            &scope.repository_root,
            &scope.selected_path,
            &path,
            ReportLimits::default().max_file_bytes,
        ) {
            Ok(current) => changes.push(change_with_lines(
                ChangeKind::Untracked,
                path,
                None,
                None,
                Some(current),
            )),
            Err(error) => resolution.uncertainty.push(change_uncertainty(
                "worktree_read",
                format!("Could not read untracked path `{path}`: {error}."),
            )),
        }
    }
    changes
}

fn blob_data(repository: &gix::Repository, id: gix::ObjectId, resolution: &mut ChangeResolution) -> Option<Vec<u8>> {
    match repository.find_blob(id) {
        Ok(blob) => Some(blob.data.to_vec()),
        Err(error) => {
            resolution.uncertainty.push(change_uncertainty(
                "missing_object",
                format!("Could not read a changed blob: {error}."),
            ));
            None
        }
    }
}

fn change_with_lines(
    kind: ChangeKind, path: String, previous_path: Option<String>, before: Option<Vec<u8>>, after: Option<Vec<u8>>,
) -> ResolvedChange {
    let changed_lines = changed_lines(before.as_deref(), after.as_deref());
    ResolvedChange { kind, path, previous_path, changed_lines, symbols: Vec::new() }
}

fn changed_lines(before: Option<&[u8]>, after: Option<&[u8]>) -> Vec<ChangedLineRange> {
    let Some(after) = after else {
        return Vec::new();
    };
    let Ok(after) = std::str::from_utf8(after) else {
        return Vec::new();
    };
    let after = after.lines().collect::<Vec<_>>();
    let before = before
        .and_then(|before| std::str::from_utf8(before).ok())
        .map(|before| before.lines().collect::<Vec<_>>())
        .unwrap_or_default();
    let mut prefix = 0usize;
    while prefix < before.len() && prefix < after.len() && before[prefix] == after[prefix] {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < before.len().saturating_sub(prefix)
        && suffix < after.len().saturating_sub(prefix)
        && before[before.len() - 1 - suffix] == after[after.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let start = prefix.saturating_add(1);
    let end = after.len().saturating_sub(suffix).max(start);
    vec![ChangedLineRange { start, end }]
}

fn line_ranges_overlap(location: &SourceLocation, ranges: &[ChangedLineRange]) -> bool {
    ranges
        .iter()
        .any(|range| location.start.line <= range.end && location.end.line >= range.start)
}

fn in_change_scope(path: &str, scope: &str) -> bool {
    scope == "." || path == scope || path.starts_with(&format!("{scope}/"))
}

fn change_uncertainty(kind: impl Into<String>, detail: impl Into<String>) -> ChangeUncertainty {
    ChangeUncertainty { kind: kind.into(), detail: detail.into() }
}

fn change_kind_key(kind: ChangeKind) -> u8 {
    match kind {
        ChangeKind::Added => 0,
        ChangeKind::Deleted => 1,
        ChangeKind::Modified => 2,
        ChangeKind::Renamed => 3,
        ChangeKind::Untracked => 4,
    }
}
