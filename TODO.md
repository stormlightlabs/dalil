# To-Do/Task List

## Milestone: Task-relevant retrieval

**Exit condition:** Dalil returns and explains a small, diverse set of files
and symbols for realistic orientation and implementation tasks.

### T1 — Make ranking task-aware

A user's task changes the recommended files and symbols in a deterministic,
explainable way.

### T2 — Select a diverse, bounded file set

Recommendations cover the useful roles and project roots for a task without
padding the result with weak files.

### T3 — Turn explanations into reading guidance

`explain` tells the user why to read an item, what evidence supports
it, and what to inspect next.

## Milestone: Task-oriented context bundles

**Exit condition:** CLI consumers can request a task, symbol, path, project, or
change and receive one bounded context bundle with orientation, relevant code,
relationships, tests, history, uncertainty, and next reads.

### T4 — Deliver the context bundle end to end

One typed request and one typed result serve orientation,
implementation, debugging, refactoring, and review workflows.

### T5 — Add a source-based teaching scaffold

A context bundle can optionally explain an unfamiliar subsystem in
a concise sequence grounded in repository evidence.

**Verification:** Added fixtures for a clear entry flow, multiple plausible
entry points, and insufficient evidence. The CLI fixture verifies that every
teaching observation points to evidence already selected in the JSON bundle.

## Milestone: Change-aware review context

**Exit condition:** Dalil can compare revisions or inspect a dirty worktree
and return bounded, explicitly uncertain context about changed symbols, nearby
dependencies, relevant tests, ownership, and history.

### T6 — Resolve change inputs safely

Callers can name a revision range or dirty worktree without losing
read-only guarantees or receiving an ambiguous change set.

### T7 — Return impact context

Reviewers receive evidence-backed inspection targets around a
change without a prediction that the change will break them.

## Milestone: Primary CLI workflows

**Exit condition:** A human or agent can orient, map, search, and request
task-shaped context without knowing about history aggregates, internal rankings,
or caches.

### R2 — Add first-class orientation

`dalil` and `dalil orient` give a concise, typed answer about where
to start without exposing the complete repository analysis by default.

### R3 — Add bounded search

**Outcome:** A user can find a few strong path, symbol, or concept anchors for a
subsequent context request or source read.

**Blocked by:** R2.

**Acceptance criteria:**

- [x] Accept plain queries and an explicit symbol query through the CLI and
      typed operation.
- [x] Return a small ranked set of matching paths and symbols with a reason,
      evidence, confidence, and limitations for each result.
- [x] Reuse the recommendation fields already needed by orientation, context,
      impact, and explain where they fit instead of introducing parallel result
      meanings.
- [x] Include a directly related file or test only when it helps anchor the
      next read.
- [x] Apply one result and token budget with deterministic ordering and an
      explicit shortfall when strong matches do not exist.
- [x] Do not expose graph traversal, callers, paths, centrality, or a general
      graph query language as public search modes.
- [x] Lead the quick start with orientation, repository mapping, task context,
      change impact, search, and explanation examples.
- [x] Keep focused `history` usage in an evidence-inspection section while
      retaining its documented behavior.
- [x] Keep README usage concise and link to detailed guides where needed.
- [x] Keep help, README examples, generated man pages, and completions aligned
      with the packaged command surface.

**Verification:** CLI coverage exercises path, exact-symbol, concept,
ambiguous-symbol, missing, and budget-limited queries in Markdown and JSON.
The release-asset generator verifies help, man pages, and completions use the
packaged command and option spelling.

## Milestone: Incremental analysis

**Exit condition:** Repeated requests reuse a persistent analysis index, refresh
only invalidated work, and remain equivalent to a cold analysis.

### T8 — Persist the analysis index

**Outcome:** Expensive reusable facts survive between processes under the user
cache without changing repository contents.

**Blocked by:** T4.

**Acceptance criteria:**

- [x] Define a versioned on-disk index for file fingerprints, bounded syntax
      summaries, lexical edges, selected history facts, and repository metadata.
- [x] Key index identity by repository, worktree, revision or content state,
      analysis options, language-pack identity, and tool compatibility.
- [x] Write atomically under the user cache and recover safely from missing,
      partial, corrupt, incompatible, or permission-denied state.
- [x] Preserve explicit cache status, provenance, cleanup, and no-cache controls.
- [x] Do not persist source content unless a separately approved requirement
      proves it necessary.

**Verification:** Round-trip, corruption, concurrent-writer, permission,
cleanup, and cold/warm-equivalence coverage runs through the CLI cache boundary.
The index uses the existing private, atomic cache writer and a distinct index
status in map and context provenance.

