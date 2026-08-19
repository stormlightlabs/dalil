---
title: Quick start
description: Produce a repository briefing and narrow it to the code you need to read.
section: Get started
group: Getting started
order: 2
---

Start in the Git worktree you want to understand:

```sh
dalil
dalil orient
dalil map
dalil export
dalil context --task 'fix parser cache invalidation' --changed-path src/map/cache.rs
dalil impact --revision-range 'HEAD~1..HEAD'
dalil impact --task-path src/parser.rs --symbol parse --json
dalil traverse reverse-dependencies src/parser.rs --depth 3
dalil search parser
dalil explain src/map.rs
```

`dalil` and `dalil orient` return the same orientation report: repository
identity, first reads, project roots, runtime entry points, tests, useful
history, and any limitations that affect the result.

`dalil search --symbol CacheStore` performs an exact symbol lookup. Use
`dalil history` after these workflows when you need focused Git evidence. Use
`dalil export` when you need a persistent `.dalil/map.json` and `.dalil/map.md`
for later inspection or sharing. See [repository evidence bundles](/docs/guides/repository-evidence-bundles/).

## Rank a task briefing

Give Dalil a concise task description when you know what you need to change:

```sh
dalil --task 'fix parser cache invalidation' --changed-path src/map/cache.rs
dalil map --task 'find the parse source entry point' --symbol parse_source --json
dalil context --task 'fix parser cache invalidation' --changed-path src/map/cache.rs --teach --json
```

`--task` derives local search terms and ranks matching files with related code.
Add a symbol, path, language, project root, changed path or symbol, or
`--search` term when you know a precise target. `impact` accepts the same file
and symbol seeds and follows incoming graph relationships to find downstream
review targets.

Use `--focus` and `--focus-path` to raise a file or term's priority:

```sh
dalil --focus parser --focus-path src --budget 500
dalil map src --exclude 'src/generated/**' --json
dalil explain Parser --focus Parser --json
```

`--budget` limits ranked map selection and compact Markdown output. For
`dalil context`, it applies to the selected evidence across the whole context
bundle. Search applies its result limit and token budget to one anchor set.
Add `--teach` when you want a source-grounded reading sequence for an unfamiliar
subsystem. Exact focus paths can include a classified `bin/` entry within the
normal safety limits.

## Search before reading source

Use a plain query for a path or concept. Use `--symbol NAME` when you know the
exact identifier:

```sh
dalil search parser
dalil search --symbol CacheStore --json
dalil search cache --limit 3 --budget 600
```

Each result explains why it is an anchor and records its evidence, confidence,
and limitations. Search can add one direct lexical file or test anchor. It does
not expose caller, callee, centrality, traversal, or graph-query modes.

## Choose an output format

Markdown is the default for readability. Use JSON for tools or HTML for a standalone
browser report:

```sh
dalil --json
dalil orient --json
dalil --html > dalil-report.html
dalil --html --open
```

Reports go to stdout whereas progress and diagnostics go to stderr, so output
redirection writes a clean report file.
