+++
id = "arcl-t-01M65H0911CR3RJ4RZ0NGBTX1J"
phase = "arcl-ph-01M65H08SADJJZD0XMBD0ET77H"
title = "Report what a change missed"
status = "pending"
priority = "normal"
position = 3
blocked-by = ["arcl-t-01M65H08ZZWHP7SFFAA0X7P6W2"]
+++

Answer "what did my change miss" for the dirty worktree or a revision range:
files that usually change with the changed ones, related tests, and the
command that runs those tests.

Acceptance criteria:

- [ ] Output lists co-changed files not in the change, ranked, with the
      commits behind each
- [ ] Output lists related tests and a command to run them for Cargo, npm,
      pytest and Go projects
- [ ] A replay check: for commits split in two, the second half's files rank
      in the top 10
