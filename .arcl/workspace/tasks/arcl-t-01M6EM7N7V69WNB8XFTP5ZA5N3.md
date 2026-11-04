+++
id = "arcl-t-01M6EM7N7V69WNB8XFTP5ZA5N3"
phase = "arcl-ph-01M65H08QHQG2WE9TPFPAR1JTJ"
title = "Read path qualifiers outside Rust"
status = "in_progress"
priority = "high"
position = 5
+++

Rust references record the path before their name (`map` in `map::f()`), and graph construction resolves it as module evidence. Other languages have the same pattern: Python `module.f()`, Go `pkg.F()`, Ruby `Module::f`, C# and Java namespaces, Lua `require` tables.

Acceptance criteria:

- [ ] Each language with a module-qualified call form records its qualifier
- [ ] A fixture per language resolves a qualified cross-file call without an import
