---
title: Git history
description: Inspect bounded repository history without turning activity into a quality score.
section: Reference
group: Reference
order: 5
---

`dalil history` reads committed Git data and reports five signals:

## Choose a signal

```text
history                 all signals
history churn           changed-path frequency
history contributors    author concentration
history bugs            fix-related path clusters and churn overlap
history activity        author-date commits grouped by month
history firefighting    revert, hotfix, emergency, and rollback language
```

The default history window is 365 days. Recent contributor concentration uses
180 days. Override the window or keywords when the repository needs a different
scope:

```sh
dalil history bugs --window-days 30 --bug-keyword parser --json
dalil history bugs --keyword-match substring --json
dalil history contributors --include-emails --json
```

## Read the evidence

Bug and firefighting keywords use case-insensitive, word-aware matching by
default. Each evidence commit records the terms it matched. Pass
`--keyword-match substring` when substring matching is intentional.

Contributor analysis applies the `.mailmap` stored at the analyzed HEAD and
records raw-to-canonical mappings. Missing names are grouped as `Unknown`, and
email matching is case-insensitive. Compact output omits email addresses unless
`--include-emails` is supplied.

## Limits and provenance

Churn includes absolute commit counts and a rate per KiB using each path's
current HEAD blob size. Empty, binary, generated, deleted, oversized, and
resource-limited paths are labeled explicitly. Dalil does not currently follow
renames, so an exact-path count does not include history recorded under an
earlier name.

History provenance records the observed committer-date range, whether a field
uses author or committer time, current-HEAD semantics, and whether history is
complete, shallow, missing objects, or partial.

History output is evidence for investigation. Commit counts, churn, author
concentration, and keyword matches are not code-quality scores.
