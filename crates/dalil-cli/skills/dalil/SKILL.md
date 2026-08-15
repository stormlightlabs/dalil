---
name: dalil
description: Use Dalil to orient in an unfamiliar Git repository, find implementation anchors, gather task context, discover relevant tests, and inspect local change context before review.
---

# Dalil

Use Dalil before broad file searches when the repository is unfamiliar or the
next source read is unclear. Run commands from the target Git worktree. Dalil
reads the repository and writes its index only under the user's cache directory.

Start small. Read the recommended source and make a follow-up request with a
path, symbol, task, or changed path instead of requesting an exhaustive map.

## Orient and map

Start with the reading plan:

```sh
dalil orient --json
```

Read the paths in `orientation` before relying on the recommendation. Use a
map when the repository layout, supported-language symbols, or project roots
need closer inspection:

```sh
dalil map --budget 750 --json
```

## Find an implementation

When you know a concept but not its location, search for a small set of anchors:

```sh
dalil search 'cache invalidation' --json
dalil search --symbol CacheStore --json
```

Inspect an anchor directly, then explain it or compile task context:

```sh
dalil explain src/map/cache.rs --json
dalil context --task 'fix parser cache invalidation' --changed-path src/map/cache.rs --budget 750 --json
```

Use `context.relevant_tests` to find tests tied to the selected evidence. Use
`context.next_reads` when the first result does not answer the task. Add
`--teach` only when a source-grounded reading sequence would help:

```sh
dalil context --task 'understand cache invalidation' --teach --budget 750 --json
```

## Review local work

Before reviewing saved local edits, request impact context:

```sh
dalil impact --dirty-worktree --task 'review cache changes' --budget 750 --json
```

For a committed change, use a local revision range:

```sh
dalil impact --revision-range 'HEAD~1..HEAD' --json
```

Treat relationships as lexical, structural, manifest, or history evidence to
inspect. They do not prove runtime behavior or predict breakage.

## Read the limits

Check `quality`, `limitations`, `omissions`, `uncertainty`, collection totals,
and budget fields in JSON before acting. Unsupported or malformed source,
resource limits, incomplete history, ambiguous lexical references, and stale
or unavailable cache data can narrow what Dalil found.

Dalil supports Rust, JavaScript/JSX, TypeScript/TSX, Python, Ruby, Java, C#,
Go, Lua, and Zig. For another language, or when evidence is uncertain, inspect
source, tests, manifests, and repository instructions directly. Dalil narrows
exploration; it does not replace source inspection.
