+++
id = "arcl-t-01M6EM7TCJ467XCMX12DASHF8G"
phase = "arcl-ph-01M65H08RDC7CBQ0Z8ATRYA2KY"
title = "Index with the same limits under every profile"
status = "pending"
priority = "normal"
position = 3
blocked-by = ["arcl-t-01M65H08XYV97ZE71YFM3YTB9H"]
+++

The compact profile indexes with a 128-symbol per-file limit; definitions and calls are exempt, other references are not. A uniform limit cost 43% more cache and evicted the cache past its old 32 MiB budget. Revisit once the postcard store exists.

Acceptance criteria:

- [ ] Index limits no longer depend on the profile; the profile shapes only output
- [ ] Store size and warm query time on Dalil are recorded before and after