### T9 — Refresh only invalidated analysis

**Outcome:** A small repository change causes proportionate reanalysis while
unchanged results remain reusable.

**Blocked by:** T8 and R3.

**Acceptance criteria:**

- [x] Detect file, manifest, revision, worktree, option, and language-pack
      changes that invalidate stored facts.
- [x] Reparse changed files and recompute only dependent lexical, structural,
      history, and ranking state.
- [x] Fall back to cold analysis when invalidation cannot be proven safe.
- [x] Report reused, refreshed, stale, bypassed, and failed cache state without
      making output nondeterministic.
- [x] Demonstrate lower warm-request latency and work without crossing memory or
      disk ceilings.

**Verification:** Mutate one input category at a time, compare every warm result
with a cold run, and record targeted cold/warm work and latency measurements.

## Milestone: Shared integration surfaces

**Exit condition:** CLI, MCP, agent, and native consumers call the same bounded
analysis operations and cannot drift in selection, evidence, uncertainty, or
safety behavior.

### T10 — Establish an embeddable core

**Outcome:** In-process consumers can call stable analysis operations without
depending on CLI parsing or renderer internals.

**Blocked by:** T4, T7, and R3.

**Acceptance criteria:**

- [x] Expose orientation, map, context, impact, explain, search, capabilities,
      and cache operations through typed inputs and outputs.
- [x] Extract analysis and reusable storage into a `dalil-core` workspace crate.
      Keep the `dalil` CLI package in `crates/dalil-cli` and preserve
      `cargo install dalil`.
- [x] Separate analysis, storage, rendering, and transport boundaries while
      reusing current domain types where they already fit.
- [x] Keep dependencies directed from the CLI into `dalil-core`; the core does
      not depend on CLI parsing, rendering, transport, or protocol code.
- [x] Keep internal graph and parser structures private unless a demonstrated
      consumer needs them.
- [x] Define cancellation, progress, budget, warning, and error behavior for
      long-running calls.
- [x] Prove CLI output still comes from the same operations.

**Verification:** Add library integration tests and compare library-derived
results with black-box CLI JSON fixtures.

### T11 — Add a bounded MCP adapter

**Outcome:** MCP clients can request Dalil context through a small task-level
tool surface.

**Blocked by:** T10.

**Acceptance criteria:**

- [x] Implement MCP as a separate workspace crate that depends on `dalil-core`,
      keeping MCP protocol and runtime dependencies out of the core and CLI.
- [x] Map MCP tools to orientation, repository map, context, impact, explain, search,
      capabilities, and cache-status operations.
- [x] Keep responses task-oriented and bounded; do not expose an exhaustive
      graph endpoint.
- [x] Preserve typed provenance, uncertainty, omissions, budget behavior, and
      cancellation across the transport.
- [x] Document local setup and the read-only safety contract.
- [x] Verify MCP and CLI requests with equivalent inputs return equivalent
      semantics.

**Verification:** Add protocol conformance, cancellation, oversized-request,
error-mapping, and CLI-equivalence tests.

### T12 — Ship an agent skill

**Outcome:** Coding agents receive concise instructions for when and how to use
Dalil instead of rediscovering repository exploration workflows.

**Blocked by:** R3.

**Acceptance criteria:**

- [ ] Cover unfamiliar-repository orientation and mapping, implementation
      lookup, impact review, relevant-test discovery, and next-read workflows.
- [ ] Prefer compact requests, narrow focus, and follow-up calls over exhaustive
      output.
- [ ] Explain uncertainty, unsupported-language behavior, and when direct source
      inspection remains necessary.
- [ ] Keep examples valid for the packaged CLI and avoid host-specific claims.
- [ ] Validate the skill against representative benchmark tasks.

**Verification:** Run the documented prompts against public or synthetic
fixtures and confirm every command, option, and expected field matches the
packaged CLI.

### T13 — Add bounded host lifecycle adapters

**Outcome:** Approved editors or coding hosts can inject fresh Dalil context
at useful lifecycle points without hidden background behavior.

**Blocked by:** T9 and T10.

**Acceptance criteria:**

- [ ] Define explicit events such as repository open, session start, task
      change, before edit, after edit, and before review.
- [ ] Keep every injection small, advisory, cancellable, and controlled by the
      host.
- [ ] Share the persistent index across lifecycle calls without requiring a
      daemon.
- [ ] Add only adapters with a named host, demonstrated demand, and a stable
      integration boundary.
