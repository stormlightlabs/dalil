+++
id = "arcl-t-01M6JEM9VBF1GVNGV6X2S1PXM3"
phase = "arcl-ph-01M6JEM9KCJ0759TJR9M2ET1RA"
title = "Drop deleted files and separate docs churn in history"
status = "pending"
priority = "high"
position = 0
+++

The briefing's churn list leads with TODO.md, README.md and ROADMAP.md. TODO.md is deleted, and docs churn reads as noise for people (QA note).

Acceptance criteria:

- [ ] Churn and related history signals leave out paths that no longer exist at HEAD
- [ ] Documentation churn is reported on its own line, apart from code churn
- [ ] The briefing shows code churn first
