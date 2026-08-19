# To-Do/Task List

## Foundation

### T1 — Export the repository evidence map

- [x] Export the shared typed repository snapshot to `.dalil/map.json` and
      `.dalil/map.md`.
- [x] Preserve stable identifiers, deterministic ordering, freshness metadata,
      provenance, quality, limitations, and collection summaries.
- [x] Keep normal analysis commands repository-read-only.
- [x] Publish generated files safely and atomically inside `.dalil/`.

### T1.1 — Add a reviewable repository snapshot

- [x] Add `dalil export --review` and non-writing `--check` mode.
- [x] Keep the snapshot deterministic and focused on stable public
      surface and architecture.
- [x] Allow repositories to commit `review.md` without committing the complete
      evidence map.

### T2 — Record explicit task projections

- [x] Add `dalil export --task <TASK>` with append-only task records.
- [x] Preserve exact task input, matching repository state, orientation output,
      quality, and limitations.
- [x] Keep task records collision-safe and publish them only after their matching
      analysis completes.

## Milestone: Repository query engine

**Exit condition:** `dalil-core` exposes bounded repository-wide queries that
CLI and integrations can use without reconstructing answers from reports.

### T3 — Add a typed repository query model

**What to build:** One typed query/result layer for repository search and
lookup operations.

**Acceptance criteria:**

- [x] Define typed queries for text, path, symbol, project, language, symbol
      kind, test, changed-path, and revision-aware filters.
- [x] Define one bounded result contract with deterministic ordering, result
      totals, omissions, and continuation where needed.
- [x] Keep query semantics in `dalil-core`; CLI, MCP, and renderers only adapt
      typed requests and responses.
- [x] Reuse existing repository evidence and caches instead of reparsing rendered
      output or rescanning unrelated files.
- [x] Preserve provenance, confidence, ambiguity, and partial-analysis state in
      query results where those fields apply.
- [x] Add JSON fixtures for query compatibility and deterministic repeated runs.

**Verification:** Exercise exact, prefix, substring, filtered, empty, ambiguous,
and high-cardinality queries on small fixture repositories.

### T4 — Strengthen `dalil search` (trigram index)

**What to build:** `dalil search` becomes a fast repository-wide retrieval tool
rather than a thin concept lookup.

**Acceptance criteria:**

- [x] Support lexical content, path, and symbol search through the T3 query
      model.
- [x] Add filters for project, language, symbol kind, tests, and current changes.
- [x] Rank exact symbol and path matches ahead of broad lexical matches while
      keeping ordering deterministic.
- [x] Return concise human output by default and the complete bounded typed
      result through `--json`.
- [x] Report total matches and omitted results instead of silently truncating.
- [x] Ensure warm repeated searches avoid unnecessary parsing or Git work.

**Verification:** Add compiled-CLI fixtures covering exact symbols, ambiguous
names, path filters, monorepos, large match sets, and changed-file filters.

### T5 — Expose symbol and relationship queries (petgraph backed)

**What to build:** Users and agents can interrogate repository relationships
directly.

**Acceptance criteria:**

- [x] Add typed operations for symbol lookup, definitions, references, imports,
      dependencies, reverse dependencies, and related tests.
- [x] Add callers and callees for relationships Dalil can support with explicit
      evidence quality; do not present unresolved lexical matches as precise
      calls.
- [x] Provide paged CLI surfaces for the operations that are useful to humans.
- [x] Preserve stable node and relationship identifiers in JSON responses.
- [x] Explain ambiguous or partial relationships with provenance and confidence.
- [x] Avoid adding new parser semantics in adapters or renderers.

**Verification:** Use fixtures with duplicate names, aliases, cross-file
references, unresolved calls, multiple projects, and tests.

## Milestone: Relationship graph

**Exit condition:** Repository relationships can be traversed efficiently and
composed into higher-level analysis.

### T6 — Add graph traversal primitives

**What to build:** Efficient bounded traversal over Dalil's typed repository
relationships.

**Acceptance criteria:**

- [x] Add adjacency access by node and relationship kind without scanning the
      complete exported map.
