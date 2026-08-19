---
title: Source analysis
description: See what Dalil extracts, where its evidence stops, and how analysis is constrained.
section: Reference
group: Reference
order: 4
---

`dalil map` inventories a worktree and extracts structural evidence from Rust,
JavaScript, JSX, TypeScript, TSX, Python, Ruby, Java, C#, Go, Lua, and Zig
source files.

```sh
dalil map
dalil map src --exclude 'src/generated/**'
dalil map --profile evidence --json
```

## Extracted evidence

For supported source files, Dalil records definitions and lexical references
with symbol kind, visibility, enclosing scope, source location, and compact
declaration context. It resolves language- and import-aware lexical file edges
when the source provides enough evidence, and records the resolution reason,
confidence, ambiguity, and candidate group.

The map also includes repository landmarks such as READMEs, agent and
contributor instructions, manifests, lockfiles, project roots, build and test
entry points, CI configuration, ownership, licenses, submodules, and nested
repositories. See [manifest support](/docs/reference/manifests/) for declared
entry points and commands.

Go analysis retains package and receiver scopes, import aliases, exported
visibility, and `_test.go` declarations. Lua analysis covers local and global
functions, methods, variables, assignments, table fields, calls, and literal
`require` paths. Zig analysis covers containers, functions, variables, fields,
tests, public declarations, calls, type uses, field access, and literal
`@import` paths.

## Relationship queries

Use `dalil relationships` to inspect one symbol or file relationship at a time:

```sh
dalil relationships definitions CacheStore --json
dalil relationships references CacheStore
dalil relationships callers handle
dalil relationships dependencies src/cache.rs
dalil relationships tests src/cache.rs
```

The operation names are `symbol`, `definitions`, `references`, `imports`,
`dependencies`, `reverse-dependencies`, `tests`, `callers`, and `callees`.
Symbol operations use an exact name. File operations use a repository-relative
path. `--limit` and `--budget` control the page size and output size; JSON
reports totals, omissions, continuation, stable node IDs, relationship IDs,
source evidence, confidence, and limitations.

`callers` and `callees` include only references tagged as calls. Ambiguous
lexical candidates stay marked as ambiguous and are not presented as resolved
semantic calls.

## Graph traversal

Use `dalil traverse` when a direct relationship query is not enough:

```sh
dalil traverse neighbors src/map.rs --depth 2 --limit 20
dalil traverse path src/cli.rs src/report.rs --direction outgoing
dalil traverse reverse-dependencies src/map.rs --depth 3
```

`neighbors` walks incoming, outgoing, or both directions. `path` returns one
shortest path and uses relationship confidence and stable edge IDs to break
ties. `reverse-dependencies` follows incoming file-level dependency and import
edges, so it can walk transitive dependents for change review. Use `--kind` to
select dependency, import, reference, type-reference, or call edges, and
`--project` to stop at a detected project root.

Every traversal has a maximum depth, edge-inspection work limit, result limit,
and token budget. JSON output reports visited nodes, inspected edges, limits,
omissions, and the edge evidence for each returned step. Visited node IDs stop
cycles and repeated nodes from expanding indefinitely. Paths and neighborhoods
retain confidence, ambiguity, provenance, and partial-source limitations from
the relationships they use.

## Change impact

Use `dalil impact` when a change needs downstream review rather than one direct
relationship query:

```sh
dalil impact --dirty-worktree --json
dalil impact --revision-range 'HEAD~1..HEAD' --json
dalil impact --task-path src/parser.rs --json
dalil impact --symbol parse --json
```

Impact seeds come from resolved dirty-worktree or revision-range paths and
symbols, plus explicit `--task-path`, `--changed-path`, `--symbol`, and
`--changed-symbol` inputs. The report includes `seeds`, ranked file targets and
symbols, affected projects, ranked likely tests, and `relationship_path` data
for graph-reached targets.

A target marked `direct` is a changed or explicitly seeded path or symbol, or a
one-edge downstream result. `transitive` requires more than one retained graph
edge. `inferred` covers ambiguous or structural evidence that does not establish
a direct graph relationship. The traversal is capped at 16 edges of depth and
20,000 inspected edges. JSON reports `impact.traversal`; when a cap stops the
walk, `impact.uncertainty` says that the result is incomplete.

Impact walks incoming dependency, import, reference, type-reference, and call
edges because a file or symbol used by another node is the downstream review
seed. The relationship graph remains lexical evidence. It does not prove
compiler-resolved callers, runtime control flow, or breakage.

## Evidence boundaries

Relationships are lexical and structural evidence. They do not prove runtime
control flow. Dynamic Lua imports, metatable behavior, runtime table mutation,
Zig comptime evaluation, inferred types, generic instantiation, error-union
flow, and non-literal imports remain unresolved and appear as limitations.

Generated, vendored, minified, and source-map paths are excluded unless an
exact focus path selects them. Non-source documentation, configuration, and
assets appear as inventory omissions rather than unsupported programming
languages. Nested repositories and checked-out submodules are boundaries by
default; pass `--recursive` to include their source.

Dalil records parse errors, invalid query packs, ambiguous references, partial
evidence, and resource limits beside the affected collections. Use
`dalil capabilities --json` to inspect installed grammars and query packs, and
`dalil doctor` to check repository discovery, path safety, cache permissions,
schemas, and effective limits.

## Profiles and limits

The `compact` profile is the default. It favors a small ranked selection for
orientation. The `evidence` profile raises collection caps for consumers that
need a broader structural sample.

Compact analysis allows up to 4,096 files, 1 MiB per file, 64 MiB of source,
2,048 syntax levels, 20,000 symbols, 32 lexical candidates per reference,
2,000 edges or findings, 100,000 reachable commits, and 128 history items per
collection. Analysis work is limited to 30 seconds and rendered output to
8 MiB. Compact output includes at most 64 landmarks and 32 project roots.
Reports retain collection totals and truncation reasons when a cap applies.

## Cache controls

Dalil stores per-file records and a versioned repository index under
`$XDG_CACHE_HOME/dalil`, or `~/.cache/dalil` when that variable is unset.
Later runs reparse changed source and reuse unaffected lexical relationships.
Map JSON reports reused, invalidated, refreshed, stale, bypassed, and failed
cache state.

```sh
dalil map --cache always
dalil map --cache files --cache-file src/parser.rs
dalil map --cache manual
dalil map --no-cache
dalil cache path
dalil cache status
dalil cache prune
dalil cache clear
```

`--no-cache` bypasses cache reads and writes. Normal analysis commands never
write inside the analyzed repository. `dalil export` is the explicit
repository-writing command; see [repository evidence bundles](/docs/guides/repository-evidence-bundles/).