- [ ] Give an approved adapter its own workspace crate only when it adds
      protocol or runtime dependencies or has a separate executable boundary.
- [ ] Preserve identical analysis semantics across native, CLI, and MCP paths.

**Verification:** For each approved adapter, add lifecycle, cancellation,
stale-state, and semantic-equivalence tests before documenting support.

## Milestone: Evidence-driven extensions

**Exit condition:** New language, semantic, framework, or distribution
capabilities are admitted only when measured failures or demonstrated demand
justify their cost and their contracts remain explicit.

### T14 — Assess additional language support

**Outcome:** A specific language gap either produces an approved, fixture-backed
implementation ticket or a documented decision not to add support.

**Blocked by:** Nothing.

**Acceptance criteria:**

- [ ] Identify demand from task failures, issue evidence, or a named integration
      rather than ecosystem popularity alone.
- [ ] Define required symbol, relationship, manifest, entry-point, test, and
      ambiguity behavior before implementation.
- [ ] Require conformance, malformed-source, generated-code, and mixed-language
      fixtures for first-class support.
- [ ] Record expected quality and maintenance costs before adding a parser or
      grammar dependency.

**Verification:** Review the evidence and proposed fixture contract before
creating any language implementation ticket.

### T15 — Assess semantic or framework providers

**Outcome:** An optional provider is added only when bounded lexical and syntax
evidence fails a representative task and the provider materially improves it.

**Blocked by:** T4.

**Acceptance criteria:**

- [ ] Name the retrieval or context failures the provider must fix and the
      operations that consume its evidence.
- [ ] Keep lexical and syntax analysis as the deterministic fallback.
- [ ] Define offline behavior, dependency cost, cache identity, provenance,
      confidence, timeouts, and partial-failure handling.
- [ ] Label framework conventions as framework evidence, not compiler-resolved
      semantics.
- [ ] Reject providers that require repository-controlled execution or make the
      core depend on a remote service.

**Verification:** Compare the provider on focused public or synthetic fixtures
and approve it only when the gain exceeds its latency, complexity, and
maintenance cost.

### T16 — Assess distributable query packs

**Outcome:** The project has evidence for or against loading query-pack updates
independently from the main binary.

**Blocked by:** T8 and T14.

**Acceptance criteria:**

- [ ] Define the concrete update problem that built-in packs cannot solve.
- [ ] Specify compatibility, signing or trust, cache identity, offline fallback,
      rollback, and failure isolation before implementing a loader.
- [ ] Preserve a complete safe built-in baseline when no external pack is
      available.
- [ ] Do not contact a registry or install packs without an explicit user
      action.
- [ ] Proceed to implementation only if the update benefit outweighs the new
      supply-chain and compatibility surface.

**Verification:** Review the threat model and a local proof of concept before
creating production implementation tickets.

## Milestone: Context and scale benchmarking

**Exit condition:** Public or synthetic benchmarks measure retrieval quality,
context efficiency, latency, work, memory where measurable, and output size
across the completed feature set.

### T17 — Build the context-quality benchmark

**Outcome:** Retrieval and context changes can be compared against a
reproducible baseline instead of judged from a few repository examples.

**Blocked by:** T1–T16 and R2–R3.

**Acceptance criteria:**

- [ ] Define fixtures for orientation, implementation search, behavior
      tracing, bug fixes, feature extensions, refactors, change review, and
      relevant-test discovery.
- [ ] Use public or synthetic repositories with expected useful paths or
      graded path rankings for each task.
- [ ] Measure useful-file recall, precision, ranking quality, project-root
      coverage, and relevant-symbol recall.
- [ ] Measure returned tokens, useful files per 1,000 tokens, redundant
      evidence, and the share of budget consumed by one file.
- [ ] Record qualitative failure labels for missing entry points, duplicated
      roles, irrelevant central files, weak explanations, and token waste.
- [ ] Capture the completed feature set as a deterministic baseline and define
      regression thresholds suitable for local and CI use.

**Verification:** Run the corpus twice from cold state and once from valid warm
state; confirm stable metrics, equivalent results, and actionable fixture-level
failures.

### T18 — Establish performance and scale gates

**Outcome:** CI detects unacceptable latency, work, memory, or output growth
before a release candidate is built.

**Blocked by:** T1–T16 and R2–R3.

**Acceptance criteria:**

- [ ] Add CI-friendly cases for Dalil, a large ignored or vendor tree,
      high-ambiguity sources, large monorepos, and synthetic 10k- and
      100k-commit histories.
