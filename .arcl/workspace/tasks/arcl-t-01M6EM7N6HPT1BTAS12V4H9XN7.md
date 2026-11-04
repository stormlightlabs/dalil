+++
id = "arcl-t-01M6EM7N6HPT1BTAS12V4H9XN7"
phase = "arcl-ph-01M65H08QHQG2WE9TPFPAR1JTJ"
title = "Tag only callee positions as calls"
status = "completed"
priority = "normal"
position = 4
evidence = "Done in #7 at e125618: only callee positions get call evidence in all 10 languages; member calls on unknown receivers link only to methods at low confidence. On Dalil, callers of insert and label have no high or medium false edges."
+++

`is_call_like` in `map/parser.rs` walks up from any identifier to the nearest call expression, so arguments and the module path in `map::f()` are tagged as calls. A call to anything named `map` then gains a false caller.

Acceptance criteria:

- [ ] Only the callee of a call expression gets call evidence, in every supported language
- [ ] Fixtures cover an argument, a qualified path and a method receiver
- [ ] `dalil relationships callers` on Dalil returns no caller whose call site is an argument
