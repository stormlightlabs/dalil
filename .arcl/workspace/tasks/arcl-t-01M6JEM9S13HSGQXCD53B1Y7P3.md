+++
id = "arcl-t-01M6JEM9S13HSGQXCD53B1Y7P3"
phase = "arcl-ph-01M6JEM9KCJ0759TJR9M2ET1RA"
title = "Rebuild context from search, history and co-change"
status = "pending"
priority = "high"
position = 0
+++

Context recall@10 is 6.1% on Dalil and 3.3% on fd, against a keyword baseline at 38.1% and 93.0%. It spends its budget on README, AGENTS.md and Makefile.

Acceptance criteria:

- [ ] Context combines similar-commit files, search hits and co-changed files with reciprocal rank fusion, and explains which source placed each file
- [ ] Landmarks appear only when no task-matched file fills the slot
- [ ] Replay recall@10 beats the keyword baseline on Dalil, recorded in the evaluation page
