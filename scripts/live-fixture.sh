#!/usr/bin/env bash
# Manage the live GitHub fixture used by the end-to-end suite (issue #23).
#
# Fixture PRs stay open permanently so the suite and local agent-led TUI tests
# can use them at any time; closing and reopening them only generates
# notification emails.
#
# Usage: scripts/live-fixture.sh <command>
#   up        Reset branches and reopen any closed fixture PRs (recovery only).
#   reset     Force-push every fixture branch to its fixture-start/* tag and
#             restore draft state. Requires the PRs to be open.
#   conflict  Move fixture/trunk to fixture-conflict/trunk so `gh stack sync`
#             on stack s2 hits a rebase conflict. Undo with `reset`.
#   check     Verify the fixture: tags, branches, PR states and labels.
#   status    Print each fixture branch and whether it matches its start tag.
#   rebuild   Recreate missing branches from their tags and missing PRs for
#             stacks s1/s2, s4 and s5. Merged and fork PRs cannot be rebuilt
#             here; see docs/architecture/07-live-validation.md.
#
# Environment: STACK_STAT_FIXTURE_REPO (default cam1se/stack-stat).
set -euo pipefail

repo="${STACK_STAT_FIXTURE_REPO:-cam1se/stack-stat}"
url="https://github.com/${repo}.git"
labels=("fixture" "do not merge")

# Branches restored to fixture-start/<name> by `reset`.
branches=(s1-a s1-b s1-c s2-a s2-b s5-single s6-nopr trunk)
# PRs that stay open; `reset` restores the drafts among them.
open_prs=(27 28 29 31 32 37 40 43)
# Of those, PRs that must be drafts while open.
draft_prs=(27 28 29 31 32 43)
# PRs that are permanently merged or closed; tests only read them.
merged_prs=(34 35 45)
closed_prs=(38 44)

remote_sha() { git ls-remote "$url" "$1" | cut -f1; }

pr_state() { gh pr view "$1" --repo "$repo" --json state --jq .state; }

reset_branches() {
  for b in "${branches[@]}"; do
    tag="$(remote_sha "refs/tags/fixture-start/$b")"
    [ -n "$tag" ] || { echo "missing tag fixture-start/$b" >&2; exit 1; }
    tmp="$(mktemp -d)"
    git -C "$tmp" init -q
    git -C "$tmp" fetch -q --depth 1 "$url" "refs/tags/fixture-start/$b:refs/tags/t"
    git -C "$tmp" push -q --force "$url" "refs/tags/t:refs/heads/fixture/$b"
    rm -rf "$tmp"
  done
}

reset_drafts() {
  for pr in "${draft_prs[@]}"; do
    is_draft="$(gh pr view "$pr" --repo "$repo" --json isDraft --jq .isDraft)"
    [ "$is_draft" = true ] || gh pr ready "$pr" --repo "$repo" --undo >/dev/null
  done
}

reopen_prs() {
  for pr in "${open_prs[@]}"; do
    [ "$(pr_state "$pr")" = OPEN ] || gh pr reopen "$pr" --repo "$repo" >/dev/null
  done
}

check() {
  failures=0
  fail() { echo "FIXTURE: $*" >&2; failures=$((failures + 1)); }
  for b in "${branches[@]}"; do
    [ -n "$(remote_sha "refs/tags/fixture-start/$b")" ] || fail "missing tag fixture-start/$b"
    [ -n "$(remote_sha "refs/heads/fixture/$b")" ] || fail "missing branch fixture/$b"
  done
  for pr in "${open_prs[@]}"; do
    want=OPEN
    got="$(pr_state "$pr")" || { fail "PR #$pr not found"; continue; }
    [ "$got" = "$want" ] || fail "PR #$pr is $got, expected $want"
  done
  for pr in "${merged_prs[@]}"; do
    [ "$(pr_state "$pr")" = MERGED ] || fail "PR #$pr is not MERGED (it cannot be recreated by script)"
  done
  for pr in "${closed_prs[@]}"; do
    [ "$(pr_state "$pr")" = CLOSED ] || fail "PR #$pr is not CLOSED"
  done
  for pr in "${open_prs[@]}" "${merged_prs[@]}" "${closed_prs[@]}"; do
    have="$(gh pr view "$pr" --repo "$repo" --json labels --jq '.labels[].name')"
    for label in "${labels[@]}"; do
      printf '%s\n' "$have" | grep -qx "$label" || fail "PR #$pr lacks the '$label' label"
    done
  done
  if [ "$failures" -ne 0 ]; then
    echo "fixture check failed with $failures problem(s); see docs/architecture/07-live-validation.md" >&2
    exit 1
  fi
  echo "fixture ok"
}

