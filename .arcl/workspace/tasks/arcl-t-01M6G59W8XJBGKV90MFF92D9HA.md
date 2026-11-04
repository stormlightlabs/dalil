+++
id = "arcl-t-01M6G59W8XJBGKV90MFF92D9HA"
phase = "arcl-ph-01M65H08QHQG2WE9TPFPAR1JTJ"
title = "Attribute removed lines from the base revision"
status = "pending"
priority = "high"
position = 7
+++

A hunk that only removes lines has no current line, so impact records a removed_lines uncertainty and names no definition. Parse the before blob for those hunks and list, in before coordinates, the definitions that were removed and the definitions that enclosed removed lines, so impact can seed the callers of a deleted function.

Acceptance criteria:
- Deleting a whole function lists it as removed.
- Deleting lines in a function body lists the enclosing function.
- A removal at the start of a file lists the removed definition, not the next one.
