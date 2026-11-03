+++
id = "arcl-t-01M6F927A36GQE5SX8K57VYN6H"
phase = "arcl-ph-01M65H08QHQG2WE9TPFPAR1JTJ"
title = "Resolve changed lines per hunk"
status = "in_progress"
priority = "normal"
position = 6
+++

`changed_lines` in `map/changes.rs` trims the common prefix and suffix, so every edit in a file becomes one range from the first change to the last. On Dalil's #3, four one-line edits in `map.rs` became lines 44 to 57 and `cache.rs` became 13 to 345, so impact lists every definition in between as changed.

Acceptance criteria:

- [ ] Each hunk gets its own range, from a line diff (a dependency such as `imara-diff` through gix's `blob-diff` feature, or a small Myers diff)
- [ ] A fixture with two separated edits lists only the definitions at those edits
- [ ] On Dalil, `dalil impact --revision-range d90a55c~1..d90a55c` lists only the constants #3 changed in `map.rs`
