+++
id = "arcl-c-01M6GCDDXMN94JF6DS9EHEY2QN"
title = "Impact drops result uncertainty under a tight budget without marking truncation"
status = "captured"
created-at = "2026-11-03T09:39:00.148Z"
+++

Impact fills its budget with uncertainty last (crates/dalil-core/src/report/impact.rs, the add_if_fits loop), so a tight budget drops result caveats such as incomplete_history first. `budget.truncated` does not count dropped uncertainty items. Reproduce: `dalil impact --changed-path crates/dalil-cli/src/report.rs --json` on Dalil keeps only `evidence_scope`. Found while revising #6.
