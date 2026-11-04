+++
id = "arcl-c-01M6JAS83J7ESN4J61497Y8Y44"
title = "Split plain search queries into terms"
status = "captured"
created-at = "2026-11-04T03:48:59.122Z"
+++

`dalil search <query>` matches a plain query as one substring (`filters.text.mode = substring`). A sentence such as a commit subject matches nothing: in `cargo replay`, all 120 cases on Dalil and fd returned no results, so `dalil search` scored zero recall. `context` already derives search terms from `--task`. Consider tokenizing plain queries the same way, or ranking by term matches, and rerun the replay to compare.