rebuild() {
  tmp="$(mktemp -d)"
  gh repo clone "$repo" "$tmp/w" -- -q
  cd "$tmp/w"
  for b in "${branches[@]}"; do
    if [ -z "$(remote_sha "refs/heads/fixture/$b")" ]; then
      echo "recreating branch fixture/$b from its tag"
      git fetch -q origin "refs/tags/fixture-start/$b:refs/tags/t-$b"
      git push -q origin "refs/tags/t-$b:refs/heads/fixture/$b"
    fi
  done
  git fetch -q origin
  has_pr() { [ -n "$(gh pr list --repo "$repo" --head "$1" --state all --json number --jq '.[0].number // empty')" ]; }
  ensure_stack() {
    if has_pr "fixture/$2"; then return; fi
    echo "creating stack $1"
    base="${3:-main}"
    git checkout -q -f "fixture/$2"
    shift 3
    gh stack init --base "$base" "${@/#/fixture/}"
    gh stack submit --auto
  }
  ensure_stack s1 s1-a main s1-a s1-b s1-c
  ensure_stack s2 s2-a fixture/trunk s2-a s2-b
  if ! has_pr fixture/s5-single; then
    echo "creating PR for fixture/s5-single"
    gh pr create --repo "$repo" --draft --base fixture/single-base --head fixture/s5-single \
      --title "fixture: s5-single (single PR, non-default base)" --body "Live fixture for stack-stat e2e tests (#23). Do not merge." >/dev/null
  fi
  gh label create "fixture" --repo "$repo" --color BFD4F2 \
    --description "Live e2e test fixture" 2>/dev/null || true
  gh label create "do not merge" --repo "$repo" --color B60205 \
    --description "Never merge this PR" 2>/dev/null || true
  for pr in "${open_prs[@]}" "${merged_prs[@]}" "${closed_prs[@]}"; do
    gh pr edit "$pr" --repo "$repo" --add-label "fixture" --add-label "do not merge" >/dev/null \
      || echo "warning: could not label PR #$pr" >&2
  done
  cd /
  rm -rf "$tmp"
  echo "rebuild done; run '$0 check' to verify"
}

cmd="${1:-status}"
case "$cmd" in
  status)
    for b in "${branches[@]}"; do
      head="$(remote_sha "refs/heads/fixture/$b")"
      tag="$(remote_sha "refs/tags/fixture-start/$b")"
      if [ "$head" = "$tag" ]; then state=clean; else state=DRIFTED; fi
      printf '%-18s %s\n' "fixture/$b" "$state"
    done
    ;;
  reset)
    reset_branches
    reset_drafts
    echo "fixture reset"
    ;;
  up)
    reopen_prs
    reset_branches
    reset_drafts
    echo "fixture up"
    ;;
  conflict)
    tmp="$(mktemp -d)"
    git -C "$tmp" init -q
    git -C "$tmp" fetch -q --depth 1 "$url" "refs/tags/fixture-conflict/trunk:refs/tags/t"
    git -C "$tmp" push -q --force "$url" "refs/tags/t:refs/heads/fixture/trunk"
    rm -rf "$tmp"
    echo "fixture/trunk now conflicts with stack s2"
    ;;
  check)
    check
    ;;
  rebuild)
    rebuild
    ;;
  *)
    echo "usage: $0 <up|reset|conflict|check|status|rebuild>" >&2
    exit 2
    ;;
esac
