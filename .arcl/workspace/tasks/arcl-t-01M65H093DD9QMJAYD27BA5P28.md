+++
id = "arcl-t-01M65H093DD9QMJAYD27BA5P28"
phase = "arcl-ph-01M65H08T6XANNJTGAFMMKV0VE"
title = "Audit interfaces and safety before release"
status = "pending"
priority = "normal"
position = 2
blocked-by = ["arcl-t-01M65H0921KYA88TBM58V1D8PH"]
+++

Before the first release, check that CLI, core, MCP, the store and exports
agree and fail safely.

Acceptance criteria:

- [ ] Cover malformed source, hostile paths, corrupt store, partial history,
      stale exports, symlink races and output limits
- [ ] A test proves ordinary commands do not write to the repository
- [ ] A test proves `dalil export` writes only under `.dalil/`
- [ ] Each confirmed failure gets a regression test
