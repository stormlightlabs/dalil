+++
id = "arcl-t-01M6JEM9P4B52ZMB22PV8MZK56"
phase = "arcl-ph-01M6JEM9KCJ0759TJR9M2ET1RA"
title = "Add a usages command"
status = "pending"
priority = "high"
position = 0
+++

Neither agent in the A/B found `relationships references`. People and agents both ask "where is this used".

Acceptance criteria:

- [ ] `dalil usages <name>` lists every use with file:line, snippet, enclosing definition and confidence, from the existing relationship graph
- [ ] Results are bounded with totals and omissions, and readable in Markdown
- [ ] The Dalil skill and docs name it
