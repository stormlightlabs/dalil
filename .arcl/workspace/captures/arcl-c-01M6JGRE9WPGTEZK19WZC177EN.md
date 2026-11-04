+++
id = "arcl-c-01M6JGRE9WPGTEZK19WZC177EN"
title = "Sync the embedded schema copy with schema/v1/dalil.json"
status = "captured"
created-at = "2026-11-04T05:33:24.156Z"
+++

crates/dalil-core/schema/v1/dalil.json lacks search_results.request.filters, request.revision and search_results.query, which schema/v1/dalil.json has. Found while revising #10, which synced only matched_terms. Sync the copies and add a test that fails when they differ.
