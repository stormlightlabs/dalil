#!/usr/bin/env bash
# Prepares a Claude Code cloud session on dalil: installs the pinned arcl
# release from its prebuilt installer, then builds the local arcl database
# from .arcl/workspace when the clone has none. Local sessions are left
# alone; install arcl there as AGENTS.md's Tasks section says.
set -euo pipefail

# The arcl release this repository pins. AGENTS.md's Tasks section names
# the same version.
version=0.3.0

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

# arcl goes in a directory of its own, so putting it on PATH moves nothing
# else ahead of the image's tools.
bin="$HOME/.local/share/fabrial/arcl-$version/bin"
arcl=$bin/arcl
if [ "$("$arcl" --version 2>/dev/null)" != "arcl $version" ]; then
  mkdir -p "$bin"
  installer=$(mktemp)
  trap 'rm -f "$installer"' EXIT
  curl --proto '=https' --tlsv1.2 -fsSL --connect-timeout 10 --max-time 120 \
    -o "$installer" \
    "https://github.com/stormlightlabs/arclightning/releases/download/v$version/arcl-cli-installer.sh"
  # The installer's own curl sets no timeout and reads no variable for one,
  # so timeout(1) limits the whole install instead.
  ARCL_CLI_UNMANAGED_INSTALL="$bin" timeout 300 sh "$installer" --quiet
fi
path_line="export PATH=\"$bin:\$PATH\""
if [ "$(command -v arcl)" != "$arcl" ] && [ -n "${CLAUDE_ENV_FILE:-}" ] &&
  ! grep -qxF "$path_line" "$CLAUDE_ENV_FILE" 2>/dev/null; then
  echo "$path_line" >> "$CLAUDE_ENV_FILE"
fi

# The database is local and ignored by git. Build it once from the committed
# workspace; a resumed session keeps the one it has. arcl init refuses a file
# that is not a database, such as one an interrupted init left, so a database
# that fails to open moves aside under a name .arcl/.gitignore ignores, and
# init builds a new one. The workspace in git holds the tasks either way.
cd "$CLAUDE_PROJECT_DIR"
if [ -e .arcl/arcl.db ] && ! "$arcl" tree >/dev/null 2>&1; then
  broken=.arcl/arcl.db-broken-$(date +%Y%m%dT%H%M%S)
  for f in .arcl/arcl.db .arcl/arcl.db-wal .arcl/arcl.db-shm; do
    if [ -e "$f" ]; then
      mv "$f" "$broken${f#.arcl/arcl.db}"
    fi
  done
  echo "session-start: .arcl/arcl.db did not open; moved it to $broken and rebuilt it" >&2
fi
if [ ! -e .arcl/arcl.db ]; then
  "$arcl" init --quiet
fi
"$arcl" --version
