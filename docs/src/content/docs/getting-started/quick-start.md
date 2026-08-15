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
```

The default briefing combines a repository overview, an ordered reading plan,
and concise history observations. It is designed to help you decide where to
look before you make a change.

## Rank the briefing for a task

Give Dalil a concise task description when you know what you need to change:

```sh
dalil --task 'fix parser cache invalidation' --changed-path src/map/cache.rs
dalil map --task 'find the parse source entry point' --symbol parse_source --json
```

`--task` derives local search terms and ranks matching files with related code.
Add a symbol, path, language, project root, changed path or symbol, or
`--search` term when you know a precise target.

Use `--focus` and `--focus-path` to raise a file or term's priority:

```sh
dalil --focus parser --focus-path src --budget 500
dalil map src --exclude 'src/generated/**' --json
dalil explain Parser --focus Parser --json
```

`--budget` limits ranked map selection and compact Markdown output. Exact focus
paths can include a classified `bin/` entry within the normal safety limits.

## Choose an output format

Markdown is the default. Use JSON for tools or HTML for a standalone browser
report:

```sh
dalil --json
dalil --html > dalil-report.html
dalil --html --open
```

Reports go to stdout. Progress and diagnostics go to stderr, so output
redirection writes a clean report file.
