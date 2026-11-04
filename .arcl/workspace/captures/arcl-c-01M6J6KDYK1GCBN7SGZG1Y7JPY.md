+++
id = "arcl-c-01M6J6KDYK1GCBN7SGZG1Y7JPY"
title = "Narrow Go imported-module edges to the qualifier's import"
status = "captured"
created-at = "2026-11-04T02:35:54.195Z"
+++

A Go call `a.F()` links to `F` in the directory of any import in the file, so a file that imports `app/a` and `app/b` links `a.F()` to both. References now record `a` as their qualifier (arcl-t-01M6EM7N7V69WNB8XFTP5ZA5N3); resolve it against the import that binds `a`, including aliases, and keep only that directory.
