+++
id = "arcl-t-01M6H7WWZR8T4PGNAR9KECS17R"
phase = "arcl-ph-01M65H08QHQG2WE9TPFPAR1JTJ"
title = "Resolve member calls by receiver type"
status = "pending"
priority = "high"
position = 8
+++

A member call on a receiver other than `self`, `Self`, `this`, `super`, `base`, `cls` or an imported module resolves by name alone. Since #7 it links only to methods, at low confidence with reason `member_receiver_unknown`, so `m.insert()` on a `HashMap` can still reach a repository's `T::insert`.

Infer the receiver's type from evidence the parser already reads: local variable types and initializers (`let t = T::new()`, `let t: T`), parameter annotations (`fn f(t: &T)`), and struct field types (`self.index.insert()`). When the type is known, link only to methods defined in that type's `impl` or class body, and drop the edge when the type is not defined in the repository.

## Acceptance criteria

- `x.insert()` with `x: HashMap` links to no repository `insert`.
- `t.insert()` with `t: T` links to `T::insert` at medium confidence or higher.
- Calls whose receiver type stays unknown keep the low `member_receiver_unknown` edge.
