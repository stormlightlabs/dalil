+++
id = "arcl-t-01M65H08V53HC5P4HK6MJ0Y31C"
phase = "arcl-ph-01M65H08QHQG2WE9TPFPAR1JTJ"
title = "List only definitions as impact's changed symbols"
status = "pending"
priority = "normal"
position = 2
+++

`dalil impact --revision-range HEAD~1..HEAD` lists identifiers such as
`writeln`, `output` and `expect`, repeated, as changed symbols. Changed
symbols should be definitions: functions, types, modules and similar.

Acceptance criteria:

- [ ] Changed symbols list only definitions, each once
- [ ] A CLI fixture covers a change that edits a function body full of calls
