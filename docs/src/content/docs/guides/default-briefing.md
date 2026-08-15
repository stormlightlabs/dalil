---
title: Orient a repository
description: Use Dalil's repository overview and reading plan to orient yourself before editing.
section: Guides
group: Guides
order: 3
---

`dalil [PATH]` begins with a repository overview and an ordered reading plan.
It then adds up to five concise history observations with the evidence that
supports them. JSON retains the complete map and history report.

![Dalil terminal briefing with a repository overview and ordered reading plan](/dalil-briefing.png)

## Rank the reading plan for a task

Pass task details to rank related code ahead of the broader map:

```sh
dalil --task 'fix parser cache invalidation' --changed-path src/map/cache.rs
dalil --task 'fix parser cache invalidation' --symbol CacheStore --language rust
```

The same task options work with `dalil map` and `dalil explain`. JSON includes
normalized task inputs and per-file ranking evidence, so you can see why a file
appeared in the reading plan.

## Select a profile

The default `compact` profile returns a three-to-five-file structural selection
when enough strong evidence fits the budget, plus bounded samples of files,
symbols, edges, findings, omissions, and history evidence. It reports a
shortfall when fewer than three useful source files fit. Use the `evidence`
profile for a larger, still resource-limited sample:

```sh
dalil --profile compact --html > briefing.html
dalil map --profile evidence --json > map-evidence.json
```

Each collection reports its observed total, returned count, truncation state,
and reason in JSON.

## Read limitations with the evidence

Dalil labels parse errors, unsupported or partial language evidence, ambiguous
lexical references, generated files, and resource limits beside the affected
output. Churn and commit-message matches are signals for investigation, not
quality scores.

Use `dalil explain PATH-OR-SYMBOL` when you need the typed focus, graph,
ranking, history-overlap, landmark, ambiguity, and omission evidence behind a
recommendation.

![Dalil explaining why a source file is relevant and what to read next](/dalil-explain.png)
