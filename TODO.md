# Tickets: Codeplat V1

## Release blockers

- Generated, vendored, and minified source can still consume analysis limits and degrade recommendations.
- Zig does not yet have first-class structural-map support.
- Scale benchmarks do not yet enforce latency/output ceilings for ignored trees, high ambiguity, and deep history.
- The configured Linux, macOS, Windows, Rust 1.85, and dependency-policy jobs need a green release-candidate run.

## Completed foundation

Earlier completed tickets established the CLI/report contract, five history signals, seven language families,
cache modes, bounded lexical maps, the integrated briefing, the evidence-backed default reading plan,
hostile-repository containment, report provenance/schema fixtures, history correctness, explainable lexical
evidence, repository landmarks/topology, and the concise default history briefing.

The packaging work added metadata/licensing, minimal `gix` features, dependency policy, cross-platform/MSRV CI,
checksummed artifacts, generated completions/man pages, and release documentation.

## 18. Build the default repository reading plan

Make `codeplat [PATH]` lead with a practical, evidence-backed sequence of files to read
rather than a flat ranked winner and long diagnostic sections.

The same typed reading plan must be available in JSON.

## 19. Make the default history briefing concise and useful

Replaced exhaustive history tables in the default Markdown briefing with 3–5 distinct,
evidence-backed observations, while preserving detailed history commands and machine evidence.

## 20. Keep generated, vendored, and minified source out of the default plan

Classify low-value generated/vendor/minified source before parsing so it cannot consume default
analysis limits, recommendations, or actionable quality status.

## 21. Add first-class Go maps

Gave Go repositories the same bounded structural-map and reading-plan support as existing
first-class languages.

## 22. Add first-class Lua maps

Gave Lua repositories bounded structural maps that handle common module patterns without
pretending dynamic name resolution is semantic.

## 23. Add first-class Zig maps

**What to build:** Give Zig repositories bounded structural maps for declarations, imports, tests, and public API
orientation.

**Blocked by:** None - can start immediately

**Acceptance criteria:**

- [ ] A reviewed upstream Zig Tree-sitter grammar and versioned query pack are registered with minimal features.
- [ ] Definitions cover functions, variables/constants, container types, fields, tests, and public declarations
      with accurate locations, scopes, and declaration snippets.
- [ ] References/import evidence covers calls, identifiers, field access, type uses, and literal `@import`
      paths; compile-time and inferred semantics remain explicit limitations.
- [ ] `pub`, nested containers, anonymous containers, error unions, generics/comptime syntax, malformed input,
      duplicate names, and test blocks have positive and negative conformance fixtures.
- [ ] Zig participates in mixed-language ranking, capabilities, provenance, cache identity, Markdown, and JSON;
      the generic reading-plan contract can consume its ranked evidence without language-specific logic.
- [ ] README/help/roadmap language lists match implemented support.

**Verification:**

- Run default, JSON map, focused, and capabilities commands against Zig and mixed fixtures.
- Assert definitions, references, visibility, import evidence, limitations, and ranked evidence.
- Run the standard workspace checks plus `cargo package --locked`.

## 24. Make compact quality and strict policy actionable

Separate expected compact projection from conditions that make a briefing materially
unsafe or misleading, so normal bounded output does not look like a failed analysis.

## 25. Enforce V1 scale and usefulness gates

**What to build:** Turn the current resource ceilings and subjective release review into repeatable evidence that
the default briefing stays fast, bounded, and useful on realistic repositories.

**Blocked by:** Tickets 19, 20, 21, 22, 23, and 24

**Acceptance criteria:**

- [ ] CI-friendly benchmarks cover Codeplat, a large ignored/vendor tree, high-ambiguity sources, and synthetic
      10k/100k-commit histories with documented latency and output ceilings.
- [ ] Benchmark failures identify the exceeded work/output dimension and do not depend on network access or
      private repositories.
- [ ] A reusable release-review rubric checks reading-plan count/coverage/reasons, concise history, actionable
      quality, stdout/stderr, and manual usefulness without reducing the result to one opaque score.
- [ ] The release binary is rerun across the available first-party project corpus; only aggregate outcomes and
      reproducible public/synthetic regressions are retained.
- [ ] Small-project, Codeplat, and mixed-monorepo Markdown briefings pass recorded human review with no known P0
      usability or correctness finding waived without rationale and expiry.

**Verification:**

- Run the benchmark harness under documented CI time and output ceilings.
- Run every fixture in Markdown/JSON, compact/evidence, strict/non-strict, and relevant cache modes.
- Run the release-binary project sweep and inspect aggregate failures, recommendation coverage, partial/unsupported
  causes, maximum output, and unexpected stderr.
- Run all standard workspace and package checks.

## 26. Ship V1

**What to build:** Produce the supportable V1 release only after every product, safety, performance, packaging,
and platform gate is green.

**Blocked by:** Ticket 25

**Acceptance criteria:**

- [ ] The default reading plan, concise history, generated/vendor handling, quality policy, and all ten language
      families match README/help/schema/capabilities documentation.
- [ ] Formatting, all-feature workspace tests, Clippy with warnings denied, docs, schema compatibility, package
      verification, dependency policy, minimal `gix` features, generated assets, and benchmark gates pass.
- [ ] Linux, macOS, Windows, and Rust 1.85 CI jobs pass on the release candidate.
- [ ] Checksummed release archives are reproducible from the committed lockfile and install/uninstall/cache cleanup
      instructions are verified.
- [ ] No release blocker remains in this file. Any waived P0 finding has an owner, rationale, and expiry recorded
      before release.

**Verification:**

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo test --workspace --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`
- `cargo package --locked`
- `cargo release-assets` followed by generated completion/man-page existence checks
- Inspect CI, benchmark results, package contents, feature tree, dependency policy, and artifact checksums.

## V2

- Revision comparison between repository states.
- F#, Elixir, C, and C++ language support.
- Semantic-provider and framework-specific recommendations.

## Frontier

- Ticket 23: Add first-class Zig maps.

## Parking Lot

- [x] Read bounded manifest metadata to identify declared runtime entry points, library exports,
      and common build, test, and run commands instead of relying only on conventional filenames.
- [ ] Detect runnable examples and classify integration or end-to-end tests separately so the reading
      plan can prefer them as executable specifications when stronger evidence is available.
- [ ] Add an entry-point walkthrough that follows qualified lexical call and data-flow evidence one
      level at a time, with ambiguity and the lack of semantic resolution stated beside each hop.
- [ ] Identify an evidence-backed gateway artifact: a short file or example that connects the repository's
      stated purpose to its runtime path and makes the rest of the reading plan easier to understand.
- [ ] Let `explain` recommend the next file to inspect from the current path or symbol, using
      bounded graph, project-topology, test, and history evidence to support opportunistic exploration.
- [ ] Offer a concise teaching-scaffold report with the repository purpose, languages, workflow or
      module relationships, key files, and exactly two source-based synthesis exercises; include the
      files consulted and avoid unsupported architectural claims.
- [ ] Attach bounded recent-commit context to recommended files so readers can investigate why
      important code changed without treating churn or commit language as a quality judgment.
