# ADR-0005: Gate GraphQL polling behind conditional REST requests

- **Status:** Proposed
- **Date:** 2026-11-27
- **Deciders:** tcamise

## Context

Every poll ran a GraphQL stack-discovery query and a detail query for the whole stack. For a stack of about ten PRs that costs roughly 25 to 40 points per poll. At the default 15 to 30 second cadence a long-running dashboard spends more than the 5,000-point hourly GraphQL budget, which the account shares with every other tool and agent. On 2026-11-27 an account hit the limit with every `gh api graphql` call failing while REST kept working. The existing backoff only starts after the budget is empty. GraphQL has no conditional requests, but REST does: GitHub answers an unchanged resource with `304 Not Modified`, which is not billed. A probe against `pulls/N`, `pulls/N/reviews`, `pulls/N/comments`, and a commit's `check-runs` and `status` returned ETags and honoured `If-None-Match`, and the rate-limit counter did not move on a 304.

## Decision

Run conditional REST requests in front of the GraphQL queries and query GraphQL only after a change.

- **Gate the provider.** `ChangeGatedProvider` wraps the `gh` adapter and asks a `ChangeProbe` first. An all-304 poll returns the cached stack and details with no GraphQL.
- **Full refresh on any change.** One `200` refreshes the whole stack, because rebase comparisons span layers. A per-PR refresh is a possible later optimization.
- **Safety refresh.** REST cannot see review-thread resolution, so a full refresh also runs every `poll.github_full` seconds (default 600), after any action, on `r`, on a branch switch, and when nothing is cached.
- **Direct HTTP client.** The probe uses `ureq` with the token from `gh auth token`. `gh api` exits 1 on a 304 and prints the headers only with `-i`, so it would need output parsing and a process spawn per request, and the existing failure classifier would misread a 304 as an error.
- **In-memory ETags.** They are per process, keyed by URL, and dropped for a commit when the PR head moves. A restart costs one full refresh.
- **Fail safe.** A probe rate limit surfaces as a rate limit without querying GraphQL; any other probe failure falls back to the full queries.

## Consequences

- An idle dashboard issues no GraphQL queries. While checks change, the cost is one full refresh per real change instead of one per poll.
- A resolved review thread can show as unresolved for up to `github_full` seconds, and merge-queue entry that changes no REST field can be late by the same bound.
- A new dependency with a TLS stack (`ureq`, rustls, ring) enters the release builds, including the cross-compiled Linux arm64 target.
- Only `github.com` is supported by the probe; elsewhere it fails and every poll runs the full queries.
- The first poll after start sends about five REST requests per PR with full bodies, billed at one point each against the separate REST budget.

## Alternatives considered

- **Raise the poll intervals.** Free and needs no code, but it only scales the spend down and delays every update.
- **Webhooks.** Push-based and cheapest, but creating one on an org repository needs admin rights.
- **A GitHub App token.** A separate, larger budget, but it needs app registration and distribution of credentials.
- **`gh api` with `-i` and `If-None-Match`.** Avoids a dependency, but needs per-request process spawns and special-casing the exit code.
- **Per-PR GraphQL refresh.** Cheaper per change, but needs merging partial details and recomputing rebase comparisons.
