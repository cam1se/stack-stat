#!/usr/bin/env bash
# Reset or perturb the live GitHub fixture used by the end-to-end suite (issue #23).
#
# Usage: scripts/live-fixture.sh <reset|conflict|status>
#   reset     Force-push every fixture branch back to its fixture-start/* tag and
#             restore draft state. Requires push access to the fixture repository.
#   conflict  Move fixture/trunk to fixture-conflict/trunk so `gh stack sync` on
#             stack s2 hits a rebase conflict. Undo with `reset`.
#   status    Print each fixture branch and whether it matches its start tag.
#
# Environment: STACK_STAT_FIXTURE_REPO (default cam1se/stack-stat).
set -euo pipefail

repo="${STACK_STAT_FIXTURE_REPO:-cam1se/stack-stat}"
url="https://github.com/${repo}.git"
branches=(s1-a s1-b s1-c s2-a s2-b trunk)
draft_prs=(27 28 29 31 32)

remote_sha() { git ls-remote "$url" "$1" | cut -f1; }

cmd="${1:-status}"
case "$cmd" in
  status)
    for b in "${branches[@]}"; do
      head="$(remote_sha "refs/heads/fixture/$b")"
      tag="$(remote_sha "refs/tags/fixture-start/$b^{}")"
      [ -n "$tag" ] || tag="$(remote_sha "refs/tags/fixture-start/$b")"
      if [ "$head" = "$tag" ]; then state=clean; else state=DRIFTED; fi
      printf '%-14s %s\n' "fixture/$b" "$state"
    done
    ;;
  reset)
    for b in "${branches[@]}"; do
      tag="$(remote_sha "refs/tags/fixture-start/$b")"
      [ -n "$tag" ] || { echo "missing tag fixture-start/$b" >&2; exit 1; }
      tmp="$(mktemp -d)"
      git -C "$tmp" init -q
      git -C "$tmp" fetch -q --depth 1 "$url" "refs/tags/fixture-start/$b:refs/tags/t"
      git -C "$tmp" push -q --force "$url" "refs/tags/t:refs/heads/fixture/$b"
      rm -rf "$tmp"
    done
    for pr in "${draft_prs[@]}"; do
      is_draft="$(gh pr view "$pr" --repo "$repo" --json isDraft --jq .isDraft)"
      [ "$is_draft" = true ] || gh pr ready "$pr" --repo "$repo" --undo >/dev/null
    done
    echo "fixture reset"
    ;;
  conflict)
    tmp="$(mktemp -d)"
    git -C "$tmp" init -q
    git -C "$tmp" fetch -q --depth 1 "$url" "refs/tags/fixture-conflict/trunk:refs/tags/t"
    git -C "$tmp" push -q --force "$url" "refs/tags/t:refs/heads/fixture/trunk"
    rm -rf "$tmp"
    echo "fixture/trunk now conflicts with stack s2"
    ;;
  *)
    echo "usage: $0 <reset|conflict|status>" >&2
    exit 2
    ;;
esac
