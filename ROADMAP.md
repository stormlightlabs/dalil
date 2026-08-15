---
title: "Dalil roadmap"
status: "in-progress"
updated: "2026-08-16"
---

Dalil is a local codebase reference engine for humans and coding agents. Its
deterministic context compiler turns repository structure, history, task
signals, and current changes into a small, evidence-backed set of code and
repository evidence worth inspecting next.

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
- The primary CLI surface is `orient`, `map`, `context`, `impact`, `search`, and
  `explain`; users do not need to understand history aggregates, internal
  rankings, or caches to use it.
- Adding internal analysis does not expand default output unless it improves
  the selected references.
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

Consumers should request outcomes through `orient`, `map`, `context`, `impact`,
`explain`, and `search`. `map` provides a bounded structural overview that can
replace broad exploratory file reads. Low-level edges, symbols, rankings, and
landmarks remain typed evidence and internal library operations where needed.

Focused `history` commands remain available for deeper evidence inspection.
Cache, capability, and health commands remain maintenance tools rather than
primary ways to work with a codebase.

### Treat budgets as part of ranking

Budgeting is a selection problem, not tail truncation. Results balance direct
task matches, important files, relevant declarations, dependencies, tests,
runtime paths, history, and supporting evidence. Breadth and diversity are
requirements when several files are needed to understand the task.

## Product Contract

### Current Command Surface

```text
dalil [OPTIONS] [PATH]
dalil orient [OPTIONS] [PATH]
dalil map [OPTIONS] [PATH]
dalil context [OPTIONS] [PATH]
dalil impact [OPTIONS] [PATH]
dalil search <QUERY> [OPTIONS] [PATH]
dalil search --symbol <NAME> [OPTIONS] [PATH]
dalil explain [OPTIONS] <PATH-OR-SYMBOL> [PATH]
dalil history [OPTIONS] [PATH]
dalil history <churn|contributors|bugs|activity|firefighting> [OPTIONS] [PATH]
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

CLI help organizes compatible commands by purpose without creating a second
analysis pipeline. `search` joins this surface with R3:

```text
Work with a codebase:
  dalil
  dalil orient [OPTIONS] [PATH]
  dalil map [OPTIONS] [PATH]
  dalil context [OPTIONS] [PATH]
  dalil impact [OPTIONS] [PATH]
  dalil search <QUERY> [OPTIONS] [PATH]
  dalil explain [OPTIONS] <PATH-OR-SYMBOL> [PATH]

Inspect evidence:
  dalil history [OPTIONS] [PATH]

Maintain Dalil:
  dalil cache <path|status|prune|clear>
  dalil capabilities [--json]
  dalil doctor [OPTIONS] [PATH]
```

The default command and `dalil orient` execute the same orientation operation.
`context` accepts task-ranking options and local change inputs: `--base` with
`--head`, a single `--revision-range base..head`, or `--dirty-worktree`.
`impact` accepts the same change forms. Both resolve change inputs through
isolated local Git access and return bounded changed-path and changed-symbol
evidence with typed uncertainty. Existing commands remain compatible while the
help hierarchy and documentation move users toward the primary workflows.

### Explaining Recommendations

Recommendations reuse target, purpose, reason, evidence, confidence, and
limitations where those fields fit. Each operation can add the information its
question needs without creating parallel meanings for the same ideas. `orient`
answers where to start, `context` answers what matters to a task, `impact`
answers what surrounds a change, and `search` anchors a path, symbol, or
concept. Plain search accepts one query; `search --symbol NAME` performs an exact
lookup against retained syntax evidence. `explain` expands the evidence behind one
selected recommendation.

### Orientation Briefing

The default command and `dalil orient` return the same typed
`OrientationReport`. The report contains repository identity, starting points,
important roots, runtime entry points, tests, useful history, next reads, and
uncertainty. It is not a complete `RepositoryReport` with the orientation
embedded inside it. `map` remains a first-class bounded view of repository
structure and the evidence behind it.

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

`dalil context` accepts structured equivalents of:

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
├── suggested next reads
└── optional source-grounded teaching sequence
```

