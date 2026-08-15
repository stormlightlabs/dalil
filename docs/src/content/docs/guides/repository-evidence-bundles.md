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

## Keep or share it

Treat the complete bundle as generated working data by default:

- Add `.dalil/` to `.gitignore` when the bundle is local context.
- Upload it as a CI artifact when another job or tool needs the full snapshot.
- Commit it only when the repository has a specific need for a versioned full
  map and accepts large diffs and merge conflicts.

Dalil's output is deterministic when its repository facts are unchanged, but
determinism does not make a large artifact easy to review. A committed map also
becomes stale after source changes. Dalil does not edit `.gitignore` or choose
the repository's storage policy. Prefer the complete bundle as local or
uploaded evidence rather than a required source-controlled file.

## Write boundary

`export` is the only command that writes to the analyzed repository. It refuses
a non-directory `.dalil` collision, symlink or reparse-point destinations, and
paths outside the worktree. Ordinary orientation, map, context, impact, search,
explain, history, capability, and doctor commands do not create a bundle.
