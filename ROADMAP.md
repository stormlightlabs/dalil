---
title: "Dalil roadmap"
status: "in-progress"
updated: "2026-10-30"
---

Dalil is a local repository guide for coding agents, usable by people through
the same commands. It answers three questions an agent meets on every task:

1. What is this repository, and where do I start?
2. How was a change like this one made before?
3. What did my change miss: companion files, related tests, the command to
   run them?

Dalil answers from the repository's own Git history first, and from code
search and the symbol graph second. It runs offline, does not call a model,
and does not execute repository code.

Open work lives in arcl, under `.arcl/workspace`, in the "Roadmap" plan. This
file holds the decisions behind it. `arcl ready` lists what can start.

## Why history first

On 2026-10-30 we ran Dalil on its own repository (222 files, 50 commits):

- `dalil context --task 'add a new language (Kotlin) support'` recommended
  only `README.md`.
- `dalil relationships callers language_capabilities` returned 0 results.
  `grep` finds 2 call sites in 2 files.
- `dalil impact` listed identifiers such as `writeln` and `expect` as changed
  symbols.
- A warm `map`, `orient` or `search` took 0.8 to 0.9 s, against about 10 ms for
  `grep`. The cache was 23 MB of JSON.

The commit `e11dc77 feat: Zig structural map support` touched the files a
Kotlin change needs: `Cargo.toml`, `map/languages.rs`, `map/parser.rs`,
`landmarks.rs`, the Zig query files and both test suites. Dalil read that
commit but used it only for churn counts.

