# To-Do/Task List

## Milestone: Repository evidence bundle

**Exit condition:** `dalil export` creates a safe, refreshable `.dalil/` bundle
containing a portable JSON evidence map and a human-readable Markdown
projection of the same snapshot.

### T1 — Export the repository evidence map — complete

**What to build:** A user can run `dalil export` to write the current repository
map to `.dalil/map.json` and `.dalil/map.md` through one end-to-end CLI flow.

**Blocked by:** None - can start immediately.

**Acceptance criteria:**

- [x] Reuse the typed core analysis and current renderers where their semantics
      match. Do not reconstruct evidence from rendered output.
- [x] Give `map.json` a portable schema with repository identity, revision,
      worktree fingerprint, projects, files, symbols, relationships,
      landmarks, tests, bounded history, quality, limitations, provenance,
      collection summaries, schema version, and producer version.
- [x] Keep task rankings, reading order, token allocations, teaching steps, and
      impact conclusions out of the canonical map.
- [x] Give nodes and relationships stable identifiers and deterministic order
      when their repository facts are unchanged.
- [x] Render `map.md` from the same snapshot. It may project collections for
      readability but must report totals, omissions, snapshot identity, and
      freshness metadata.
- [x] Write only after an explicit export request. Normal analysis commands
      must retain their current repository-read-only behavior.
- [x] Refuse an unsafe repository root, destination outside the worktree,
      symlink or reparse-point component, parent traversal, and a non-directory
      `.dalil` collision.
- [x] Create parent directories and map files with private permissions, publish
      complete temporary files atomically, and replace only Dalil-owned map
      files. Do not edit `.gitignore` or remove unknown `.dalil/` contents.
- [x] Give both files one snapshot identifier so readers can detect a
      mismatched pair after an interrupted refresh.
- [x] Document how to export, inspect, refresh, ignore, or commit the bundle,
      including the stale-map and merge-conflict tradeoffs.

**Verification:**

- Add compiled-CLI fixtures for a clean repository, dirty worktree, monorepo,
  unsupported source, existing bundle, unsafe destination, and interrupted
  replacement.
- Assert JSON schema compatibility, stable identifiers, deterministic repeated
  exports, shared snapshot identity, Markdown/JSON semantic parity, private
  permissions where supported, and unchanged unknown files.
- Run the standard format, workspace test, Clippy, documentation, package, and
  release-asset checks from ROADMAP.md.

### T1.1 — Add a reviewable repository snapshot

**What to build:** A user can generate a compact `.dalil/review.md` whose Git
diff shows changes to the repository's public surface and architecture without
committing the complete evidence map.

**Blocked by:** T1.

**Acceptance criteria:**

- [ ] Add `dalil export --review` to write the review snapshot and
      `dalil export --review --check` to regenerate and compare it without
      changing repository files.
- [ ] Render one stable fact per line: project roots, public or exported
      symbols, cross-project dependencies, runtime entry points, test roots,
      and grouped coverage or omission totals.
- [ ] Omit individual references, private and local symbols, source locations,
      absolute paths, timestamps, revision identifiers, worktree state, and
      full ignored-file inventories.
- [ ] Sort every section deterministically and cap the result at 2,000 lines or
      200 KiB. Record totals and deterministic omissions when the cap applies.
- [ ] Make check mode return a distinct non-zero status when the committed
      snapshot is missing or stale, with a command that refreshes it.
- [ ] Keep `.dalil/map.json` and `.dalil/map.md` independent of the review
      snapshot so repositories can ignore the complete map while committing
      only `.dalil/review.md`.
- [ ] Document the intended Git workflow, selective `.gitignore` rules, merge
      behavior, generated-file notice, and the difference between the review
      snapshot and the portable evidence map.

**Verification:**

- Add compiled-CLI fixtures for first write, unchanged check, semantic change,
  irrelevant private change, deterministic overflow, and missing snapshot.
- Assert that unchanged semantic input is byte-identical, private source churn
  does not alter the review snapshot, and check mode never writes.
- Run the standard verification commands from ROADMAP.md.

## Milestone: Task orientation journal

**Exit condition:** An export with task text appends one durable task record
containing the original input and the orientation generated from the matching
repository snapshot.

### T2 — Record task inputs and orientation output

**What to build:** A user can run `dalil export --task <TASK>` and receive
`.dalil/tasks/<timestamp>-<task-slug>-<id>.md` without losing or rewriting
earlier task records.

**Blocked by:** T1.

**Acceptance criteria:**

- [ ] Generate the orientation through the shared typed operation with the
      supplied task as a ranking seed.
- [ ] Record a stable task ID, UTC creation time, Dalil version, map snapshot
      ID, repository revision, worktree fingerprint, original task, rendered
      orientation, quality, and limitations.
- [ ] Preserve the task text exactly, including blank lines, Unicode, Markdown
      headings, delimiters, and code fences, while keeping the task file valid
      Markdown.
