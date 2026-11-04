+++
id = "arcl-t-01M6JEM9METVR68FF6HQY453KH"
phase = "arcl-ph-01M6JEM9KCJ0759TJR9M2ET1RA"
title = "Split search queries into ranked terms"
status = "completed"
priority = "high"
position = 1
evidence = "Done in #10 at b0b0f79: plain search queries split into ranked terms; search recall@10 0% to 12.2% on Dalil and 23.5% on fd; a trigram index bug that missed matches is fixed."
+++

`dalil search` matches a plain query as one substring, so a commit subject or a phrase like `zig language support parser` returns nothing (0% recall in the replay harness on Dalil and fd).

Acceptance criteria:

- [ ] A plain multi-word query is split into terms and results are ranked by how many terms match and where (symbol name, path, content)
- [ ] An exact phrase is still available with quotes
- [ ] The replay harness shows search recall@10 above 0 on Dalil and fd, recorded in the evaluation page