A lexical symbol graph is less precise than a language server and less
flexible than `grep`, and several tools already ship one. The
[Codebase-Memory paper](https://arxiv.org/pdf/2603.27277) measured a
tree-sitter graph over MCP against a grep-and-read agent: half the tool calls,
lower answer quality (0.83 against 0.92). History is a signal agents cannot
get from `grep`, and Dalil already parses it.

## Position

| Tool       | What it does                                                       | When it runs          |
| ---------- | ------------------------------------------------------------------ | --------------------- |
| CodeRabbit | Hosted LLM pull request review over a code graph                   | After a PR opens      |
| Greptile   | Hosted LLM review, API and MCP over a code graph                   | After a PR opens      |
| Graphify   | Local tree-sitter knowledge graph, `graph.json`, report, HTML view | On demand             |
| Dalil      | Local history-first retrieval, no model                            | While the agent works |

CodeRabbit and Greptile judge a finished change. Dalil points at files while
the change is being written, so the two kinds of tool can run together.
Graphify overlaps with Dalil's export bundle and graph. Dalil does not compete
there: `export`, `map.md` and `--html` stay, but get no new scope.

## Product claim

Dalil names the files a real change touched, measured as recall@k over replayed
commits, with a warm query under 50 ms and no model. Every retrieval change is
judged against that measurement.

## Design

### Repository store

One private store holds everything Dalil indexes. Exported formats, such as
`map.json`, are projections of it and keep their own schema.

- Paths, symbols, commits and authors are interned. `PathId`, `SymbolId`,
  `CommitId` and `AuthorId` are `u32` indices into one string table. Other
  structures hold only these integers.
- The store is encoded with `postcard`, behind a header with a format version
  and a checksum of the encoded body. A version mismatch or a failed checksum
  rebuilds the store; `postcard` alone can decode a corrupted body without an
  error. Memory-mapping is considered only if load time still dominates.
- The trigram index, symbol index and relationship graph move to interned IDs.
  Today `FileId` is a `String`, so every trigram posting repeats a full path.

### Commit table

A commit never changes, so its row is keyed by its oid and written once.
`CommitId` is the order of ingest, not of history.

```rust
struct CommitRec {
    oid: [u8; 20],
    time: i64,
    author: AuthorId,
    subject: StrRef,
    parents: Range<u32>, // into commit_parents
    files: Range<u32>,   // into commit_files
}

commits: Vec<CommitRec>
commit_parents: Vec<CommitId>
commit_files: Vec<PathId>     // changed paths of every commit, flat
path_commits: Csr<PathId, CommitId> + Vec<(PathId, CommitId)>  // sealed + appended
```

A warm run walks every parent from `HEAD` and stops on each branch of the walk
at a commit whose oid is already stored. Side branches behind a merge are
ingested, and switching branches, rebasing or force pushing only adds the
commits not yet seen. Ingest costs time proportional to the new commits.

Rows are never dropped because `HEAD` stops reaching them. A query marks the
commits reachable from `HEAD` in a bitset, computed from `commit_parents` and
cached per `HEAD`, and ignores the rest. Merge commits keep their parents but
no file lists, as today.

New rows for `path_commits` go to the append buffer. Compaction merges the
buffer into the sealed CSR once it exceeds a tenth of the sealed size, so its
cost is spread over the commits that filled it. Compaction also drops the
rows of commits no local branch, tag or `HEAD` reaches, so rewritten history
does not grow the store without bound.

Rename detection runs once per commit at ingest, using `gix` rewrite tracking.
It records each rename with its `CommitId`, and forwarding follows the renames
reachable from the commit asked about, so history recorded under an old name
counts toward that commit's name for the file.

### Co-change

Co-change is computed per query and not stored as a matrix. Given seed files,
walk their reachable `path_commits`, count the other files in each commit and
weight each commit by `1 / files_in_commit`. Commits that touch more than a
configured number of files (default 50) are skipped, since formatting passes
and mass renames carry no pairing signal. The work is limited by the seeds'
history, not by repository size. A per-path cache of the top 32 neighbors is
added only if profiling shows the query is too slow.

### Commit-message index

An inverted index maps terms in each commit's subject and body to `CommitId`s,
ranked with BM25. A task's text finds the best matching commits. Their files,
passed through the rename table, become candidates. This answers "how was a
change like this made before".

### Fusion

A recommended file's score combines four ranked lists: commit-message matches,
co-change with the seed files, trigram and path search, and graph proximity.
Start with reciprocal rank fusion, which needs no weights. The evaluation
decides whether learned weights are worth adding.

### Evaluation

Each historical commit `i` is a test case: its subject is the task, and the
files it touched are the expected answer. Dalil must answer as it would have
at `i`'s parent, and the two kinds of list get there differently.

- History lists, the commit-message index and co-change, are masked. The
  reachability bitset is computed from `i`'s parent instead of `HEAD`, so `i`
  and every later commit drop out. BM25 document counts and IDF come from the
  masked set, and renames are forwarded as of the parent.
- Code lists, trigram and path search and the graph, describe a tree and
  cannot be masked: today's tree holds the files `i` created. They are built
  from the parent's tree for each case.

The harness reads the parent's tree with `gix` into a temporary directory.
It runs no hooks, filters or checkout, and removes the directory when the case
ends, when it is interrupted, or at the next start after a killed run.

The harness reports recall@5, recall@10, bytes returned and latency per
query. It compares each Dalil command against a baseline of keyword search
plus path ranking. Routine CI runs on small synthetic repositories. A larger
tier runs on pinned public repositories before a release.

## Targets

On a repository with 10,000 files and 50,000 commits:

- A warm query takes under 50 ms at the median.
- History ingest after a pull takes time proportional to the new commits,
  with compaction spread over the commits that filled the buffer.
- The store is at most a quarter of the size of today's JSON cache for the
  same repository.

Cold indexing gets a published time budget once the store exists and can be
measured.

## Agent surface

The MCP server and the skill expose a few tools, one per question at the top
of this file, plus search. Their names and parameters are still open; the
current CLI command names are not the target. Every collection a tool returns
has a default limit and reports totals and omissions.

Human output keeps the same evidence. The standard limitation text moves out
of the default output into `--json` and `explain`.

## Distribution

Dalil has not been released and will not be published to crates.io. Releases
use fabrial's flow: a Release PR workflow writes the version and
`CHANGELOG.md`, the merge is tagged, and the tag builds binaries with `dist`,
creates the GitHub release and updates the Homebrew tap. The current
`release.yml` is replaced.

## Deferred

These stay out of the plan until the evaluation shows they help:

- Structural analysis: centrality, communities and dependency components.
- Architecture and subsystem projections.
- New export formats and HTML views.
- Language-server or SCIP providers for precise references.
- More languages.

## Principles

1. Outputs have stated limits and report what they omit.
2. CLI, MCP and the skill call the same typed operations in `dalil-core`.
3. Analysis is offline and deterministic. It does not execute project code,
   Git hooks, filters, pagers, credential helpers or network transports.
4. Ordinary commands do not write to the repository. Only `dalil export`
   writes, and only under `.dalil/`.
5. Derived results keep their provenance and confidence. A lexical match is
   not reported as a resolved call.
6. A dependency is added only when it simplifies a committed capability.

## Technical constraints

- Rust edition 2024 with MSRV 1.85.
- Dependencies point from `dalil-cli` and `dalil-mcp` into `dalil-core`.
- The store layout is private and versioned apart from exported schemas.
- Fixtures and benchmarks use public or synthetic repositories only.
- The gates are the command `gates` names in `.github/fabrial.toml`.
