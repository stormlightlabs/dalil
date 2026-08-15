---
title: Task-aware Ranking
description: Rank Dalil's source map and reading plan against the work you need to do.
section: Guides
group: Guides
order: 4
---

Task inputs reorder the source map and reading plan around code relevant to
your work. Dalil also weighs dependency structure, history evidence when
available, and any `--focus` or `--focus-path` you provide.

## Start with a task

Describe the work in a short phrase. Dalil derives local lexical terms from the
text and matches them against source paths, symbols, and declaration context.

```sh
dalil --task 'fix parser cache invalidation'
dalil map --task 'find the parse source entry point' --json
```

The default command applies task ranking to its reading plan. `dalil map` shows
the ranked source map. `dalil explain` includes task evidence for one path or
symbol. `dalil context` returns a selected task bundle across files, tests,
relationships, history, and next reads.

## Add known targets

Add the details you already have. You can repeat most task options.

| Option                  | Use it for                                                              |
| ----------------------- | ----------------------------------------------------------------------- |
| `--symbol NAME`         | Files that define or refer to a symbol. Qualified names work too.       |
| `--task-path PATH`      | Files below a task-relevant repository path.                            |
| `--language LANGUAGE`   | Files in one or more source languages.                                  |
| `--project PATH`        | Files inside a recognized project root.                                 |
| `--changed-path PATH`   | Files below a changed path.                                             |
| `--changed-symbol NAME` | Files that define or refer to a changed symbol.                         |
| `--search TERM`         | A lexical term matched against paths, symbols, and declaration context. |

For example, a Rust cache change in a monorepo might use:

```sh
dalil \
  --task 'invalidate parser cache after a source change' \
  --project packages/compiler \
  --language rust \
  --changed-path packages/compiler/src/map/cache.rs \
  --changed-symbol CacheStore \
  --symbol parse_source
```

`--language` accepts `rust`, `javascript` or `js`, `jsx`, `typescript` or `ts`,
`tsx`, `python` or `py`, `ruby` or `rb`, `java`, `c_sharp`, `csharp`, `c#`,
`go`, `lua`, and `zig`.

## Combine task context with focus

Task inputs identify related code. Use `--focus` and `--focus-path` when a
file or term must take priority.

```sh
dalil --task 'repair parser cache' --symbol parse_source --focus cache --focus-path src/map
dalil map --task 'update the web client' --project apps/web --focus-path apps/web/src
```

Use `--budget N` to limit ranked map results and compact Markdown output. JSON
always retains the task inputs.

## Inspect the evidence

Use JSON when you need to see why a result ranked:

```sh
dalil map --task 'fix parser cache invalidation' --changed-path src/map/cache.rs --json
```

The `map.task_seeds` field records the normalized inputs. Each `map.ranking`
entry includes `matched_seeds` and score contributions for centrality, seed
proximity, lexical relevance, history evidence, and focus. The contributions
add up to the entry's `score`.

`map.selection` returns three to five strong source files when that evidence
fits the budget. It favors task matches, runnable entry points, tests, and
project-root coverage over repeated files from one subsystem. JSON also lists
likely primary languages, relevant paths omitted by the selection bound, and a
shortfall when fewer than three useful files fit.
