+++
id = "arcl-t-01M6JEM9N99E7BXADS5BNKD1Q8"
phase = "arcl-ph-01M6JEM9KCJ0759TJR9M2ET1RA"
title = "Show line, snippet and enclosing definition on search hits"
status = "pending"
priority = "high"
position = 0
+++

Search hits name a file and a reason such as "matched identifier `Some`", with no line numbers, so agents fall back to grep to see the match.

Acceptance criteria:

- [ ] Each hit carries file:line, a one-line snippet and its enclosing definition, in JSON and Markdown
- [ ] Match reasons name the matched text and where it matched
- [ ] A symbol query returns its definition first, then its uses
