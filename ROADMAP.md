---
title: "Dalil roadmap"
status: "in-progress"
updated: "2026-08-19"
---

Dalil is a local repository intelligence engine for humans and coding agents.
It builds a rich, queryable model of a codebase, then turns that model into
small, inspectable answers and artifacts.

## Product direction

Dalil should answer both kinds of questions:

> What does this repository contain and how is it connected?

and:

> Given this task, change, symbol, or subsystem, what is the smallest useful
> projection of that repository intelligence?

The first question defines Dalil's capability surface. The second defines its
output discipline.

Dalil should know more than it normally prints.

### Product principles

1. **Rich model, bounded outputs.** The repository model may contain files,
   symbols, definitions, references, calls, imports, dependencies, tests,
   manifests, history, changes, and derived structural signals. Human and agent
   outputs remain concise by default.
2. **Search is infrastructure.** Repeated lexical, path, symbol, structural,
   and Git-aware queries should be cheap after indexing.
3. **Relationships are queryable facts.** Callers, callees, references,
   dependencies, neighborhoods, paths, and change propagation are first-class
   operations rather than details hidden inside reports.
4. **Artifacts are projections, not dumps.** Architecture, subsystem, impact,
   review, context, and repository-map artifacts select and summarize the
   underlying model for a specific use.
5. **One semantic core.** CLI commands, MCP tools, agent integrations, HTML,
   JSON, Markdown, and exported artifacts must use the same typed operations.
6. **Local and inspectable by default.** Core analysis stays offline,
   deterministic where the underlying evidence is deterministic, and does not
   execute repository-controlled code.
7. **Evidence carries provenance.** Derived relationships and structural claims
   retain source, resolution, confidence, ambiguity, and limitations.

## Repository intelligence model

Dalil has three conceptual layers:

```text
repository
    |
    v
index
  files | text | symbols | git | manifests | tests
    |
    v
relationship graph
  refs | calls | imports | dependencies | containment | changes
    |
    v
query + analysis
  search | neighbors | paths | impact | structure | ranking
    |
    v
bounded projections
  orient | context | architecture | subsystem | impact | review | export
```

The implementation does not need to expose these layers as separate storage
systems. The distinction exists to keep indexing, graph analysis, and report
rendering from collapsing into one pipeline.

## Query surface

Dalil should expose composable repository-wide primitives in `dalil-core`.
CLI, MCP, and other adapters project these operations rather than implementing
new semantics.

The target operation families are:

- repository overview and project discovery;
- lexical, path, symbol, and filtered search;
- symbol lookup and definition/reference queries;
- callers, callees, imports, and dependency queries where supported;
- bounded neighborhoods and paths through repository relationships;
- current-change and revision-range impact;
- relevant tests, entry points, and project boundaries;
- history, churn, activity, and hotspot signals;
- structural summaries such as central nodes, components, and communities when
  the evidence supports them.

Every operation must have deterministic limits or explicit pagination. A query
that matches hundreds of facts should return the best bounded result plus
collection totals or continuation metadata rather than flooding model context.

## Repository artifacts

The repository bundle remains an explicit export surface:

```text
.dalil/
├── map.json
├── map.md
├── review.md
└── tasks/
    └── <timestamp>-<task-slug>-<id>.md
```

- `map.json` is the portable, versioned repository evidence model.
- `map.md` is a bounded human-readable projection of that snapshot.
- `review.md` is a compact, stable architectural surface intended for Git
  review.
- Task records preserve task-specific projections when explicitly requested.

Additional artifacts should be able to use the same model without expanding
`map.md` into a universal report. Candidate projections include architecture,
subsystem, hotspot, and change-impact reports.

Artifacts should answer a recognizable question, fit a published size budget,
and report omitted detail. The complete repository model is available through
JSON and typed queries; Markdown should favor usefulness over completeness.

## Repository bundle contract

### Explicit repository writes

Dalil writes `.dalil/` only through explicit export operations. Ordinary
analysis and query commands remain read-only with respect to the target
repository.

The export flow may create `.dalil/`, replace Dalil-owned generated files, and
append explicitly requested records. It must not edit `.gitignore`, delete
unknown files, or decide whether generated artifacts should be committed.

### Portable evidence map

The JSON map stores reusable repository facts and evidence:

- repository identity, revision, worktree fingerprint, scope, and provenance;
- project roots, files, symbols, manifests, entry points, tests, and landmarks;
- typed relationships with source, target, evidence kind, resolution,
  confidence, and ambiguity;
- bounded history and current-change facts;
- collection summaries, limitations, and quality metadata;
- schema and producer versions needed to reject incompatible data.

Task rankings, reading order, token allocation, graph ranking, impact
conclusions, communities, and other analyses are projections over repository
facts unless their representation explicitly defines them as derived evidence.
They do not silently become canonical facts.

Node and relationship identifiers remain stable when their underlying facts do
not change. Arrays and Markdown sections use deterministic ordering. Changing
the meaning of an existing exported field requires a schema version change.

### Markdown and review projections

`map.md` renders a bounded overview from the same typed snapshot as `map.json`.
It may omit low-priority detail but records totals and omissions.

`review.md` remains intentionally smaller. It contains stable public-surface
and architectural facts useful in a Git diff and excludes noisy reference-level
or worktree-specific detail.

### Task records

