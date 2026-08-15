---
title: "Dalil roadmap"
status: "in-progress"
updated: "2026-08-15"
---

Dalil is a local codebase reference engine for humans and coding agents. Its
next milestone is a repository-local evidence bundle that can be inspected,
shared, and carried between agent tasks without exposing Dalil's private cache
format.

## Product direction

Dalil should answer:

> Given this repository, task, current changes, and context budget, what is the
> smallest useful set of evidence someone should inspect next?

A persistent evidence map is the substrate for that answer. Task-aware context
selection remains the product.

The repository bundle makes that substrate available outside one command run:

```text
.dalil/
├── map.json
├── map.md
├── review.md
└── tasks/
    └── <timestamp>-<task-slug>-<id>.md
```

- `map.json` is the portable, versioned representation of Dalil's repository
  evidence.
- `map.md` is the human-readable projection of the same snapshot.
- `review.md` is an optional, compact snapshot for meaningful Git diffs.
- Each task record preserves the original task and the orientation report
  generated for it.

The bundle is an exported product artifact, not Dalil's incremental cache. The
cache remains private to the user cache directory and may change with Dalil's
implementation.

## Repository bundle contract

### Explicit repository writes

Dalil writes `.dalil/` only through `dalil export`. Ordinary `orient`, `map`,
`context`, `impact`, `search`, and `explain` calls remain read-only with
respect to the target repository.

The export flow may create `.dalil/`, replace Dalil-owned map files, and append
task records. It must not edit `.gitignore`, delete unknown files, or decide
whether the bundle should be committed. Documentation will explain the
freshness and sharing tradeoffs so the repository owner can choose.

### Portable evidence map

The JSON map stores evidence that is reusable across tasks:

- repository identity, revision, worktree fingerprint, scope, and provenance;
- project roots, files, symbols, manifests, entry points, tests, and landmarks;
- typed relationships with source, target, evidence kind, resolution,
  confidence, and ambiguity;
- bounded history facts, limitations, quality, and collection summaries;
- schema and producer versions needed to reject incompatible data.

Task scores, personalized PageRank, selected reading order, token allocation,
teaching steps, and impact conclusions are projections over the map. They do
not become canonical map facts.

Node and relationship identifiers must stay stable when their underlying
repository facts have not changed. Arrays and Markdown sections use
deterministic ordering. The exported schema is additive within a schema
version; changing the meaning of an existing field requires a new version.

### Markdown map

`map.md` renders the same exported snapshot as `map.json`. It gives a human a
bounded repository overview, project roots, entry points, important files,
symbols, relationships, tests, history observations, provenance, freshness,
and limitations. It does not parse the JSON after export or run a second
analysis pipeline.

The Markdown can omit low-priority detail to remain readable, but it records
collection totals and omissions and points readers to `map.json` for the full
portable representation.

### Review snapshot

The complete map is useful as local or uploaded evidence, but its size and
incidental detail make it a poor default for version control. An explicit
review export will write `.dalil/review.md` for repositories that want a
generated architectural snapshot in Git.

The file contains project roots, public or exported symbols, cross-project
dependencies, runtime entry points, test roots, and grouped coverage totals.
It uses stable relative paths, deterministic order, and one semantic fact per
line. It excludes individual references, private symbols, source locations,
timestamps, revision identifiers, and worktree state. A hard line and byte
limit keeps pull-request diffs readable; overflow is summarized with totals
and deterministic omission notices.

