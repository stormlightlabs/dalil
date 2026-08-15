---
title: Reports
description: Render Dalil reports as Markdown, JSON, or standalone HTML.
section: Reference
group: Reference
order: 6
---

Dalil renders the same typed report model as Markdown, JSON, or HTML. The
selected analysis profile controls how much evidence the report contains; the
format controls how that evidence is presented.

## Markdown

Markdown is the default. It works well in a terminal, a pull request, or a text
file.

```sh
dalil
dalil map src
dalil history contributors
```

Use `--format markdown` when a script or wrapper needs to select the default
explicitly:

```sh
dalil map --format markdown > map.md
```

## JSON

JSON is intended for scripts, coding agents, and other tools that need typed
fields rather than formatted prose.

```sh
dalil --json
dalil map --format json > map.json
dalil explain Parser --json
```

`--json` is shorthand for `--format json`.

With task context, JSON records normalized inputs in `map.task_seeds`. Each
`map.ranking` entry includes `matched_seeds` and score contributions for
centrality, seed proximity, lexical relevance, history evidence, and focus.
Their sum is the entry's `score`. Supply `--task`, `--symbol`, `--task-path`,
`--language`, `--project`, `--changed-path`, `--changed-symbol`, or `--search`
to provide task context. `map.selection` records likely primary languages,
task-relevant paths excluded by the selection bound, and a shortfall when fewer
than three strong source files fit the token budget.

JSON reports use `schema_version: 1`. The schema is available at
[`schema/v1/dalil.json`](https://github.com/stormlightlabs/dalil/blob/main/schema/v1/dalil.json),
with compatibility examples in
[`schema/v1/golden`](https://github.com/stormlightlabs/dalil/tree/main/schema/v1/golden).

## HTML

HTML produces a standalone document for reading, sharing, or archiving in a
browser.

```sh
dalil --html > dalil-report.html
dalil history --format html > history.html
dalil doctor . --html > doctor.html
dalil --html --open
```

`--html` is shorthand for `--format html`.

`--open` writes the report to a private temporary file, opens it with the
platform's default browser, and prints the temporary path to stderr. It does not
write a second copy to stdout. On macOS, Dalil uses `open`; on Linux and
other Unix systems, it uses `xdg-open`. On Windows, it asks the system to open
the file with its registered handler.

With Markdown or JSON, `--open` leaves stdout unchanged and prints a warning to
stderr.

The document contains its CSS, JavaScript, and complete JSON report data. The
report remains usable without JavaScript; the script adds the **Copy JSON**
button. Google Sans, Google Sans Code, and IBM Plex Sans load from Google Fonts
when a network connection is available, with system font fallbacks for offline
use.

The report starts with the system light or dark theme. The theme control in the
masthead saves a different choice when browser storage is available.

## Profiles and formats

The `compact` profile is the default. It selects a bounded set of evidence for
quick orientation. The `evidence` profile returns larger bounded collections.
Either profile can use any report format.

```sh
dalil --profile compact --html > briefing.html
dalil map --profile evidence --json > map-evidence.json
dalil history --profile evidence --format markdown
```

Collection totals and truncation reasons remain part of the report when a
profile returns a sample.

For compact Markdown, `--budget` limits the whole rendered report as well as
the ranked structural-map selection. Dalil writes the summary and the
command's priority content first: the reading plan for a briefing and the
target-specific evidence for `explain`. If the remaining sections do not fit,
the report ends with a truncation notice. Use JSON for complete typed
collections or `--profile evidence` for verbose Markdown.

## Output and exit status

Reports go to stdout. Progress and diagnostics go to stderr, so redirecting
stdout writes a clean report file:

```sh
dalil --html > report.html
```

`--color`, `--no-color`, and the `NO_COLOR` environment variable affect
diagnostics only. Report output never contains ANSI color sequences.

With `--strict`, Dalil writes the report before returning exit status `5`
when relevant evidence is stale, incomplete, resource-limited, unsafe,
unsupported, or partial. This lets automation retain the report that explains
the failure.

Choose one format per command. Combining conflicting options such as `--json`
and `--html` returns a command-line usage error.