- [ ] Use a filesystem-safe bounded slug and content-derived suffix so empty,
      long, non-ASCII, duplicate, and same-second tasks cannot collide.
- [ ] Append a new file for each explicit task export. Never overwrite or
      rewrite an earlier task record during another task or map refresh.
- [ ] Publish the task record only after the matching map snapshot and
      orientation are complete. A failed export must leave no partial record.
- [ ] Do not create a task file when task text was not explicitly supplied.
- [ ] Warn in command help and documentation that task records are repository
      files and may contain sensitive input.

**Verification:**

- Add compiled-CLI fixtures for multiline Markdown, Unicode, empty normalized
  slugs, repeated tasks, same-second tasks, dirty worktrees, partial analysis,
  and publication failure.
- Assert exact task round-tripping, orientation task personalization, map/task
  snapshot linkage, collision resistance, append-only behavior, and cleanup of
  temporary files after failure.
- Run the standard verification commands from ROADMAP.md.

## Milestone: Context and scale benchmarking

**Exit condition:** Public or synthetic benchmarks measure retrieval quality,
context efficiency, repository export, refresh cost, latency, work, memory
where measurable, and output size.

### T3 — Build the context-quality benchmark

**What to build:** Retrieval and export changes can be compared against a
reproducible task corpus instead of a few repository examples.

**Blocked by:** T1 and T2.

**Acceptance criteria:**

- [ ] Define fixtures for orientation, implementation search, behavior
      tracing, bug fixes, feature extensions, refactors, change review,
      relevant-test discovery, and repository export.
- [ ] Use public or synthetic repositories with expected useful paths or graded
      path rankings for each task.
- [ ] Measure useful-file recall, precision, ranking quality, project-root
      coverage, relevant-symbol recall, returned tokens, redundancy, and the
      share of budget consumed by one file.
- [ ] Measure exported node and relationship coverage, JSON and Markdown size,
      task-record usefulness, and cold versus warm refresh work.
- [ ] Record fixture-level failure labels for missing entry points, duplicated
      roles, irrelevant central files, weak explanations, stale artifacts, and
      token waste.
- [ ] Capture the shipped feature set as a deterministic baseline with local
      and CI regression thresholds.

**Verification:** Run the corpus twice from cold state and once from valid warm
state; confirm stable metrics, semantically equivalent results, and actionable
fixture-level failures.

### T4 — Establish performance and scale gates

**What to build:** CI detects unacceptable analysis, export, memory, disk, or
output growth before a distribution candidate is built.

**Blocked by:** T3.

**Acceptance criteria:**

- [ ] Add CI-friendly cases for Dalil, a large ignored or vendor tree,
      high-ambiguity sources, large monorepos, large `.dalil/tasks/`
      directories, and synthetic 10k- and 100k-commit histories.
- [ ] Exercise cold analysis, warm analysis, targeted invalidation, task
      context, impact, first export, unchanged export, changed export, and task
      append through every supported integration path.
- [ ] Define latency, work, memory where measurable, disk, and output ceilings.
      A failure must identify the exceeded dimension and fixture.
- [ ] Keep every benchmark public or synthetic, deterministic, bounded, and
      runnable without network access.
- [ ] Define a small routine CI tier and a full pre-distribution scale tier.

**Verification:** Run the harness under its documented ceilings and confirm
intentional regressions fail with the expected dimension and fixture name.

## Milestone: Product quality review

**Exit condition:** The completed product passes correctness, security,
compatibility, usefulness, and cross-interface consistency review with no
unresolved critical finding.

### T5 — Review usefulness and semantic consistency

**What to build:** Representative repositories receive useful, consistent
answers across native reports, integrations, and repository artifacts.

**Blocked by:** T3 and T4.

**Acceptance criteria:**

- [ ] Apply one review rubric to a small project, Dalil, a mixed monorepo, a
      generated-heavy repository, and representative change sets.
- [ ] Review reading plans, context, impact uncertainty, history usefulness,
      token economy, exported map readability, and task records.
- [ ] Confirm equivalent requests retain equivalent semantics across CLI,
      core, MCP, agent instructions, lifecycle calls, and `.dalil/` output.
- [ ] Retain only aggregate outcomes and public or synthetic regressions from
      the available project corpus.
- [ ] Convert correctness failures into focused regression tests and resolve
      every critical usability finding.

**Verification:** Complete the rubric and rerun T3 and T4 after fixes.

### T6 — Audit safety and compatibility

**What to build:** Dalil preserves its bounded, deterministic behavior and
limits repository writes to the explicit `.dalil/` export under hostile and
partial conditions.

**Blocked by:** T3 and T4.

**Acceptance criteria:**

- [ ] Exercise hostile paths, external filters, malformed source, corrupt
      cache, unsupported languages, partial history, cancellation, output
      limits, stale exports, symlink races, and interrupted publication.
