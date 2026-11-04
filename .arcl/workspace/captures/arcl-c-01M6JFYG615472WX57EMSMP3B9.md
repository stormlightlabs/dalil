+++
id = "arcl-c-01M6JFYG615472WX57EMSMP3B9"
title = "Search token budget returns about 3 results"
status = "captured"
created-at = "2026-11-04T05:19:14.113Z"
+++

At the default `--budget 1000`, `dalil search --limit 10` returned a median of 3 paths per case in the replay harness on Dalil and fd (max 5), so recall@10 equals recall@5. Each match carries the full symbol (context, scope, location), the evidence list and per-file limitations, and the budget counts all of it. Options: count only the fields Markdown shows, drop the symbol context from JSON matches, or raise the default budget for search.
