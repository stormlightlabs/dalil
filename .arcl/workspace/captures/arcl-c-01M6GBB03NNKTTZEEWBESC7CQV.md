+++
id = "arcl-c-01M6GBB03NNKTTZEEWBESC7CQV"
title = "Treat Lua and Zig per-file reference caveats as method limitations"
status = "captured"
created-at = "2026-11-03T09:20:11.892Z"
+++

parser.rs adds a constant caveat to every Lua file (literal `require` only) and every Zig file (literal `@import` only). Human output repeats it per file under that file's evidence in `orient`, `search` and `map --profile evidence`. PR #6 left them out because the per-file renderers would need to drop them and point to `--json` without a report-level note. Move both into `dalil_core::limitations` and filter them in the per-file renderers.
