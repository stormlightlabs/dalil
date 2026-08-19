---
title: Context bundles
description: Request one set of repository evidence for a task.
section: Guides
group: Guides
order: 5
---

`dalil context` combines the repository overview, task-aware file selection,
lexical relationships, likely tests, history, risks, uncertainty, omissions,
and next reads in one result.

```sh
dalil context --task 'fix parser cache invalidation'
```

Use it when an orientation briefing is too broad and a single `explain` target
is too narrow.

## Add task evidence

The context command accepts the same task options as `dalil map`:

```sh
dalil context \
  --task 'review parser cache changes' \
  --symbol CacheStore \
  --changed-path src/map/cache.rs \
  --project packages/compiler \
  --teach \
  --budget 750 \
  --json
```

`--task` supplies a short description. `--symbol`, `--task-path`, `--project`,
`--changed-path`, and `--changed-symbol` anchor the request when you know the
relevant code. `--language` and `--search` narrow ranking further.

`--base` and `--head` compare local revisions. An omitted endpoint defaults to
`HEAD`. `--revision-range` accepts one `base..head` range, while
`--dirty-worktree` compares the index with local files and includes untracked
paths. Dalil reports added, deleted, modified, renamed, and untracked paths.
For supported languages, it also records symbols whose locations overlap changed
lines.

Dalil resolves changes through its embedded Git library. It does not invoke Git,
hooks, filters, repository programs, or remotes. `change_resolution.uncertainty`
records unresolved revisions, unsafe paths, unavailable source, and bounded work.

## Teach an unfamiliar subsystem

Add `--teach` when you want a short reading sequence from the selected context
rather than another broad overview. The scaffold may cover a behavior start,
lexical control-flow lead, state or data declaration, relevant test, and next
read. Dalil omits any section that the selected bundle cannot support. Under a
tight budget, it retains a runtime recommendation before generic orientation
files so the scaffold has source evidence to cite.

Each step has `observed` evidence and an `ordering` label. Observed records
point to returned files, symbols, relationships, tests, or next reads.
`inferred` means Dalil chose a reading order from that evidence. `ambiguous`
means the returned evidence contains multiple plausible starts and does not
establish one.

## Read the result

JSON places the task-shaped result at `context`. It includes:

- `request`: normalized task, path, project, change, revision-context, budget,
  profile, and teaching inputs.
- `orientation`: repository scope, worktree state, primary languages, project
  roots, and instructions or manifests worth reading first.
- `change_resolution`: resolved local revisions, changed paths, changed symbols,
  and typed uncertainty.
- `files`: ranked recommendations with reasons, ranking evidence, selected
  symbols, and snippets where they fit.
- `relationships`, `relevant_tests`, and `history`: supporting evidence for the
  selected files.
- `risks`, `uncertainty`, `omissions`, and `next_reads`: limits and follow-up
  reads that qualify the recommendation.
- `teaching`: an optional scaffold requested with `--teach`.

The normal `map` and `history` objects are omitted from a context response.
Use those commands when you need their full bounded projections.

## Budget

`--budget` applies to the selected evidence across the whole bundle. Dalil adds
higher-priority task evidence first and records projection in
`context.budget.truncated`. It does not reserve a fixed share for each section.
A scaffold that cannot fit beside its source evidence is omitted rather than
adding ungrounded summary text.

Markdown and JSON describe the same bundle. Markdown can be trimmed to the
compact report budget; use JSON when a consumer needs every selected field.

## Review a change

`dalil impact` accepts the same revision and dirty-worktree inputs as
`context`, then prepares a review list around the change:

```sh
dalil impact --revision-range 'HEAD~1..HEAD'
dalil impact --dirty-worktree --task 'review parser changes' --json
```

The report seeds the shared relationship graph from resolved changes and any
explicit file or symbol inputs. It ranks direct and downstream file targets,
affected projects, symbols, and likely tests under the report budget. Each
high-priority graph target includes the retained relationship path that led to
it. `direct`, `transitive`, and `inferred` labels separate changed or one-edge
evidence, multi-edge downstream evidence, and ambiguous or structural evidence.

`impact.traversal` reports the depth and edge-inspection limits. If the walk
reaches a limit, `impact.uncertainty` says which downstream evidence may be
missing. The report still includes the earlier results. Relationships identify
code to inspect; they do not prove compiler-resolved callers, runtime control
flow, or breakage.
