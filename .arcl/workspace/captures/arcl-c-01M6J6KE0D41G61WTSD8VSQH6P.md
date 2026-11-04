+++
id = "arcl-c-01M6J6KE0D41G61WTSD8VSQH6P"
title = "Join qualified-path relationships per reference"
status = "captured"
created-at = "2026-11-04T02:35:54.253Z"
+++

In report/relationships.rs a qualified-path edge joins every reference of its name that has a qualifier. In Java, C#, Python, Go and Lua a variable receiver such as `x` in `x.baz()` is also recorded as a qualifier, so a file with both `Strings.baz()` (resolved) and `x.baz()` (unknown receiver) joins both calls to the qualified target. Recording which references an edge came from would make the join exact.
