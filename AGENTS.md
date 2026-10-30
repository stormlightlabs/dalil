# AGENTS.md

<!-- fabrial:begin -->
## Branches and pull requests

This repository uses fabrial. `fab init` wrote this section and the files it
names, `fab update` refreshes them, and `fab check` reports where they have
drifted.

- Never commit to `main`. Push a `claude/<topic>`, `codex/<topic>` or
  `storm/<task>` branch; the Branch PR workflow opens the pull request as a
  draft from stormlight-thunderbot.
- Commits on those branches may carry the agent's attribution trailers. The
  squash commit replaces them with the PR title and number, such as
  `Add Pi dispatch (#21)`, the PR body above its
  `<!-- fabrial:footer` line, `Merged from <branch>.` and the owner's
  `Co-authored-by` line from `.github/fabrial.toml`. Attribution lines and
  session links in the body are dropped, so nothing on `main` carries AI
  attribution. Editing the title or body of a reviewed PR posts the new
  squash message. Auto-merge keeps the message it was turned on with.
- Before pushing, run the command `gates` names in `.github/fabrial.toml`.
- The owner approves the PR and merges it, with the posted squash message or
  their own edit of it. Agents never approve or merge.
- Keep the PR body to what changed and why. Put verification in a PR comment.
  Leave out attribution lines and session links.
- Agents post PR comments as stormlight-thunderbot through the Bot comment
  workflow (`.github/workflows/comment.yml`), never with their own GitHub
  access.
- The agent that opens a PR reviews it with the `storm` skill and ends with a
  `Review finished at <sha>` comment. That turns `storm/review` green for that
  head, marks the draft ready and sets the squash message in the bot's status
  comment, which it edits rather than posting anew. The owner merges, by hand
  or with auto-merge they turn on; agents and the gate never turn it on.
  `/storm:ready <sha>` from the owner, a member or a collaborator accepts
  that head as it is, and `/storm:stop` turns auto-merge off.

## Writing

Everything here is read by a person: docs, commit messages, PR titles and
bodies, comments, and replies. Keep it short and plain:

- Lead with the point, and say each thing once.
- Use short sentences, common words, and one term for one thing.
- Cut preambles, summaries of what was just said, and sign-offs.
- Name the property, the limit or the failure, not a judgment of it.
- Stay inside the length targets: a PR body around 150 words, a review comment
  200, any other comment 100.

`.claude/skills/writing-docs/` holds the full standard, and its
`references/tells.md` lists the patterns that make text read as generated.
Check a draft against both before committing or posting.
<!-- fabrial:end -->

## Tasks

The installed skills read this section for the tracker's commands. Dalil's
tasks live in arcl, in tracked files under `.arcl/workspace`; `arcl.db` is a
local cache. In a cloud session `.claude/hooks/session-start.sh` installs the
pinned `arcl` (0.3.0) and builds the cache in the session's checkout only.
Elsewhere, install the same release:

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/stormlightlabs/arclightning/releases/download/v0.3.0/arcl-cli-installer.sh | sh
```

Every task worktree, cloud sessions included, needs `arcl init` before its
first `arcl` command.

| Step              | Command                                                       |
| ----------------- | ------------------------------------------------------------- |
| Read a task       | `arcl task show <id>` and `arcl context <id>`                 |
| Check readiness   | `arcl ready`, and `arcl task explain <id>` for one task       |
| Start             | `arcl init`, then `arcl task start <id>`, in the task's worktree |
| Complete          | `arcl task complete <id> --evidence "Done in #<n> at <sha>: <summary>"` |
| Hand off          | `arcl task handoff <id> --note-file <file>`                   |
| Pick up           | `arcl context <id>`, `arcl task unpark <id>`, `arcl task start <id>` |
| Record new work   | `arcl task create --phase <id> "<title>"`, or `arcl capture create` |

- Every change to a task is a file change: commit it on the branch it
  describes, so the status reaches `main` with the merge. Record new tasks and
  bugs found along the way on the branch in progress, never in a pull request
  of their own.
- `arcl task start` refuses a task that is not ready. Pass `--force` only when
  the owner asks after seeing the reasons `arcl task explain` gives.
- A handoff parks the task. Unpark only when `arcl task explain` gives parking
  as the only reason it is not ready.
- When a merge conflicts on a task file, keep the newer status.