When requested, the teaching sequence cites only evidence already returned in
the bundle. It labels direct observations separately from inferred or ambiguous
reading order, and omits unsupported steps. Every recommendation retains typed
evidence and confidence. Markdown and JSON render the same selected bundle, and
the configured budget applies to the whole result. Compact Markdown may be
projected further for terminal output.

### Change and Impact Context

Change-aware `context` starts from safely resolved local revision ranges,
dirty-worktree paths, and changed symbols when source locations make them
detectable. The planned `impact` operation will reuse this resolution evidence.
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

`search` returns a small set of path, symbol, and concept anchors when a task
cannot yet be tied to known targets. It may include a directly related file or
test when that evidence helps the next source read. It does not expose graph
traversal, path finding, centrality, or a general query language. Richer graph
queries remain internal unless repeated consumer needs justify a public
operation.

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

The persistent user-cache index stores file identities and hashes, query-pack
identity, bounded syntax summaries, lexical edges, HEAD identity, and bounded
history aggregates. It is versioned, atomically written, and private to the
user cache. A warm request reuses unchanged source records and lexical edges.
File fingerprints, manifests, repository revision, worktree inventory, options,
and language packs invalidate affected facts; bounded graph cases that cannot
prove an equivalent partial refresh run cold. Map and context provenance report
both per-file cache state and repository-index state. A missing, corrupt,
incompatible, or unavailable index falls back to fresh local analysis. The first
implementation does not require a daemon.

## Integration Contract

### Embeddable Core

The CLI is one adapter over typed analysis operations:

```rust
map(request) -> RepositoryReport
orient(request) -> OrientationReport
context(request) -> ContextBundle
impact(request) -> ImpactReport
explain(request) -> Explanation
search(request) -> SearchResults
```

Task-shaped results reuse recommendation fields where they fit while retaining
the specialized information needed for orientation, task context, change
review, search, and explanation. `map` provides the bounded repository-wide
structural view and is distinct from the shorter orientation briefing.

Rendering stays outside the analysis core where practical. The compiled CLI
remains the highest-level black-box compatibility boundary even after a library
API is available.

The operations live in the `dalil-core` workspace crate. The `dalil` package
in `crates/dalil-cli` owns request parsing and rendering and preserves
`cargo install dalil`. It depends on `dalil-core`; the core does not depend on
CLI, transport, or protocol code. See [the core API guide](docs/src/content/docs/guides/embeddable-core.md).

### Agent Interfaces

- The CLI remains the universal interface for humans, shell-capable agents,
  scripts, CI, unsupported hosts, and integration debugging.
- The `dalil-mcp` workspace crate exposes the primary workflows, including the
  bounded repository map, and preserves response budgets, provenance,
  uncertainty, and quality metadata. MCP protocol and runtime dependencies stay
  in that crate. See [the MCP integration guide](docs/src/content/docs/integrations/mcp.md).
- Native hosts prefer the core library when they can pass task text, inspected
  files, edited files, identifiers, budget, and worktree changes directly.

A small agent skill teaches consumers when to use each operation and when to
fall back to source search. Dalil narrows exploration; it does not replace
reading source.

Lifecycle adapters may offer a small orientation notice at session start, pass
typed task hints before exploration, invalidate or refresh analysis after edits,
and request impact context at the review boundary. Hooks remain advisory and
bounded, and they do not inject a large report after every write. Add a
workspace crate for a named host only when its adapter brings protocol or runtime
dependencies or needs a separate executable boundary. The agent skill remains a
packaged asset rather than a Rust crate.

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
- Keep workspace dependencies directed from CLI, MCP, and approved host adapter
  crates into `dalil-core`. Package publishable crates in dependency order while
  keeping `dalil` as the CLI installation package.
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
cargo package --workspace --exclude xtask --locked
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

Dalil is not an exhaustive code knowledge graph, graph query language,
checked-in repository database, generated documentation system, default
embeddings or vector-search system, LLM analysis pipeline, daemon,
framework-intelligence platform, hosted indexing service, autonomous coding
agent, project wiki, architectural memory engine, source host, general code
quality score, language-server replacement, compiler-grade analyzer, or
substitute for reading source.

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
