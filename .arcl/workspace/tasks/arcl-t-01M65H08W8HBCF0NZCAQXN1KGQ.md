+++
id = "arcl-t-01M65H08W8HBCF0NZCAQXN1KGQ"
phase = "arcl-ph-01M65H08QZ0AWDGX26SNY66VYZ"
title = "Replay commits to measure context recall"
status = "completed"
priority = "high"
position = 1
evidence = "Done in #9 at 1b419f4: cargo replay replays commits from parent trees and reports recall, bytes and latency for context, search and a keyword baseline. On Dalil, context R@10 6.1% vs baseline 38.1%; on fd, 3.3% vs 93.0%."
+++

Measure how well Dalil names the files a real change touched, as ROADMAP.md's
**Evaluation** describes. This first version runs on the current code against
each commit's parent tree.

Acceptance criteria:

- [ ] An `xtask` command takes a repository path and a commit count, replays
      non-merge commits with at most 50 changed files, and gives each commit's
      subject as the task
- [ ] Each case reads the parent's tree with `gix` into a temporary directory,
      without `git worktree`, checkout, hooks or filters
- [ ] The directory is removed when the case ends or is interrupted, and
      leftovers from a killed run are removed at the next start
- [ ] It reports recall@5, recall@10, bytes returned and latency for
      `dalil context` and `dalil search`
- [ ] It reports the same numbers for a baseline of keyword search plus path
      ranking
- [ ] Results for Dalil and one pinned public repository are recorded in
      `docs/src/content/docs/reference/evaluation.md`
