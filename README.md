# Dalil

Dalil (Arabic for “guide”) is a local codebase reference engine for humans and
coding agents. It finds the files, symbols, relationships, tests, and history
that are most useful for the task at hand.

![Dalil HTML repository briefing shown in a browser](./assets/dalil-report.png)

Dalil supports Rust, JavaScript, JSX, TypeScript, TSX, Python, Ruby, Java, C#,
Go, Lua, and Zig. It reads local files and Git objects without running project
code, hooks, filters, repository programs, or network transports.

## Install

Install the published crate:

```sh
cargo install --locked dalil
```

To build a source checkout instead:

```sh
cargo build --locked --release
```

## Quick start

Run Dalil from the Git worktree you want to understand:

```sh
dalil
dalil context --task 'fix parser cache invalidation' --changed-path src/map/cache.rs
dalil impact --revision-range 'HEAD~1..HEAD'
dalil search parser
dalil explain src/map.rs
```

`PATH` defaults to the current directory. Dalil discovers the enclosing Git
repository and keeps analysis inside it.

The main workflows are:

- `dalil` or `dalil orient` returns a short repository briefing and reading
  plan.
- `dalil map` inventories source, symbols, lexical relationships, project
  roots, entry points, tests, and other landmarks.
- `dalil context` selects one task-shaped evidence bundle.
- `dalil impact` prepares a review list for a revision range or dirty worktree.
- `dalil search` finds path, symbol, or concept anchors. `dalil explain` shows
  the evidence behind one recommendation.
- `dalil history` reports bounded churn, contributor, bug-cluster, activity,
  and firefighting signals.

Pass `--json` for typed output or `--html` for a standalone browser report:

```sh
dalil map --profile evidence --json > map.json
dalil --html > dalil-report.html
dalil --html --open
```

Reports go to stdout and diagnostics go to stderr, so redirection writes a
clean report file.

## Repository evidence bundles

`dalil export` writes a persistent snapshot under `.dalil/`:

```text
.dalil/
├── map.json
└── map.md
```

`map.json` is the complete portable evidence map. `map.md` is a shorter view of
the same snapshot. Treat both as generated working data by default: ignore them
locally or upload them as CI artifacts unless the repository has a specific
reason to version the full map.

See [repository evidence bundles](docs/src/content/docs/guides/repository-evidence-bundles.md)
for refresh, freshness, sharing, and Git guidance.

## Integrations

The CLI calls typed operations in `dalil-core`. Native adapters can use the
[core API](docs/src/content/docs/guides/embeddable-core.md), coding agents can
install the [Dalil skill](crates/dalil-cli/skills/dalil/SKILL.md), and MCP
clients can run the separate [`dalil-mcp` adapter](docs/src/content/docs/integrations/mcp.md).

## Documentation

- [Installation](docs/src/content/docs/getting-started/installation.md)
- [Quick start](docs/src/content/docs/getting-started/quick-start.md)
- [Repository orientation](docs/src/content/docs/guides/default-briefing.md)
- [Task-shaped context bundles](docs/src/content/docs/guides/context-bundles.md)
- [Source analysis, limits, and caching](docs/src/content/docs/reference/source-analysis.md)
- [Git-history evidence](docs/src/content/docs/reference/history.md)
- [Manifests and entry points](docs/src/content/docs/reference/manifests.md)
- [Report formats and schemas](docs/src/content/docs/reference/report-formats.md)
- [Agent integration](docs/src/content/docs/integrations/agents.md)

## Inspiration

- [Aider's repository map](https://aider.chat/docs/repomap.html)
- [codebase orient skill](https://github.com/DrCatHicks/learning-opportunities/tree/main/orient)
- [The Git Commands I Run Before Reading Any Code](https://piechowski.io/post/git-commands-before-reading-code/)
