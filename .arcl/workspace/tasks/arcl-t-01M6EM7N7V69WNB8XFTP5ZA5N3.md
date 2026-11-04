+++
id = "arcl-t-01M6EM7N7V69WNB8XFTP5ZA5N3"
phase = "arcl-ph-01M65H08QHQG2WE9TPFPAR1JTJ"
title = "Read path qualifiers outside Rust"
status = "completed"
priority = "high"
position = 5
evidence = "Done in #8 at 3988611: Ruby, Java, C#, Python and Lua record and resolve path qualifiers inside the caller's project root; Go records its qualifier. Fixtures per language pass; Python and Go use imports, which valid code needs."
+++

Rust references record the path before their name (`map` in `map::f()`), and graph construction resolves it as module evidence. Other languages have the same pattern: Python `module.f()`, Go `pkg.F()`, Ruby `Module::f`, C# and Java namespaces, Lua `require` tables.

Acceptance criteria:

- [ ] Each language with a module-qualified call form records its qualifier
- [ ] A fixture per language resolves a qualified cross-file call without an import
