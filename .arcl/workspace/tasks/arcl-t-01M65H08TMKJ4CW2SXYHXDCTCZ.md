+++
id = "arcl-t-01M65H08TMKJ4CW2SXYHXDCTCZ"
phase = "arcl-ph-01M65H08QHQG2WE9TPFPAR1JTJ"
title = "Stop the symbol limit from hiding callers"
status = "completed"
priority = "normal"
position = 1
evidence = "Done in #3 at a475b27: graph commands read the unbounded map, definitions and calls survive the symbol limits, the edge limit counts distinct edges, and Rust path qualifiers resolve cross-file calls. callers language_capabilities returns both call sites on Dalil."
+++

`dalil relationships callers language_capabilities` returns 0 results on
Dalil's own repository, while `grep` finds 2 call sites in
`report/analysis.rs` and `report/diagnostics.rs`. The likely cause is the
128-symbols-per-file limit, which drops symbols from large files, together
with cross-file edges that come only from explicit imports.

Acceptance criteria:

- [ ] Find the cause and record it in the pull request
- [ ] A fixture with a file over the symbol limit and a same-crate caller
      without an import returns the caller
- [ ] `dalil relationships callers language_capabilities` on Dalil returns the
      2 call sites
