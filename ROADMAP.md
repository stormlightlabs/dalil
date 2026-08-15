---
title: "Dalil roadmap"
status: "in-progress"
updated: "2026-08-15"
---

Dalil is a deterministic context compiler for software repositories. It turns
repository structure, history, task signals, and current changes into a small,
evidence-backed map of what a person or coding agent should inspect next.

The product begins with repository orientation and grows through milestones.
Milestones define the scope and order of the work.

Dalil should answer:

> Given this repository, task, current changes, and context budget, what is the
> smallest useful set of evidence someone should inspect next?

## Users and Use Cases

- A developer entering an unfamiliar repository can get a useful first reading
  sequence without already knowing its language or layout.
- A coding agent can request task-specific context within a fixed budget and
  decide whether the evidence is sufficient before exploring broadly.
- A maintainer can inspect the likely reach of a change, relevant tests, and
  supporting history without treating lexical evidence as semantic proof.
- A monorepo user can see relevant project roots without one central file or
  unrelated package monopolizing the result.
- An agent host can integrate through the CLI, MCP, or a native library while
  receiving the same typed evidence and quality semantics.

## Success Criteria

- The default briefing leads with a short, useful reading sequence and a few
  distinct history observations rather than exhaustive analysis tables.
- Task and focus inputs materially affect recommended files, symbols, reading
  order, and supporting evidence. A query with no useful match says so plainly.
- Context requests return a diverse, answer-shaped bundle within the caller's
  budget, with a reason, evidence, confidence, and limitations for each
  recommendation.
- Change analysis identifies direct structural relationships, relevant tests,
  runtime paths, and history evidence without predicting whether code will
  break.
- Warm analysis can reuse a persistent user-cache index while preserving fresh
  provenance, invalidation, quality, and stale-cache reporting.
- CLI, MCP, and native consumers use the same analysis operations and cannot
  drift semantically.
- Retrieval changes are measured against realistic repository tasks for useful
  path recall, precision, ranking, diversity, and token efficiency.
- Every interface remains read-only, bounded, deterministic, offline-capable,
  explicit about uncertainty, and safe against hostile repository state.

## Strategic Principles

### Optimize for context selection

A complete dependency graph is not the product. Structural and history evidence
exists to answer where to start, what matters to a task, what surrounds a
symbol, what a change may affect, which tests deserve inspection, and what to
read next.

Add graph or analysis types only when they improve those decisions.

### Keep analysis deterministic and local

The core engine must remain useful without an LLM, network service, daemon, or
repository-controlled execution. LLMs belong in the consuming agent, not in the
analysis pipeline.

### Let task context shape ranking

Task signals must influence graph personalization and selection, rather than
acting only as a boost after global ranking. Inputs are explicit and typed:

- task text;
- focus paths and symbols;
- identifiers supplied by the caller;
- changed paths and symbols;
- paths already inspected;
- relevant project roots;
- desired evidence families and remaining context budget.

The host interprets conversations and editor state. Dalil does not scrape
either source.

### Prefer task-shaped operations

Consumers should request outcomes through `orient`, `context`, `impact`,
`explain`, and `search`. Low-level edges, symbols, rankings, and landmarks remain
typed evidence and internal library operations where needed.

### Treat budgets as part of ranking

Budgeting is a selection problem, not tail truncation. Results balance direct
task matches, important files, relevant declarations, dependencies, tests,
runtime paths, history, and supporting evidence. Breadth and diversity are
requirements when several files are needed to understand the task.

## Product Contract

### Current Command Surface

```text
dalil [OPTIONS] [PATH]
dalil map [OPTIONS] [PATH]
dalil history [OPTIONS] [PATH]
dalil history <churn|contributors|bugs|activity|firefighting> [OPTIONS] [PATH]
dalil explain [OPTIONS] <PATH-OR-SYMBOL> [PATH]
dalil cache <path|status|prune|clear>
dalil capabilities [--json]
dalil doctor [OPTIONS] [PATH]
```

- `PATH` defaults to the current directory and must resolve within the
  discovered worktree.
- Markdown is the human default. `--json` is shorthand for `--format json` and
  is the portable machine contract.
- `--focus` and `--focus-path` are the current task-personalization inputs and
  may be repeated.
- `--profile compact|evidence` defaults to `compact`. Both profiles are bounded.
- `--budget`, cache controls, exclusions, recursive traversal, color policy,
  strictness, and exit categories retain their documented behavior.

