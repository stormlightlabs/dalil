+++
id = "arcl-t-01M6JEM9METVR68FF6HQY453KH"
phase = "arcl-ph-01M6JEM9KCJ0759TJR9M2ET1RA"
title = "Split search queries into ranked terms"
status = "in_progress"
priority = "high"
position = 1
+++

`dalil search` matches a plain query as one substring, so a commit subject or a phrase like `zig language support parser` returns nothing (0% recall in the replay harness on Dalil and fd).

Acceptance criteria:

- [ ] A plain multi-word query is split into terms and results are ranked by how many terms match and where (symbol name, path, content)
- [ ] An exact phrase is still available with quotes
- [ ] The replay harness shows search recall@10 above 0 on Dalil and fd, recorded in the evaluation page
