+++
id = "arcl-t-01M65H08XBZ7BCV490F7BPRWF6"
phase = "arcl-ph-01M65H08RDC7CBQ0Z8ATRYA2KY"
title = "Intern paths, symbols, commits and authors"
status = "pending"
priority = "normal"
position = 1
+++

Replace string file IDs with interned `u32` IDs, as ROADMAP.md's **Repository
store** describes. `FileId` is a `String` today, so every trigram posting
repeats a full path.

Acceptance criteria:

- [ ] `PathId`, `SymbolId`, `CommitId` and `AuthorId` index one string table
- [ ] The trigram index, symbol index and relationship graph hold only IDs
- [ ] Reports and exported schemas are unchanged; the existing tests pass
      unmodified
