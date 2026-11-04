+++
id = "arcl-t-01M6JEM9T6CB3TZFMJNMAG0ZDF"
phase = "arcl-ph-01M6JEM9KCJ0759TJR9M2ET1RA"
title = "Lead JSON with results and keep quality flags honest"
status = "pending"
priority = "high"
position = 0
+++

Every JSON report opens with about 4 KB of limits and provenance. Flags like `truncated` on zero matches and "per-file symbol limit reached" on the needed file mislead.

Acceptance criteria:

- [ ] JSON reports put results first; limits and provenance move under one key, or behind a flag, without losing them
- [ ] `truncated` and resource-limit flags appear only when they changed this result
- [ ] Schema version and docs updated
