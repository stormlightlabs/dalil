+++
id = "arcl-t-01M65H08WSSXHF235P87CJGX41"
phase = "arcl-ph-01M65H08QZ0AWDGX26SNY66VYZ"
title = "Gate recall and output size in CI"
status = "pending"
priority = "normal"
position = 2
blocked-by = ["arcl-t-01M65H08W8HBCF0NZCAQXN1KGQ"]
+++

Keep retrieval changes from lowering recall or raising output size unnoticed.

Acceptance criteria:

- [ ] CI runs the evaluation on small synthetic repositories with known
      answers
- [ ] A drop in recall@10 or a rise in bytes past a set threshold fails the
      job and names the fixture and the measure
- [ ] The larger pre-release tier on pinned public repositories is documented
      and runs from one command
