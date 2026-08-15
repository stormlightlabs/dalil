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

## Narrow the briefing

Focus a report on a symbol, a path, or both:

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