CI can regenerate the snapshot without writing and fail when the committed
file is missing or stale. Repositories can then ignore `map.json` and `map.md`
while tracking only `review.md`. This follows the review pattern used by
[API Extractor API reports](https://api-extractor.com/pages/setup/configure_api_report/)
and Go's [one-feature-per-line API snapshots](https://go.dev/api/README), while
keeping the full code-intelligence artifact outside Git by default.

### Task records

`dalil export --task <TASK>` appends one Markdown file under `.dalil/tasks/`.
The file contains:

1. a stable record identifier and creation time;
2. repository revision, worktree fingerprint, and Dalil version;
3. the original task exactly as supplied;
4. the task-personalized orientation report and its limitations.

Task filenames use a UTC timestamp, a filesystem-safe task slug, and a short
content-derived identifier. A repeated task creates a new record rather than
overwriting history. Rendering must safely preserve arbitrary task text,
including Markdown delimiters and multiline input.

Task records are append-only from Dalil's perspective. Refreshing the map does
not rewrite previous tasks because each record describes the evidence available
when that task began.

### Refresh and freshness

Each export analyzes the current selected scope and atomically replaces
`map.json` and `map.md` as one logical snapshot. Failure before replacement
leaves the previous pair usable. A task record is published only after its map
snapshot and orientation report are complete.

Every artifact records enough repository state to identify a stale snapshot.
Dirty and untracked paths remain explicit provenance rather than being folded
into a clean-HEAD claim. Dalil may reuse its private incremental index while
building the export, but cold and warm exports of unchanged repository state
must be semantically equivalent.

### Safety

`.dalil/` is the only repository path the export flow may write. Before every
write, Dalil validates the repository root, selected scope, destination, and
all existing path components. It refuses symlinks, reparse points, parent
traversal, non-directory collisions, and destinations outside the worktree.

Writes use private temporary files, flush complete content before publication,
and replace only known Dalil-owned files. Partial reports retain typed quality
and uncertainty. Repository-controlled programs, Git hooks, filters,
credentials, pagers, editors, and network transports remain unavailable.

## Milestones

### 1. Repository evidence bundle

T1 delivers `dalil export`, which writes `.dalil/map.json` and `.dalil/map.md` from
the shared core analysis, then safely refreshes them when the repository
changes.

The milestone is complete when black-box fixtures prove schema compatibility,
semantic parity between JSON and Markdown, atomic replacement, path safety,
deterministic unchanged exports, and clear freshness metadata.

A follow-up adds the optional review snapshot and non-writing freshness check.
The milestone is complete when its diff contains only stable public-surface and
architectural facts, remains within its published size budget, and can be
tracked without committing the complete evidence map.

### 2. Task orientation journal

Extend the same export flow to accept task text and append a task record with
the original input and task-personalized orientation output.

The milestone is complete when multiline and Markdown-heavy tasks round-trip,
same-second writes cannot collide, failed exports leave no partial task, and
existing task records survive map refreshes unchanged.

### 3. Context and scale benchmarks

Build a public or synthetic benchmark corpus for orientation, implementation
search, behavior tracing, bug fixes, feature extensions, refactors, change
review, relevant-test discovery, and repository-bundle export.

Measure useful-file recall, precision, ranking quality, project-root coverage,
relevant-symbol recall, returned tokens, redundancy, latency, memory where
measurable, cache work, export size, and refresh cost. The corpus becomes the
evidence used to admit later language or provider work.

### 4. Product quality and distribution

Audit semantic consistency across CLI, core, MCP, the agent skill, and exported
artifacts. Exercise hostile paths, malformed source, corrupt cache, stale
exports, cancellation, output limits, and clean/warm/invalidated analysis.

After critical findings are resolved, build checksummed packages for supported
platforms and validate install, representative workflows, export, refresh, and
removal from packaged artifacts.

### Deferred extensions

Additional languages, semantic or framework providers, and distributable query
packs require a measured failure in the benchmark corpus or demonstrated user
demand. They are optional extensions and do not block the repository bundle or
its distribution.

## Technical constraints

- Rust edition 2024 with MSRV 1.85.
- Build reports from typed core models. Renderers and adapters must not create
  separate analysis semantics.
- Keep dependency direction from the CLI and MCP crates into `dalil-core`.
- Add a dependency only when the standard library and installed crates cannot
  provide the required behavior.
- Verify at the compiled CLI boundary with small fixture repositories. Use unit
  tests for schema projection, identifiers, rendering, path validation, and
  atomic publication.
- Keep private repository contents out of committed fixtures. Use public or
  synthetic inputs for regressions and benchmarks.

Required verification remains:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo package --workspace --exclude xtask --locked
cargo release-assets
```

## Risks and open questions

- A checked-in map can become stale or create noisy merge conflicts. The full
  map stays a local or uploaded artifact by default; repositories that need a
  versioned review surface can commit the compact snapshot and check freshness
  in CI.
- Stable portable identifiers can accidentally expose cache implementation
  details. Export IDs need their own versioned derivation from repository facts.
- A map can grow too large for agents and code review. JSON collections remain
  bounded and summarized. The review snapshot has a separate hard size limit
  and excludes reference-level detail.
- Dirty-worktree exports can look authoritative after later edits. Every
  artifact records the exact revision and worktree state it describes.
- Two-file publication cannot be perfectly atomic on every filesystem. The
  files need a shared snapshot identifier so consumers can detect a mismatched
  pair after interruption.
- Persisted task text may contain secrets. The command must state that task
  records are repository files and require explicit task input before writing
  one.
