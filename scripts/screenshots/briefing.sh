#!/usr/bin/env bash
set -euo pipefail

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo_root"

target/debug/dalil --no-cache --budget 350 . |
	freeze \
		--language markdown \
		--theme kanagawa-dragon \
		--window \
		--width 1200 \
		--wrap 100 \
		--padding 24 \
		--margin 24 \
		--background '#141415' \
		--border.radius 12 \
		--output docs/static/dalil-briefing.png