The planned task-shaped operations extend this surface without creating a
second analysis pipeline:

```text
dalil orient [OPTIONS] [PATH]
dalil context --task <TEXT> [OPTIONS] [PATH]
dalil impact <REVISION-RANGE> [OPTIONS] [PATH]
dalil search <PATH-OR-SYMBOL> [OPTIONS] [PATH]
```

The default command may remain the concise orientation entry point. Exact CLI
spelling and compatibility aliases are settled when each operation is designed.

### Orientation Briefing

The default Markdown order is:

1. Repository overview: project roots, primary supported languages, worktree
   state, and important instructions or manifests.
2. Reading plan: 3–5 strong paths when that much evidence exists, grouped
   under `start_here`, `architecture`, `runtime`, `tests`, and
   `supporting_context`.
3. History observations: 3–5 bounded statements that identify useful evidence
   without converting heuristics into quality judgments.
4. Limitations that materially affect the briefing, followed by a concise
   pointer to focused commands or the evidence profile.

Categories with no evidence are omitted. Recommendations are unique across
categories. A monorepo balances relevant project-root coverage with task
relevance. When fewer than three useful paths exist, the report explains the
shortfall instead of padding the plan.

The default report does not print exhaustive history tables, symbol lists,
omissions, parser diagnostics, or commit lists. Focused commands, JSON, and the
evidence profile preserve those workflows.

### Ranking and Selection

Recommendations combine typed evidence:

- landmarks such as instructions, README and contributor guidance, manifests,
  entry points, tests, CI, and ownership;
- source evidence such as public definitions, qualified lexical edges,
  centrality, task matches, and declaration snippets;
- project-root topology and nested repository or submodule boundaries;
- bounded history overlap used only as supporting evidence;
- explicit task, focus, change, and inspected-path seeds.

Ranking should personalize graph traversal from the supplied seeds. Selection
then ranks candidate files, chooses a diverse file set, allocates budget between
files and evidence categories, and selects relevant declarations within each
file. A deterministic novelty rule or equivalent mechanism prevents a single
central file from consuming the result.

Each recommendation records its purpose, ordinal, path, project root, concise
reason, evidence kinds, confidence, relevant limitations, and ranking seeds. A
caller must not need to reverse engineer an opaque score.

### Context Compilation

The context operation accepts structured equivalents of:

```text
task
focus paths and symbols
changed paths or revision
already inspected paths
budget
desired evidence families
```

It composes current Dalil evidence into an answer-shaped `ContextBundle`:

```text
ContextBundle
├── task interpretation evidence
├── recommended files, reasons, symbols, and bounded snippets
├── immediate structural context
├── relevant runtime or entry path
├── likely tests
├── repository instructions
├── relevant recent history
├── ambiguities and limitations
└── suggested next reads
```

Every recommendation retains typed evidence and confidence. Markdown and JSON
render the same bundle, and the configured budget applies to the whole result.

### Change and Impact Context

`impact` and change-aware `context` start from safely resolved revision ranges,
changed paths, and changed symbols when source locations make them detectable.
They may surface:

- direct dependents and affected public interfaces;
- relevant tests and runtime or entry paths;
- historical co-change evidence;
- a recommended inspection sequence;
- uncertainty caused by ambiguous or partial evidence.

The output supports navigation and review. It says that a path depends on a
changed symbol through lexical evidence or that tests reference a module; it
does not claim that a change will break code.

### Explain and Search

`explain` begins with the requested path or symbol, states why it matters to the
current task, shows the connecting evidence, recommends the next useful file,
and names remaining uncertainty. It does not repeat the full repository report
before answering.

`search` finds paths and symbols when a task cannot yet be anchored to known
targets. Richer graph queries remain internal unless repeated consumer needs
justify a public operation.

### History Observations

The five history families remain authoritative evidence inputs:

- churn hotspots;
- contributor concentration;
- bug-keyword paths and churn overlap;
- monthly activity;
- firefighting-keyword commits.

Default observations are selected for distinctness and usefulness. They retain
caveats about commit-message discipline, squash merges, exact-path rename
continuity, current-HEAD normalization, and activity not being a quality score.
Empty or noisy evidence is summarized honestly and does not fill a quota.

### Quality, Strictness, and Schema Compatibility

