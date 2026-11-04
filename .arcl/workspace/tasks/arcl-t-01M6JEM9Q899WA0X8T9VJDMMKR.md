+++
id = "arcl-t-01M6JEM9Q899WA0X8T9VJDMMKR"
phase = "arcl-ph-01M6JEM9KCJ0759TJR9M2ET1RA"
title = "Find past commits similar to a task"
status = "pending"
priority = "high"
position = 0
+++

The best A/B agent answered the Kotlin task by finding the Zig commit and reading its file list. Dalil reads history but cannot answer "how was this done before".

Acceptance criteria:

- [ ] `dalil history similar "<task>"` returns commits whose messages match the task's terms, ranked, with each commit's changed files (renames followed where cheap)
- [ ] It runs on the existing commit walk with stated limits; the commit table later makes it fast
- [ ] On Dalil, "add a new language (Kotlin) support" ranks the Zig commit in the top 3