- [x] Add bounded `neighbors` with depth, relationship-kind, project, and result
      limits.
- [x] Add shortest or best-supported `path` queries with explicit maximum depth
      and work limits.
- [x] Add reverse-dependency traversal suitable for change impact.
- [x] Preserve edge provenance, confidence, and ambiguity through returned
      paths and neighborhoods.
- [x] Detect cycles and repeated nodes without unbounded traversal.

**Verification:** CLI fixtures cover cycles, reverse dependencies, depth and work
limits, and deterministic repeated output. Traversal uses the same typed graph
and relationship evidence as direct relationship queries.

### T7 — Build change-impact traversal on the graph

**What to build:** `dalil impact` uses the shared relationship graph rather than
a separate ad hoc ranking path.

**Acceptance criteria:**

- [ ] Seed impact from dirty paths, revision ranges, files, and symbols.
- [ ] Separate direct evidence from transitive or inferred downstream impact.
- [ ] Rank affected symbols, files, projects, and tests under a fixed output
      budget.
- [ ] Report the relationship path behind high-priority impact conclusions.
- [ ] Cap traversal work and state when the cap makes the result incomplete.
- [ ] Keep existing human and JSON impact semantics compatible where practical.

**Verification:** Exercise isolated changes, shared utilities, cross-project
changes, cycles, test-only changes, ambiguous references, and large fan-out.

## Milestone: Structural repository analysis

**Exit condition:** Dalil can turn repository-wide structure into concise,
explainable architectural signals.

### T8 — Add structural analysis primitives

**What to build:** Derived repository structure suitable for architecture and
hotspot projections.

**Acceptance criteria:**

- [ ] Compute connected or dependency components over selected relationship
      kinds.
- [ ] Identify central files or symbols with repository-aware filtering so
      generic utilities and generated code do not dominate by default.
- [ ] Detect strongly coupled or cyclic dependency groups where applicable.
- [ ] Add bounded community or subsystem grouping only when the grouping can be
      explained through explicit repository relationships.
- [ ] Combine structural signals with project roots, entry points, tests, churn,
      and current changes rather than treating graph topology alone as truth.
- [ ] Keep all derived scores out of canonical repository facts unless the
      exported schema explicitly marks them as derived analysis.

**Verification:** Add fixtures with clear modules, shared utilities, cycles,
monorepo boundaries, generated code, and intentionally ambiguous structure.

### T9 — Add concise architecture and subsystem projections

**What to build:** Repository-wide graph analysis produces small artifacts and
reports rather than graph dumps.

**Acceptance criteria:**

- [ ] Add an `architecture` projection with major projects/subsystems, important
      boundaries, central abstractions, entry points, tests, and a short reading
      order.
- [ ] Add a `subsystem <anchor>` projection seeded by path, symbol, or project.
- [ ] Give both projections hard line and byte budgets with deterministic
      omission summaries.
- [ ] Include evidence or explanation hooks for every non-obvious structural
      claim.
- [ ] Render human-readable text/Markdown and typed JSON from the same model.
- [ ] Ensure projections remain useful on repositories too large to render the
      complete map.

**Verification:** Compare outputs on a small library, Dalil, a mixed monorepo,
and a generated-heavy fixture; assert size budgets and deterministic unchanged
output.

## Milestone: Agent tool surface

**Exit condition:** An agent can inspect repository structure through small,
composable Dalil calls instead of broad repeated file reads.

### T10 — Expand MCP and native agent operations

**What to build:** Agent integrations expose the repository query and graph
surface directly.

**Acceptance criteria:**

- [ ] Expose repository overview, search, symbol lookup, references,
      dependencies, callers, callees, neighbors, paths, changes, impact,
      history, architecture, subsystem, and context operations as appropriate.
- [ ] Put strict default limits on every collection-returning tool.
- [ ] Return totals, omissions, and continuation state when more information is
      available.
- [ ] Keep tool descriptions narrow enough that agents can distinguish search,
      traversal, impact, and projection operations.
