+++
id = "arcl-t-01M65H08YZTWZ9CM17WX16N8QH"
phase = "arcl-ph-01M65H08RW144BCPZ31QP7N2VS"
title = "Follow renames with a path forwarding table"
status = "pending"
priority = "normal"
position = 2
blocked-by = ["arcl-t-01M65H08YGR6ZW8CWD86B4SDCT"]
+++

History follows exact paths today, so a renamed file loses its earlier
commits.

Acceptance criteria:

- [ ] Ingest detects renames with `gix` rewrite tracking
- [ ] Each rename is stored with its `CommitId`, and forwarding follows only
      the renames reachable from the commit asked about
- [ ] Churn and `path_commits` for a renamed file include commits from before
      the rename, covered by a fixture
- [ ] Asked as of a commit before the rename, the fixture's file keeps its old
      name
