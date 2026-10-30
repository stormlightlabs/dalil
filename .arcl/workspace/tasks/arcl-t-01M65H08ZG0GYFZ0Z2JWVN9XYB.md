+++
id = "arcl-t-01M65H08ZG0GYFZ0Z2JWVN9XYB"
phase = "arcl-ph-01M65H08RW144BCPZ31QP7N2VS"
title = "Index commit messages with BM25"
status = "pending"
priority = "normal"
position = 3
blocked-by = ["arcl-t-01M65H08YGR6ZW8CWD86B4SDCT"]
+++

Index commit subjects and bodies so task text can find similar past changes,
as ROADMAP.md's **Commit-message index** describes.

Acceptance criteria:

- [ ] An inverted index maps terms to `CommitId`s and ranks with BM25
- [ ] The index updates with incremental ingest
- [ ] On Dalil, the query "add a new language (Kotlin) support" ranks
      `e11dc77 feat: Zig structural map support` in the top 3