- [ ] Keep MCP and native adapters semantically equivalent to `dalil-core`.
- [ ] Update the Dalil agent skill to prefer narrow repository-intelligence
      operations before broad source reads.

**Verification:** Run scripted agent-like investigation traces for symbol
lookup, behavior tracing, change review, relevant-test discovery, and subsystem
orientation while recording tool calls and returned bytes.

## Milestone: Indexing, quality, and scale

**Exit condition:** Repository-wide queries remain fast, bounded, and
semantically stable as repositories grow.

### T11 — Harden the private incremental index

**What to build:** Repeated search and traversal avoid rebuilding unrelated
repository state.

**Acceptance criteria:**

- [ ] Track enough file, symbol, relationship, Git, and project fingerprints to
      invalidate changed evidence selectively.
- [ ] Keep cache layout private and independently versioned from exported
      artifacts.
- [ ] Make cold, warm, and selectively invalidated results semantically
      equivalent for unchanged repository facts.
- [ ] Bound cache disk use and remove obsolete entries safely.
- [ ] Record cache work in diagnostics or benchmark instrumentation without
      leaking internal details into normal reports.

**Verification:** Cover unchanged runs, single-file edits, renames, deletes,
branch changes, schema upgrades, corrupt cache, and no-cache operation.

### T12 — Add context and scale regression gates

**What to build:** CI catches regressions in useful retrieval, graph behavior,
output size, and analysis cost.

**Acceptance criteria:**

- [ ] Add public or synthetic cases for search, symbol lookup, graph traversal,
      impact, architecture, subsystem, and relevant-test discovery.
- [ ] Measure useful result recall or expected-result rank where a fixture has a
      known answer.
- [ ] Measure returned bytes/tokens, redundancy, latency, indexing work, cache
      reuse, traversal work, memory where practical, and artifact size.
- [ ] Add routine CI cases plus a larger pre-release scale tier.
- [ ] Give every threshold failure the fixture and exceeded dimension.
- [ ] Keep all benchmark inputs offline, deterministic, public, or synthetic.

**Verification:** Run the suite cold, warm, and selectively invalidated; inject
representative ranking, output-size, and traversal-work regressions and confirm
the expected gates fail.

## Milestone: Product hardening and distribution

**Exit condition:** The repository-intelligence surface is consistent, safe,
and reproducibly packaged.

### T13 — Audit cross-interface semantics and safety

**What to build:** CLI, core, MCP, agent integrations, cache behavior, and
exported artifacts agree on repository facts and bounded-result semantics.

**Acceptance criteria:**

- [ ] Exercise malformed source, hostile paths, corrupt cache, unsupported
      languages, partial history, cancellation, stale exports, symlink races,
      ambiguous relationships, output limits, and interrupted publication.
- [ ] Confirm every interface reports uncertainty, omissions, and compatibility
      failures consistently.
- [ ] Prove ordinary query and analysis commands never write to the repository.
- [ ] Confirm export never writes outside `.dalil/` or alters unknown files.
- [ ] Convert confirmed correctness or security failures into focused regression
      tests before release.

**Verification:** Run security, compatibility, schema, query, graph, cache,
artifact, and cross-interface suites, then rerun T12.

### T14 — Build and validate the distribution candidate

**What to build:** Supported platforms produce reproducible packages containing
the complete repository-intelligence surface.

**Acceptance criteria:**

- [ ] Pass Linux, macOS, Windows, Rust 1.85, dependency-policy, schema,
      generated-asset, and package-content jobs.
- [ ] Produce checksummed archives for supported distribution targets.
- [ ] Confirm completions, man pages, schemas, agent instructions, and artifact
      documentation match the packaged binary.
- [ ] Install packages in clean environments and run representative search,
      symbol, traversal, impact, architecture, context, export, and cache
      workflows.
- [ ] Confirm clean rebuilds reproduce package contents and metadata.

**Verification:** Run the workspace and release checks from ROADMAP.md against
the candidate commit, inspect each archive, install it cleanly, execute the
representative workflows, and remove it using documented instructions.