Collection summaries retain `total`, `returned`, `truncated`, and a reason.
Top-level quality records whether the requested result may be unsafe or
materially misleading.

- Expected compact projection stays at the collection level and does not alone
  make a report degraded.
- Resource ceilings, elapsed-work interruption, missing Git objects, stale
  manual cache, unsafe paths, unsupported relevant source, and partial analysis
  of recommended or focused files are actionable quality conditions.
- Partial or unsupported files outside the selected result remain visible in
  totals and bounded samples without automatically poisoning useful output.
- `--strict` follows actionable quality, emits the typed report, and then
  returns the documented analysis-failure status.

Schema changes are additive only when old fields retain their meaning. Semantic
reuse or retyping requires a schema revision and an updated compatibility
corpus. Schema revisions are independent of release milestone placement.

### Generated, Vendored, and Minified Source

Compact analysis classifies low-value source before parsing:

- conventional generated, vendor, and dependency directories are pruned;
- generated filenames, generated-file markers, source maps, and `.min.*` files
  are classified;
- a documented bounded heuristic may classify text with very low whitespace
  and high line length;
- tracked status never grants generated or vendored source priority over
  maintained source.

Compact mode records typed counts and bounded samples but does not parse or
recommend classified files. An exact focus path may include them within all
safety and resource limits. Focus never overrides unsafe-path, binary, or hard
resource protections.

### First-Class Language Support

The current first-class languages are Rust, JavaScript/JSX, TypeScript/TSX,
Python, Ruby, Java, C#, Go, Lua, and Zig.

Each language needs an upstream Tree-sitter grammar, embedded versioned
definition and reference queries, stable extension registration, visibility and
import evidence where the grammar exposes it, malformed-input behavior, and
black-box Markdown/JSON fixtures. A definition-only limitation is permitted
only when reports state it and no false lexical edges are created.

Additional languages require corpus or user evidence that justifies their
maintenance cost. Change-aware context takes priority over speculative language
expansion.

### Trust, Scope, and Persistent Analysis

- Target repositories remain read-only. Dalil never invokes system Git,
  hooks, filters, credentials, editors, pagers, repository commands, or network
  transports.
- Tree, index, walk, and cache paths remain untrusted until validated. Worktree
  reads do not follow symlinks or reparse points and stay beneath the selected
  scope.
- Cache data lives only in the user cache directory, uses private permissions
  and atomic writes, and remains controllable through cache commands.
- All file, byte, syntax-depth, symbol, candidate, edge, finding, commit,
  elapsed-time, and output ceilings apply to every profile and interface.

A persistent index may store file identities and hashes, query-pack identity,
symbols, lexical edges, landmarks, project roots, manifest evidence, HEAD
identity, bounded history aggregates, and reusable rankings. Each query checks
cached provenance, refreshes changed analysis, recomputes affected evidence,
and reports fresh quality. The first implementation does not require a daemon.

## Integration Contract

### Embeddable Core

The CLI should become one adapter over typed analysis operations:

```rust
analyze(request) -> RepositoryReport
orient(request) -> OrientationReport
context(request) -> ContextBundle
impact(request) -> ImpactReport
explain(request) -> Explanation
search(request) -> SearchResults
```

Rendering stays outside the analysis core where practical. The compiled CLI
remains the highest-level black-box compatibility boundary even after a library
API is available.

### Agent Interfaces

- The CLI remains the universal interface for humans, shell-capable agents,
  scripts, CI, unsupported hosts, and integration debugging.
- MCP exposes only the task-shaped operations and preserves response budgets,
  provenance, uncertainty, and quality metadata.
- Native hosts prefer the core library when they can pass task text, inspected
  files, edited files, identifiers, budget, and worktree changes directly.

A small agent skill teaches consumers when to use each operation and when to
fall back to source search. Dalil narrows exploration; it does not replace
reading source.

Lifecycle adapters may offer a small orientation notice at session start, pass
typed task hints before exploration, invalidate or refresh analysis after edits,
and request impact context at the review boundary. Hooks remain advisory and
bounded, and they do not inject a large report after every write.

## Technical Plan

### Stack and Architecture

- Rust edition 2024 with MSRV 1.85.
- Clap for the typed CLI; serde and serde_json for report models; Tree-sitter
  grammar crates for language analysis; `gix` with reviewed minimal features
  for repository data; `ignore` for bounded traversal.
