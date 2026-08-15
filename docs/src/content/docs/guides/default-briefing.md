---
title: Orient a repository
description: Use Dalil's repository overview and reading plan to orient yourself before editing.
section: Guides
group: Guides
order: 3
---

`dalil [PATH]` and `dalil orient [PATH]` return the same orientation report.
It identifies first reads, important project roots, runtime entry points, tests,
useful history, and limitations. JSON returns the typed orientation report;
use `dalil map` or `dalil history` when you need the underlying analysis.

```md
# Dalil orient

Schema version: 1
Scope: `.`
Status: Analyzed

## Summary

Selected 5 orientation read(s) across 1 important project root(s).

## Repository overview

Repository: `/Users/owais/Projects/StormlightLabs/OpenSource/mariners-astrolabe`
Scope: `.`
Worktree: clean
Revision: `refs/heads/main` at `05884ff325e1678aa4c1481c75bd7e82afdcf7ac`
Primary supported languages: Rust, TypeScript

### Start here

1. `README.md` — recognized readme: recognized documentation filename `readme.md` (high; landmark, project_topology)
2. `src/lib.rs` — conventional library entry point for this project root; manifest `Cargo.toml` declares library export `dalil` as `src/lib.rs` (high; landmark, project_topology, source_map, graph)

### Important project roots

- `.` (workspace) — project root inferred from 1 manifest(s): Cargo.toml

### Runtime entry points

3. `src/main.rs` — conventional runtime entry point for this project root; manifest `Cargo.toml` declares runtime entry point `dalil` as `src/main.rs` (high; landmark, project_topology, source_map)

### Tests

4. `tests/cli.rs` — the path is inside the recognized test root tests (high; landmark, project_topology, source_map)

_Report truncated at the compact Markdown token budget; use `--json` for complete typed collections or `--profile evidence` for verbose Markdown._
```

## Rank the reading plan for a task

Pass task details to rank related code ahead of the broader map:

```sh
dalil --task 'fix parser cache invalidation' --changed-path src/map/cache.rs
dalil --task 'fix parser cache invalidation' --symbol CacheStore --language rust
```

The same task options work with `dalil orient`, `dalil map`, and `dalil explain`.
The orientation report records a short reason, evidence kind, confidence, and
limitations for each selected read. Use `dalil explain` for ranking and
relationship evidence behind one path or symbol.

## Select a profile

The default `compact` profile returns three to five first reads when enough
strong evidence fits the budget, plus concise history observations. It reports
a shortfall when fewer than three useful paths are available. Use `dalil map`
with the `evidence` profile for a larger, resource-limited structural sample:

```sh
dalil orient --profile compact --html > orientation.html
dalil map --profile evidence --json > map-evidence.json
```

`dalil map --json` reports source and relationship collection totals,
truncation state, and reasons.

## Read limitations with the evidence

Dalil labels parse errors, unsupported or partial language evidence, ambiguous
lexical references, generated files, and resource limits beside the affected
output. Churn and commit-message matches are signals for investigation, not
quality scores.

Use `dalil explain PATH-OR-SYMBOL` when you need the typed focus, graph,
ranking, history-overlap, landmark, ambiguity, and omission evidence behind a
recommendation.

For example, `dalil explain crates/dalil-core/src/map.rs --focus map --no-cache --budget 350` returns:

```md
# Dalil explain

Schema version: 1
Scope: `.`
Status: Analyzed

## Summary

Explained `crates/dalil-core/src/map.rs` using 63 source files and 131 retained lexical relationships within scoped history evidence.

## Quality

Expected bounded projection only; collection totals and reasons remain available in JSON.

### Recommendation explanation

Target: `crates/dalil-core/src/map.rs` (path)
Matched paths: `crates/dalil-core/src/map.rs`
Focus evidence: `crates/dalil-core/src/map.rs`
Provenance:
- profile `compact`; 63 analyzed source file(s); 131 retained lexical relationship(s); history scope `.`
- no task seeds were supplied or derived
Reading guidance:
- `crates/dalil-core/src/map.rs` — ranking matched focus:map (high confidence)
  - ranking: score 353648; focus matches 1; 0 incoming and 0 outgoing relationship(s); contributions centrality=3648, seed proximity=0, lexical relevance=0, history evidence=0, explicit focus=350000
  - matched seeds: `focus` `map`
  - ranking evidence: 0 of 63 returned (ProfileProjection) — Ranking contributions may omit lower-ranked source paths under the active profile.

_Report truncated at the compact Markdown token budget; use `--json` for complete typed collections or `--profile evidence` for verbose Markdown._
```