- [ ] Exercise cold analysis, valid warm analysis, targeted invalidation,
      context requests, impact requests, and every supported integration path.
- [ ] Define latency, work, memory where measurable, disk, and output ceilings.
      A failure identifies the exceeded dimension.
- [ ] Keep every benchmark public or synthetic, deterministic, bounded, and
      runnable without network access.
- [ ] Define representative CI tiers so routine checks remain proportionate
      while the full scale suite still runs before distribution.

**Verification:** Run the harness under its documented CI ceilings and confirm
intentional regressions fail with the expected dimension and fixture name.

## Milestone: Product quality review

**Exit condition:** The completed product passes cross-cutting correctness,
security, compatibility, usability, and semantic-consistency review with no
unresolved critical finding.

### T19 — Review usefulness and semantic consistency

**Outcome:** The completed feature set gives useful, consistent answers across
representative repositories and every supported interface.

**Blocked by:** T17 and T18.

**Acceptance criteria:**

- [ ] Apply a reusable review rubric to a small project, Dalil, a mixed
      monorepo, a generated-heavy repository, and representative change sets.
- [ ] Review reading-plan coverage and reasons, context usefulness, impact
      uncertainty, history usefulness, quality semantics, and token economy.
- [ ] Confirm equivalent requests have equivalent semantics across CLI,
      library, MCP, agent-skill, and approved native integration paths.
- [ ] Run the completed binary across the available project corpus and retain
      only aggregate outcomes plus public or synthetic regressions.
- [ ] Convert confirmed correctness failures into focused regression tests and
      resolve every critical usability finding.

**Verification:** Complete the rubric, inspect aggregate corpus outcomes, and
rerun T17 and T18 after fixes.

### T20 — Audit safety and compatibility

**Outcome:** The completed feature set preserves Dalil's read-only, bounded,
deterministic, and compatible behavior under hostile and partial conditions.

**Blocked by:** T17 and T18.

**Acceptance criteria:**

- [ ] Exercise hostile paths, external filters, malformed source, corrupt cache,
      unsupported languages, partial history, cancellation, and output limits.
- [ ] Confirm every interface reports partial work, uncertainty, omissions, and
      compatibility failures consistently.
- [ ] Verify cold, warm, invalidated, and no-cache results remain semantically
      equivalent where repository state is unchanged.
- [ ] Confirm schema fixtures, public library types, MCP responses, and generated
      assets match their documented compatibility contracts.
- [ ] Convert every confirmed correctness or security failure into a focused
      regression test and resolve every critical finding.

**Verification:** Run the security, compatibility, schema, cache, and
cross-interface suites, then rerun T17 and T18 after fixes.

## Milestone: Distribution readiness

**Exit condition:** The audited candidate is reproducibly packaged and ready
for distribution on every supported platform.

### T21 — Build the distribution candidate

**Outcome:** Every supported platform produces the same complete, checksummed
package from the audited source.

**Blocked by:** T19 and T20.

**Acceptance criteria:**

- [ ] Pass Linux, macOS, Windows, Rust 1.85, dependency-policy, schema,
      generated-asset, and package-content jobs on the candidate build.
- [ ] Produce checksummed archives for every supported distribution target.
- [ ] Confirm packaged completions, man pages, licenses, schemas, and agent
      instructions match the candidate binary.
- [ ] Package publishable workspace crates in dependency order while preserving
      `cargo install dalil` as the CLI installation path.
- [ ] Confirm a clean rebuild reproduces the expected package contents and
      metadata.

**Verification:** Run the standard workspace, package, release-asset, and
platform jobs against the candidate commit and inspect every archive.

### T22 — Validate packaged installation and behavior

**Outcome:** Users can install, run, and remove the packaged binary using the
documented instructions and encounter no unresolved release blocker.

**Blocked by:** T21.

**Acceptance criteria:**

- [ ] Verify install, uninstall, and cache-cleanup instructions from packaged
      artifacts in clean environments.
- [ ] Run representative orientation, map, context, impact, search, explain,
      cache, and capabilities workflows from the installed binary.
- [ ] Confirm normal successful runs keep stderr empty and all supported output
      modes remain machine-readable where promised.
- [ ] Record the benchmark and quality-review results associated with the
      candidate without relying on private or untracked documents.
- [ ] Resolve every installation, packaging, or behavior blocker found during
      candidate validation.

**Verification:** Install the packaged artifacts in clean environments, run
representative workflows, remove them using the documented steps, and compare
their outputs with the audited source build.