- Preserve clear boundaries between request parsing, repository security,
  history, landmarks and topology, language queries, cache/index storage,
  lexical ranking, report models, and rendering.
- Build every report from typed intermediates. Do not parse rendered Markdown
  or duplicate analysis for different interfaces.
- Add dependencies only for reviewed grammar support or when the standard
  library and current stack are insufficient.

### Testing Boundary

The highest stable boundary is the compiled CLI running against fixture
repositories. Unit tests cover query captures, classification, ranking,
selection, invalidation, and rendering but do not replace black-box assertions.

Required coverage includes:

- semantic Markdown snapshots and JSON assertions for small projects,
  Dalil, mixed-language repositories, and monorepos;
- conformance fixtures for every first-class language;
- generated, vendored, minified, malformed, high-ambiguity, ignored-tree,
  hostile-path, stale-cache, change-range, and deep-history cases;
- focused and task-personalized ranking, file diversity, budget allocation,
  next-read explanations, context bundles, and impact reports;
- schema compatibility, repeated-run determinism, no ANSI on stdout, empty
  non-interactive stderr, documented exits, and quiet broken pipes;
- cold and warm benchmarks that enforce latency, work, output, and cache
  invalidation limits.

Private project contents and paths are not committed as fixtures. Only aggregate
outcomes and reproducible public or synthetic regressions may enter the
repository.

### Evaluation

A public benchmark corpus should cover orientation, implementation search,
behavior tracing, bug fixes, feature extensions, refactors, change review, and
relevant-test discovery. Each task records an expected set or graded ranking of
useful paths.

Measure:

- useful-file recall, precision, ranking quality, project-root coverage, and
  relevant-symbol recall;
- returned tokens, useful files per 1,000 tokens, redundant evidence, and the
  share of budget consumed by one file;
- eventually, changes in source-search calls, full-file reads, repository
  reading tokens, tool calls, and irrelevant files inspected by agents.

The useful baseline is what a capable coding agent would otherwise explore,
not the size of the whole repository.

### Required Commands

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo package --locked
cargo release-assets
```

CI also verifies the schema corpus, dependency policy, minimal `gix` feature
graph, generated release assets, Linux, macOS, Windows, and Rust 1.85.

## Boundaries

- **Always:** preserve read-only behavior, typed evidence, deterministic
  ordering, bounded work and output, explicit uncertainty, shared analysis
  across renderers and adapters, and black-box CLI verification.
- **Ask first:** add a non-grammar dependency, change schema semantics, broaden
  language promises, alter cache retention or location, relax safety limits, or
  add framework-specific inference.
- **Never:** execute repository-controlled programs, follow paths outside scope,
  contact remotes, scrape editor or chat context, write inside target
  repositories, describe lexical evidence as semantic resolution, or hide
  omitted or partial analysis to improve apparent quality.

Dalil is not an autonomous coding agent, documentation generator, vector
database, project wiki, architectural memory engine, source host, general code
quality score, language-server replacement, universal compiler-grade analyzer,
mandatory daemon, or exhaustive MCP graph server.

## Risks and Open Questions

- Task text must influence deterministic ranking without embedding an LLM in the
  core. The typed seed model and lexical interpretation rules need a narrow,
  explainable contract.
- A target recommendation count can encourage weak filler. Results must prefer
  an explicit shortfall over low-confidence paths.
- Project-root coverage can conflict with task focus. Relevant focus wins, and
  the report records why a root received no recommendation.
- Diversity rules can hide the strongest file or waste budget on novelty. The
  benchmark must test both coverage and precision.
- Changed-symbol and impact evidence inherits lexical and parser ambiguity.
  Reports must keep those limitations beside each relationship.
- Incremental invalidation can return plausible stale results. Cache identity,
  dependency invalidation, and stale-analysis behavior need adversarial tests.
- Stable library and MCP models raise compatibility costs. Public types should
  follow demonstrated consumer needs rather than expose internal graph detail.
- Lifecycle hooks can become noisy or coercive. Keep injections small, advisory,
  and owned by the host.
- Real-project usefulness remains partly subjective. Reviews record concrete
  failure modes and recommendation coverage rather than one opaque score.

## Reference Material

- [Implementation milestones](TODO.md)
- [Research notes](notes/README.md)
- [JSON schema](schema/v1/dalil.json)
