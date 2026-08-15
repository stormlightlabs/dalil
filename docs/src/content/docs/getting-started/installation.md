---
title: Install Dalil
description: Build the Dalil CLI from a source checkout.
section: Get started
group: Getting started
order: 1
---

Dalil runs against a Git worktree. It reads the repository and its committed
history; it does not change either one.

## Build from source

Clone Dalil and build it with its committed dependency graph:

```sh
git clone https://github.com/stormlightlabs/dalil.git
cd dalil
cargo build --locked --release
```

The binary is at `target/release/dalil`. Run it from the Git worktree you want
to inspect, or give it that worktree as its path.

## Check the environment

`dalil doctor` checks repository discovery, cache access, the embedded schema,
and the available query packs:

```sh
dalil doctor .
dalil capabilities --json
```

Use JSON when another tool needs the result as typed data.
