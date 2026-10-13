#!/usr/bin/env bash
# Validate Conventional Commit subjects against types and scopes in .gitmessage.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
BASE_SHA="${BASE_SHA:-${1:-}}"
HEAD_SHA="${HEAD_SHA:-${2:-}}"
if [ -z "$BASE_SHA" ]; then BASE_SHA=$(git rev-parse origin/main 2>/dev/null || git rev-parse main); fi
if [ -z "$HEAD_SHA" ]; then HEAD_SHA=$(git rev-parse HEAD); fi
TYPES=$(grep '^# Types:' .gitmessage | sed 's/# Types: //' | tr -d ' ' | tr ',' '|')
[ -n "$TYPES" ] || { echo "ERROR: .gitmessage Types missing" >&2; exit 1; }
SCOPES=$(awk '/^# Scopes:$/ { active=1; next } active && /^#   / { sub(/^#   /, ""); print; next } active { exit }' .gitmessage | tr '\n' '|' | sed 's/|$//')
[ -n "$SCOPES" ] || { echo "ERROR: .gitmessage Scopes missing" >&2; exit 1; }
count=0
invalid=0
while IFS= read -r subject; do
  count=$((count + 1))
  if ! echo "$subject" | grep -qE "^($TYPES)(\([^)]+\))?!?: .+"; then
    echo "FAIL (format): $subject"
    invalid=$((invalid + 1))
    continue
  fi
  scope=$(echo "$subject" | sed -nE "s/^($TYPES)\(([^)]+)\)!?: .+/\2/p")
  if [ -n "$scope" ] && ! echo "$scope" | grep -qxE "($SCOPES)"; then
    echo "FAIL (scope '$scope'): $subject"
    invalid=$((invalid + 1))
  else
    echo "OK: $subject"
  fi
done < <(git log --format='%s' "$BASE_SHA..$HEAD_SHA")
[ "$invalid" -eq 0 ] || { echo "$invalid of $count commits failed" >&2; exit 1; }
echo "All $count commits pass Conventional Commits validation."