- [ ] Confirm every interface reports partial work, uncertainty, omissions,
      and compatibility failures consistently.
- [ ] Verify cold, warm, invalidated, and no-cache results remain semantically
      equivalent where repository state is unchanged.
- [ ] Confirm schema fixtures, public library types, MCP responses, generated
      assets, and repository artifacts match their documented versions.
- [ ] Prove ordinary commands never write to the repository and export never
      writes outside `.dalil/` or alters unknown files.
- [ ] Convert confirmed correctness or security failures into focused
      regression tests and resolve every critical finding.

**Verification:** Run the security, compatibility, schema, cache, artifact, and
cross-interface suites, then rerun T3 and T4 after fixes.

## Milestone: Distribution readiness

**Exit condition:** The audited candidate is reproducibly packaged and its
installed artifacts pass representative analysis and export workflows.

### T7 — Build the distribution candidate

**What to build:** Every supported platform produces the same complete,
checksummed package from the audited source.

**Blocked by:** T5 and T6.

**Acceptance criteria:**

- [ ] Pass Linux, macOS, Windows, Rust 1.85, dependency-policy, schema,
      generated-asset, and package-content jobs on the candidate build.
- [ ] Produce checksummed archives for every supported distribution target.
- [ ] Confirm packaged completions, man pages, licenses, schemas, agent
      instructions, and repository-artifact documentation match the binary.
- [ ] Package publishable workspace crates in dependency order while preserving
      `cargo install dalil` as the CLI installation path.
- [ ] Confirm a clean rebuild reproduces package contents and metadata.

**Verification:** Run the workspace, package, release-asset, and platform jobs
against the candidate commit and inspect every archive.

### T8 — Validate packaged installation and behavior

**What to build:** Users can install, use, and remove the packaged binary with
no unresolved release blocker.

**Blocked by:** T7.

**Acceptance criteria:**

- [ ] Verify install, uninstall, user-cache cleanup, and repository-bundle
      cleanup instructions from packaged artifacts in clean environments.
- [ ] Run representative orientation, map, context, impact, search, explain,
      cache, capabilities, export, refresh, and task-record workflows from the
      installed binary.
- [ ] Confirm normal successful runs keep stderr empty except for documented
      repository-write notices and all promised output modes remain
      machine-readable.
- [ ] Record associated benchmark and quality-review results using public or
      tracked artifacts.
- [ ] Resolve every installation, packaging, or behavior blocker found during
      candidate validation.

**Verification:** Install each package in a clean environment, run the
representative workflows, remove it using the documented steps, and compare
its outputs with the audited source build.

## Milestone: Evidence-driven extensions

**Exit condition:** A measured failure or demonstrated demand supports each
optional extension before implementation work begins.

### T9 — Assess additional language support

**What to build:** A specific language gap produces either a fixture-backed
implementation ticket or a documented decision not to add support.

**Blocked by:** T3.

**Acceptance criteria:**

- [ ] Identify demand from benchmark failures, issue evidence, or a named
      integration rather than ecosystem popularity alone.
- [ ] Define symbol, relationship, manifest, entry-point, test, ambiguity, and
      exported-map behavior before implementation.
- [ ] Require conformance, malformed-source, generated-code, mixed-language,
      and export fixtures for first-class support.
- [ ] Record expected quality and maintenance costs before adding a grammar.

**Verification:** Review the evidence and fixture expectations before creating
an implementation ticket.

### T10 — Assess semantic or framework providers

**What to build:** An optional provider is considered only when bounded lexical
and syntax evidence fails a representative task and the provider materially
improves it.

**Blocked by:** T3.

**Acceptance criteria:**

- [ ] Name the benchmark failures the provider must fix and the operations or
      exported relationships that consume its evidence.
- [ ] Keep lexical and syntax analysis as the deterministic fallback.
- [ ] Define offline behavior, dependency cost, cache and artifact identity,
      provenance, confidence, timeouts, and partial-failure handling.
- [ ] Label framework conventions as framework evidence rather than
      compiler-resolved semantics.
- [ ] Reject providers that require repository-controlled execution or a remote
      core dependency.

**Verification:** Compare the provider on focused public or synthetic fixtures
and approve it only when the gain exceeds its latency and maintenance cost.

### T11 — Assess distributable query packs

**What to build:** The project has evidence for or against loading query-pack
updates independently from the main binary.

**Blocked by:** T3 and T9.

**Acceptance criteria:**

- [ ] Define the update problem that built-in packs cannot solve.
- [ ] Specify compatibility, trust, cache and exported-map identity, offline
      fallback, rollback, and failure isolation before implementing a loader.
- [ ] Preserve a complete built-in baseline when no external pack is available.
- [ ] Never contact a registry or install packs without an explicit user action.
- [ ] Proceed only if the update benefit outweighs the supply-chain and
      compatibility surface.

**Verification:** Review the threat model and a local proof of concept before
creating production implementation tickets.
