+++
id = "arcl-c-01M6J6KDZEE93XK09M5AZ0HT1C"
title = "Strip __init__ from Python paths in imported-module matching"
status = "captured"
created-at = "2026-11-04T02:35:54.222Z"
+++

`normalize_module_hint` trims `/__init__` before `.py`, so `pkg/other/__init__.py` normalizes to `pkg/other/__init__` and the imported-module rule never matches a package's `__init__.py`. With `from pkg import other`, `other.g()` linked only to a sibling `pkg/decoy.py`. The qualified-path rule now strips it (`module_path` in graph.rs); the shared normalization still does not.
