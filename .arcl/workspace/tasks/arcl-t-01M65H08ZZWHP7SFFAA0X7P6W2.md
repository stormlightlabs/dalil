+++
id = "arcl-t-01M65H08ZZWHP7SFFAA0X7P6W2"
phase = "arcl-ph-01M65H08SADJJZD0XMBD0ET77H"
title = "Rank co-changed files for seed paths"
status = "pending"
priority = "normal"
position = 1
blocked-by = ["arcl-t-01M65H08YZTWZ9CM17WX16N8QH"]
+++

Rank the files that change together with a set of seed files, as
ROADMAP.md's **Co-change** describes.

Acceptance criteria:

- [ ] Each commit weighs `1 / files_in_commit`; commits over the file limit
      (default 50) are skipped
- [ ] Work is limited by the seeds' commits and reported
- [ ] A fixture with known pairs ranks the partner file first
