---
title: Report Formats
---

Codeplat renders the same typed report model as Markdown, JSON, or HTML. The
selected analysis profile controls how much evidence the report contains; the
format controls how that evidence is presented.

## Markdown

Markdown is the default. It works well in a terminal, a pull request, or a text
file.

```sh
codeplat
codeplat map src
codeplat history contributors
```

Use `--format markdown` when a script or wrapper needs to select the default
explicitly:

```sh
codeplat map --format markdown > map.md
```

## JSON

JSON is intended for scripts, coding agents, and other tools that need typed
fields rather than formatted prose.

```sh
codeplat --json
codeplat map --format json > map.json
codeplat explain Parser --json
```

`--json` is shorthand for `--format json`.

JSON reports use `schema_version: 1`. The schema is available at
[`schema/v1/codeplat.json`](../schema/v1/codeplat.json), with compatibility
examples in [`schema/v1/golden`](../schema/v1/golden).

## HTML

HTML produces a standalone document for reading, sharing, or archiving in a
browser.

```sh
codeplat --html > codeplat-report.html
codeplat history --format html > history.html
codeplat doctor . --html > doctor.html
codeplat --html --open
```

`--html` is shorthand for `--format html`.

`--open` writes the report to a private temporary file, opens it with the
platform's default browser, and prints the temporary path to stderr. It does not
write a second copy to stdout. On macOS, Codeplat uses `open`; on Linux and
other Unix systems, it uses `xdg-open`. On Windows, it asks the system to open
the file with its registered handler.

With Markdown or JSON, `--open` leaves stdout unchanged and prints a warning to
stderr.

The document contains its CSS, JavaScript, and complete JSON report data. The
report remains usable without JavaScript; the script adds the **Copy JSON**
button. Manrope and IBM Plex Sans load from Google Fonts when a network
connection is available, with sans-serif fallbacks for offline use.

The report starts with the system light or dark theme. The theme control in the
masthead saves a different choice when browser storage is available.

## Profiles and formats

The `compact` profile is the default. It selects a bounded set of evidence for
quick orientation. The `evidence` profile returns larger bounded collections.
Either profile can use any report format.

```sh
codeplat --profile compact --html > briefing.html
codeplat map --profile evidence --json > map-evidence.json
codeplat history --profile evidence --format markdown
```

Collection totals and truncation reasons remain part of the report when a
profile returns a sample.

## Output and exit status

Reports go to stdout. Progress and diagnostics go to stderr, so redirecting
stdout writes a clean report file:

```sh
codeplat --html > report.html
```

`--color`, `--no-color`, and the `NO_COLOR` environment variable affect
diagnostics only. Report output never contains ANSI color sequences.

With `--strict`, Codeplat writes the report before returning exit status `5`
when relevant evidence is stale, incomplete, resource-limited, unsafe,
unsupported, or partial. This lets automation retain the report that explains
the failure.

Choose one format per command. Combining conflicting options such as `--json`
and `--html` returns a command-line usage error.
