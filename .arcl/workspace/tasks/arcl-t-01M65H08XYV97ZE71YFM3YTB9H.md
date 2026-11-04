+++
id = "arcl-t-01M65H08XYV97ZE71YFM3YTB9H"
phase = "arcl-ph-01M65H08RDC7CBQ0Z8ATRYA2KY"
title = "Encode the store with postcard"
status = "pending"
priority = "normal"
position = 2
blocked-by = ["arcl-t-01M65H08XBZ7BCV490F7BPRWF6"]
+++

The cache is JSON: 23 MB for Dalil's 222 files, with a warm `map` taking 0.8
to 0.9 s.

Acceptance criteria:

- [ ] The store is encoded with `postcard` behind a header carrying its own
      format version and a checksum of the encoded body
- [ ] A store from another version, or one whose checksum fails, is rebuilt,
      not read; a test flips a byte in the body and sees a rebuild
- [ ] Store size and warm `map`, `orient` and `search` times on Dalil are
      recorded before and after in the pull request
