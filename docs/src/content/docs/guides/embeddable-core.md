---
title: Embed Dalil
description: Call Dalil's typed analysis operations from a native host.
section: Guides
group: Guides
order: 7
---

# Embeddable core API

`dalil-core` provides the repository analysis used by the `dalil` CLI. It
accepts typed requests and returns typed report models. It does not parse CLI
arguments, render Markdown or HTML, or implement a transport protocol.

## Call an operation

Build an `AnalysisRequest` with a `CommandDescriptor`, then call the matching
operation. The specialized operations return their result model directly.

```rust
use std::path::PathBuf;

use dalil_core::{AnalysisRequest, CommandDescriptor, map};

let root = PathBuf::from("/path/to/repository");
let request = AnalysisRequest::new(CommandDescriptor::map(root));
let repository_map = map(request)?;
```

The available operations are `orient`, `map`, `context`, `impact`, `explain`,
`search`, `capabilities`, and `cache`. Use `analyze` when an adapter needs the
full `Report` envelope.

`AnalysisRequest` contains the task seeds, profile, budget, cache policy, and
history settings used by the CLI. Results carry recommendations, evidence,
limitations, provenance, and quality fields. Adapters should render those
fields rather than reconstructing their own rankings or warnings.

## Execution control

Pass an `ExecutionControl` to `analyze_with_control` to receive `Started` and
`Completed` progress events. `CancellationToken` is cooperative and checked at
operation boundaries. Run a request in a separate task when a host needs to
stop in-progress filesystem or Git reads immediately.

Budgets come from the request's map settings and are reflected in collection
summaries and report quality. Partial analysis and omissions appear in the
returned models. `CoreError` distinguishes cancellation, an operation/request
mismatch, and the underlying analysis error.

The core is read-only with respect to the analyzed repository. Cache commands
write only to Dalil's user cache.

## Host lifecycle calls

Native hosts can use `LifecycleEvent`, `LifecycleRequest`, and `dispatch` for
host-owned lifecycle notices. The adapter has no background worker or daemon.
The host decides whether to call Dalil, show the returned report, or cancel it.

| Host event                           | Core operation |
| ------------------------------------ | -------------- |
| repository open, session start       | `orient`       |
| task change, before edit, after edit | `context`      |
| before review                        | `impact`       |

A lifecycle event accepts only its matching operation, so a review notice
cannot accidentally return orientation data. Pass changed paths or revision
inputs through the `AnalysisRequest`; Dalil does not inspect editor buffers or
host session state. After a saved edit, make another context request. The
normal cache settings reuse unchanged index records and refresh changed files.

```rust
use std::path::PathBuf;

use dalil_core::{
    AnalysisRequest, CommandDescriptor, LifecycleEvent, LifecycleRequest, dispatch,
};

let request = AnalysisRequest::new(CommandDescriptor::orient(PathBuf::from("/path/to/repository")));
let notice = dispatch(LifecycleRequest::new(LifecycleEvent::RepositoryOpen, request)?)?;
assert_eq!(notice.event, LifecycleEvent::RepositoryOpen);
```

`dispatch_with_control` accepts the usual `ExecutionControl` for cooperative
cancellation and progress. Its `LifecycleNotice.report` is the normal `Report`
from `analyze`, preserving CLI and MCP analysis semantics.
