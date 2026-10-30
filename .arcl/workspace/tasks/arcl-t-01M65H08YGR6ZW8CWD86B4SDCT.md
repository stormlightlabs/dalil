+++
id = "arcl-t-01M65H08YGR6ZW8CWD86B4SDCT"
phase = "arcl-ph-01M65H08RW144BCPZ31QP7N2VS"
title = "Ingest history into an append-only commit table"
status = "pending"
priority = "normal"
position = 1
blocked-by = ["arcl-t-01M65H08XYV97ZE71YFM3YTB9H"]
+++

Store history in the append-only commit table ROADMAP.md's **Commit table**
describes, so warm runs read only new commits.

Acceptance criteria:

- [ ] `commits`, `commit_files` and `path_commits` live in the store
- [ ] A warm run walks from `HEAD` only to the first known commit
- [ ] A rewritten branch (force push, rebase) rebuilds the affected suffix
- [ ] `dalil history` output is unchanged on the existing fixtures
