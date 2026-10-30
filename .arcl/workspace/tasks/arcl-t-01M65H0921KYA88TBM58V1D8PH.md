+++
id = "arcl-t-01M65H0921KYA88TBM58V1D8PH"
phase = "arcl-ph-01M65H08SQKQ2Z5VNVQ5VFTNQV"
title = "Implement the MCP tools and update the skill"
status = "pending"
priority = "normal"
position = 2
blocked-by = ["arcl-t-01M65H090H89MN55F284C4D8XJ", "arcl-t-01M65H0911CR3RJ4RZ0NGBTX1J", "arcl-t-01M65H091JQ20F9XKBXR392VMD"]
+++

Expose the approved tools through `dalil-mcp` and update the Dalil skill to
use them before broad file reads.

Acceptance criteria:

- [ ] Each tool calls a `dalil-core` operation; the adapter adds no analysis
- [ ] Every collection has a default limit and reports totals and omissions
- [ ] Scripted traces for orientation, a past-change lookup and a missed-file
      check record tool calls and bytes returned
