---
title: Evaluation
description: Replay real commits to measure how often Dalil names the files a change touched.
section: Reference
group: Reference
order: 7
---

`cargo replay` treats each commit as a test case. The commit's subject is the
task, and the files it changed are the answer. Dalil answers from a temporary
checkout of the commit's parent, so it sees the tree and the history as they
were before the change.

```sh
cargo replay --repo <path> --commits 60 [--rev <revision>] [--out <dir>]
```

## Cases

The harness walks back from `--rev` (default `HEAD`), newest commit first. It
skips merge commits, root commits, and commits that change no files or more
than 50. It also skips a commit whose parent tree cannot be written safely: an
entry name that a macOS or Windows filesystem reads as `.git`, such as `GIT~1`,
or two paths that differ only in case or Unicode normalization. It stops after
`--commits` cases.

Each case reads the parent's tree with `gix` into a new directory under the
system temporary directory, named `dalil-replay-<pid>-<random suffix>` and
readable only by its user. The directory holds a `.git` whose objects come
from the source repository through `objects/info/alternates`, with `HEAD` on
the parent commit. Blobs are written straight from the object database. No
`git` process, checkout, hook, filter or attribute runs, and no index is
written. The directory is removed when the case ends or on Ctrl-C; a second
Ctrl-C exits at once and leaves it. A later run removes those that the same
user's killed runs left, and warns about any it cannot remove.

## Commands

Each case runs three rankers against the checkout:

| Ranker          | What is ranked                                                        |
| --------------- | --------------------------------------------------------------------- |
| `dalil context` | `context.files[].recommendation.path` from `--task <subject>`         |
| `dalil search`  | `search.matches[].path`, repeats dropped, from `--limit 10 <subject>` |
| baseline        | files scored by subject keywords in their path and content            |

Dalil runs with `--json --no-cache` and its own `XDG_CACHE_HOME`, so no case
reuses another's cache. `results.json` records each command line. The harness
builds Dalil with `cargo build --release` unless `--dalil <path>` names a
binary.

The baseline drops a Conventional Commits prefix from the subject, lowercases
it and splits it into words of three or more characters, less a stopword list.
A file scores 10 for each word its path contains, plus each word's count in its
content, capped at 10 per word. Binary files and files over 1 MiB score on the
path only. `xtask/src/replay/baseline.rs` holds the rules.

## Measures

Recall@k is the share of the commit's changed files found in the first k
ranked paths. It is reported twice. The first counts every changed file,
including files the commit added, which no ranker can name from the parent.
The second, "in parent", counts only the changed files that exist in the parent
tree, over the cases that have at least one. A failed run counts as zero
recall.

Bytes are the stdout bytes of Dalil's `--json` report. For the baseline they
are its top 10 paths, one per line, so the two are not comparable. Latency is
the wall time of the Dalil process, or of the baseline's reading and ranking.
The summary gives the median of each over successful runs.

## Results

Measured on 2026-11-04 with Dalil 0.1.0 built from `4ac74b4`, at its
defaults (budget 1000, search limit 10), one run per case on a Linux container.
The `dalil search`, terms rows rerun the same cases with Dalil built from the
change that splits a plain query into ranked terms (branch commit `38ac9a1`).

Dalil at `4ac74b4c77ebc8c7aa7c8c1e151de88bfeee229d`: 60 cases, 2 commits
skipped for changing more than 50 files.

| Ranker                    | Recall@5 / @10 | In parent @5 / @10 | Median bytes | Median ms |
| ------------------------- | -------------- | ------------------ | -----------: | --------: |
| `dalil context`           | 6.1% / 6.1%    | 8.4% / 8.4%        |        10568 |      1734 |
| `dalil search`, substring | 0.0% / 0.0%    | 0.0% / 0.0%        |        11046 |      1837 |
| `dalil search`, terms     | 12.2% / 12.2%  | 14.2% / 14.2%      |        21098 |      2067 |
| baseline                  | 23.5% / 38.1%  | 26.5% / 43.4%      |          214 |        14 |

[sharkdp/fd](https://github.com/sharkdp/fd) (MIT or Apache-2.0) at
`cc4e40fa4e32d61733cd72926e233a3fb6c8d56d`: 60 cases, 36 merge commits
skipped.

| Ranker                    | Recall@5 / @10 | In parent @5 / @10 | Median bytes | Median ms |
| ------------------------- | -------------- | ------------------ | -----------: | --------: |
| `dalil context`           | 3.3% / 3.3%    | 3.3% / 3.3%        |        10266 |       570 |
| `dalil search`, substring | 0.0% / 0.0%    | 0.0% / 0.0%        |        10606 |       438 |
| `dalil search`, terms     | 21.8% / 21.8%  | 21.9% / 21.9%      |        20791 |       521 |
| baseline                  | 77.6% / 93.0%  | 77.8% / 93.3%      |          144 |         8 |

`dalil context` recommended one or two files per case. Of its 140
recommendations, 139 were `README.md`, `AGENTS.md` or `Makefile`. Built from
`4ac74b4`, `dalil search` matched a plain query as one substring, so no subject
returned a result. With terms, it returned a median of 3 paths per
case and at most 5, because the 1000-token budget filled first; recall@10
equals recall@5 for that reason. fd has 61 tracked files, so its baseline top
10 covers a sixth of the tree.

## Rerun

The numbers above are for Dalil built at `4ac74b4`. That commit predates
`cargo replay`, so build Dalil there in a separate worktree and pass the binary
with `--dalil`:

```sh
git worktree add /tmp/dalil-4ac74b4 4ac74b4c77ebc8c7aa7c8c1e151de88bfeee229d
cargo build --release --package dalil --manifest-path /tmp/dalil-4ac74b4/Cargo.toml
dalil=/tmp/dalil-4ac74b4/target/release/dalil
cargo replay --dalil "$dalil" --repo . --rev 4ac74b4c77ebc8c7aa7c8c1e151de88bfeee229d --commits 60
git clone https://github.com/sharkdp/fd.git /tmp/fd
cargo replay --dalil "$dalil" --repo /tmp/fd --rev cc4e40fa4e32d61733cd72926e233a3fb6c8d56d --commits 60
```

For the terms rows, pass a Dalil built from `main` after term splitting merged,
with the same `--rev` values.

Each run writes `results.json` and `summary.md` to `target/replay/<name>`, or
to `--out`.
