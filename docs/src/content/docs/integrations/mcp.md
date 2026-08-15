---
title: MCP
description: Give MCP clients bounded, read-only Dalil repository context.
section: Integrations
group: Integrations
order: 8
toc:
  - title: Run the server
    slug: run-the-server
    level: 2
  - title: Configure a client
    slug: configure-a-client
    level: 2
  - title: Available tools
    slug: available-tools
    level: 2
  - title: Read-only behavior and limits
    slug: read-only-behavior-and-limits
    level: 2
---

# MCP integration

`dalil-mcp` exposes Dalil's typed reports through the Model Context Protocol
(MCP) over standard input and output. It is for clients that cannot call
`dalil-core` directly. Native Rust hosts should use the core API instead.

## Run the server

Build the MCP adapter from a Dalil source checkout:

```sh
cargo build --locked --release -p dalil-mcp
```

Start `target/release/dalil-mcp` from the directory your MCP client uses. Each
repository tool accepts an optional `path`; it defaults to the server's current
directory.

The transport is newline-delimited JSON-RPC on standard input and output. Do
not write logs or other text to standard output.

## Configure a client

Configure a local stdio MCP server with the compiled binary. The exact setting
name depends on the client; this is the common shape:

```json
{
	"mcpServers": {
		"dalil": {
			"command": "/absolute/path/to/dalil/target/release/dalil-mcp"
		}
	}
}
```

Use an absolute command path when the client does not inherit the expected
working directory or `PATH`.

## Available tools

| Tool                 | Use                                                             |
| -------------------- | --------------------------------------------------------------- |
| `dalil_orient`       | Get a short first-reading sequence.                             |
| `dalil_map`          | Inspect a bounded repository map.                               |
| `dalil_context`      | Gather task-shaped evidence, including optional teaching steps. |
| `dalil_impact`       | Review local revision or dirty-worktree context.                |
| `dalil_explain`      | Understand why one path or symbol matters.                      |
| `dalil_search`       | Find a few path, symbol, or concept anchors.                    |
| `dalil_capabilities` | Check supported languages, query packs, and limits.             |
| `dalil_cache_status` | Inspect user-cache metadata.                                    |

Repository tools share task inputs such as `path`, `task`, `symbols`,
`task_paths`, `changed_paths`, `changed_symbols`, `budget`, and `profile`.
`dalil_context` and `dalil_impact` also accept `base`, `head`,
`revision_range`, and `dirty_worktree`. `dalil_search` accepts either `query`
or an exact `symbol`.

Tool responses put a short summary in text content and the typed report in
`structuredContent`. The structured report has the same meanings as the CLI
JSON report: provenance identifies the repository state and analysis settings;
uncertainty, omissions, collection summaries, quality, and budget fields
qualify the selected evidence.

## Read-only behavior and limits

Dalil reads local repositories and Git objects through its embedded library. It
does not run Git, hooks, filters, repository programs, or network transports.
It does not write in the analyzed repository.

The MCP adapter exposes cache status only. It does not offer cache clearing or
pruning. Analysis may refresh Dalil's user cache unless the request selects
`cache: "disabled"`.

Every tool uses the core's file, byte, syntax, history, elapsed-work, and
output limits. The adapter rejects text values longer than 4,096 bytes, lists
longer than 64 items, and budgets above 100,000. It runs at most four requests
at a time. A cancelled MCP request maps to a tool error after Dalil reaches a
cooperative operation boundary.

Dalil returns lexical and structural evidence, not compiler-resolved semantics.
Read the recommended source and its reported limitations before making a change.
