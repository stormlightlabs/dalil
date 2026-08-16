---
title: Repository evidence bundles
description: Export a portable repository map for inspection, sharing, or later agent work.
section: Guides
group: Guides
order: 6
---

Run `dalil export` from a Git worktree to create a repository-local evidence
bundle:

```sh
dalil export
dalil export src
```

The command writes two files under `.dalil/`:

```text
.dalil/
├── map.json
└── map.md
```

`map.json` is the complete portable evidence snapshot. It includes repository
identity, revision and worktree fingerprint, projects, files, symbols,
relationships, landmarks, tests, bounded history, quality, limitations,
provenance, and collection totals. Its schema is
[`schema/export/v1/map.json`](https://github.com/stormlightlabs/dalil/blob/main/schema/export/v1/map.json).

`map.md` is a shorter view of that same snapshot. It reports collection totals
and omissions, then points to `map.json` for items omitted for readability.
Both files carry one snapshot identifier. Check that identifier before using a
pair copied from an interrupted or conflicting refresh.

## Refresh a bundle

Run the same command again:

```sh
dalil export
```

Dalil analyzes the selected scope, writes complete private temporary files, and
replaces only `.dalil/map.json` and `.dalil/map.md`. It leaves other `.dalil/`
files alone. The export excludes `.dalil/` from its evidence so a previous map
does not become input to its refresh.

A repository change produces a new snapshot identifier. A map can therefore be
stale after a later checkout, merge, or local edit even when both map files
match. Compare the revision and worktree fingerprint with the worktree before
relying on an older bundle.

## Review snapshot for Git

Use the review snapshot when a repository wants a small generated file in Git:

```sh
dalil export --review
dalil export --review --check
```

`--review` writes only `.dalil/review.md`. It does not create or refresh
`map.json` or `map.md`. The snapshot has one fact per line for project roots,
public and exported symbols, library exports, cross-project dependencies,
runtime entry points, test roots, and grouped analysis or omission totals. It
excludes source locations, private symbols, timestamps, revisions, worktree
state, and individual references.

The file has a generated-file notice. Regenerate it after semantic changes;
do not resolve a merge conflict by editing it. Regenerate from the merged
worktree instead. `--check` writes nothing and exits with status 5 when
`review.md` is missing or stale. It is suitable for CI:

```sh
dalil export --review --check
```

The snapshot is capped at 2,000 lines and 200 KiB. Dalil records deterministic
omission counts when either limit applies.

To commit only the review snapshot, replace a broad `.dalil/` ignore rule with
these rules in `.gitignore`:

```gitignore
.dalil/*
!.dalil/review.md
```

## Keep or share the complete map

Treat `map.json` and `map.md` as generated working data by default. Add
`.dalil/` to `.gitignore` when the bundle is local context, or upload the files
as CI artifacts when another job needs the full snapshot. Commit the complete
map only when the repository accepts its larger diffs and merge conflicts.

Dalil does not edit `.gitignore` or choose the repository's storage policy.

## Write boundary

`export` is the only command that writes to the analyzed repository. It refuses
a non-directory `.dalil` collision, symlink or reparse-point destinations, and
paths outside the worktree. Ordinary orientation, map, context, impact, search,
explain, history, capability, and doctor commands do not create a bundle.
