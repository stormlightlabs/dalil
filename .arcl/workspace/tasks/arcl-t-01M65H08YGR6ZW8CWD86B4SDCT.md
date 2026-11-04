+++
id = "arcl-t-01M65H08YGR6ZW8CWD86B4SDCT"
phase = "arcl-ph-01M65H08RW144BCPZ31QP7N2VS"
title = "Ingest history into an oid-keyed commit table"
status = "pending"
priority = "normal"
position = 1
blocked-by = ["arcl-t-01M65H08XYV97ZE71YFM3YTB9H"]
+++

Store history in the commit table ROADMAP.md's **Commit table** describes, so
warm runs read only new commits.

Acceptance criteria:

- [ ] `commits`, `commit_parents`, `commit_files` and `path_commits` live in
      the store, with rows keyed by oid
- [ ] A warm run walks every parent from `HEAD` and skips commits already
      stored; a fixture with a merged side branch ingests the side branch's
      commits
- [ ] Switching branches, rebasing or force pushing adds only unseen commits
      and drops no rows; queries ignore commits `HEAD` does not reach
- [ ] `path_commits` appends to a buffer, and compaction merges it into the
      sealed CSR and drops rows no local branch, tag or `HEAD` reaches
- [ ] `dalil history` output is unchanged on the existing fixtures
