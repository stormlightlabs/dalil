+++
id = "arcl-c-01M6JB1S6HPG7Y3XK4Z6ER2K6M"
title = "Leave context budget for task-matched files"
status = "discarded"
created-at = "2026-11-04T03:53:38.769Z"
+++

In `cargo replay` (docs/src/content/docs/reference/evaluation.md), `dalil context` at the default budget of 1000 recommended one or two files per case, and 139 of 140 were README.md, AGENTS.md or Makefile. Recall@5 was 6.1% on Dalil and 3.3% on fd, against 23.5% and 77.6% for a keyword baseline. A spot check on fd for 'fix: let --ignore and --no-hidden override only their half' returned only README.md at budget 1000, and README.md, src/cli.rs, src/main.rs, tests/tests.rs and src/walk.rs at budget 4000: three of the four changed files. Landmarks appear to spend the default budget before task-matched files are selected.
