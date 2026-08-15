---
title: Read the default briefing
description: Use Dalil's repository overview and reading plan to orient yourself before editing.
section: Guides
group: Guides
order: 3
---

`dalil [PATH]` begins with a repository overview and an ordered reading plan.
It then adds up to five concise history observations with the evidence that
supports them. JSON retains the complete map and history report.

## Select a profile

The default `compact` profile returns selected snippets and bounded samples of
files, symbols, edges, findings, omissions, and history evidence. Use the
`evidence` profile for a larger, still resource-limited sample:

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