`dalil export --task <TASK>` remains supported, but task journaling is one
projection rather than the organizing model for Dalil. Task files preserve the
original task, matching repository state, selected evidence, quality, and
limitations. They remain append-only from Dalil's perspective.

### Cache and freshness

The private cache is an implementation detail optimized for repeated local
queries. Export formats must not expose cache layout or require consumers to
understand it.

Cold and warm analysis of unchanged repository state must be semantically
equivalent. Targeted invalidation should avoid rebuilding unrelated index and
graph state. Every exported artifact records enough repository state to detect
staleness.

### Safety

Repository analysis does not execute project code, Git hooks, filters, pagers,
editors, credential helpers, or network transports. Export validates all
repository-local destinations and publishes generated files safely.

Partial analysis retains typed uncertainty and limitations rather than
silently promoting inferred relationships to precise facts.

## Milestones

### 1. Repository evidence foundation — complete

Ship the portable repository evidence map, bounded Markdown projection,
reviewable architectural snapshot, safe export flow, and explicit task records.

This establishes the stable repository model and artifact contract on which
later query and graph capabilities build.

### 2. Repository query engine

Turn the existing evidence model into a first-class query surface.

Deliver typed core operations for filtered search, symbol lookup, definitions,
references, dependency relationships, current changes, tests, and history.
Strengthen the private index so repeated queries avoid rescanning unrelated
repository content.

The milestone is complete when CLI and JSON callers can perform the common
repository-wide queries with bounded, deterministic results and equivalent cold
and warm semantics.

### 3. Relationship graph and traversal — complete

Make repository relationships directly traversable. `dalil impact` now seeds
this graph from resolved changes and explicit file or symbol inputs, walks
incoming relationships, and reports ranked direct, transitive, and
inferred downstream evidence with the paths and limits behind each conclusion.

Add efficient adjacency access over typed relationships, neighborhoods,
paths, reverse dependencies, callers/callees where evidence supports them, and
change propagation. Preserve provenance and uncertainty through traversal.

The milestone is complete when graph-native operations are useful without
requiring consumers to parse `map.json` or reconstruct relationships from
rendered reports.

### 4. Structural repository analysis

Build useful codebase-wide analysis on top of the query and relationship
layers.

Add structural signals such as central nodes, dependency components,
architectural boundaries, hotspots, and communities where they produce stable,
explainable results. Combine structure with tests, entry points, Git history,
and current changes instead of treating the source graph in isolation.

The milestone is complete when each derived result can explain the evidence
behind it and remains bounded enough for routine agent use.

### 5. Bounded artifact projections

Use repository-wide intelligence to produce concise artifacts for recognizable
workflows.

Prioritize architecture, subsystem, impact, review, and context projections.
Each projection receives a hard output budget, reports omissions, and links its
claims back to typed repository evidence.

The milestone is complete when a user can obtain useful repository-wide
artifacts without consuming the complete graph or evidence map.

### 6. Agent and integration surface

Expose the same repository intelligence as composable agent tools.

MCP and native integrations should support repository overview, search, symbol
lookup, references, dependencies, callers/callees, neighborhoods, paths,
changes, impact, history, and bounded context construction. Large collections
must use limits or continuation rather than unbounded tool responses.

The milestone is complete when an agent can investigate a repository through
small tool calls without falling back to repeated broad file reads for facts
Dalil already knows.

### 7. Quality, scale, and distribution

Benchmark search quality, graph correctness, context efficiency, traversal
cost, artifact usefulness, indexing work, invalidation, latency, memory, and
output size on public or synthetic repositories.

Harden safety and cross-interface semantic consistency, then build and validate
checksummed packages on supported platforms.

## Deferred extensions

Optional semantic providers, framework-specific enrichers, additional
languages, alternate persistent stores, and independently distributed query
packs stay outside the active roadmap until there is a concrete implementation
reason to schedule them.

They should become TODO items only when Dalil has decided to build them. The
TODO is not a research queue.

## Technical constraints

- Rust edition 2024 with MSRV 1.85.
- Keep `dalil-core` authoritative for repository facts, queries, analysis, and
  projection models.
- Keep dependency direction from CLI and MCP crates into `dalil-core`.
- Renderers and adapters must not create separate analysis semantics.
- Keep outputs bounded by default; machine-readable interfaces must report
  totals, omissions, or continuation state when results are truncated.
- Preserve provenance, confidence, ambiguity, and partial-analysis state across
  query and graph operations.
- Add dependencies only when they materially simplify a capability Dalil has
  committed to shipping.
- Verify at the compiled CLI boundary with small fixture repositories and use
  focused unit tests for index, query, graph, schema, rendering, and safety
  behavior.
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

- A richer graph can tempt Dalil into returning too much. Output budgets and
  continuation must remain part of every public query contract.
- Approximate lexical or syntax relationships can look more precise than they
  are. Traversal and derived analysis must preserve provenance and confidence.
- A persistent index can become a second public format by accident. Cache
  layout remains private; exported schemas stay intentionally separate.
- Structural algorithms can reward generic utility nodes or generated code.
  Ranking and community outputs need repository-aware filtering and clear
  explanations.
- Cross-language call and reference resolution will remain incomplete without
  semantic providers. Dalil should return bounded uncertainty rather than hide
  the gap.
- Checked-in generated artifacts can become stale or noisy. Full evidence stays
  local by default; Git-oriented projections remain deliberately compact.
