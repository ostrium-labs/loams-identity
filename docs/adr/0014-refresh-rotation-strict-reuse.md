# ADR 0014: Refresh rotation with strict reuse detection and no grace window

- Status: Proposed
- Date: 2026-10-21

## Context

[RFC 9700] section 4.14.2 requires refresh tokens for public clients to be sender-constrained or
rotated, and describes reuse detection: if an old refresh token is used again, the server cannot
tell attacker from client and should revoke the active token. Some providers allow a short grace
window during which the previous token still works, to tolerate a client that lost the response
of a rotation (network failure, two tabs racing).

## Decision

- Rotate on every use, for every client type.
- **No grace window.** Presenting any retired refresh token revokes the family (DM-18).
- Parallel use of the current token: exactly one request wins the compare-and-append; the loser
  sees a retired token and the family is revoked. Clients must serialize refreshes.

## Alternatives considered

- **Grace window (for example 30 seconds, previous token still accepted once).** Friendlier to
  flaky clients, but it gives an attacker who stole a token the same window, and it complicates
  the TLA+ invariant. The owner's rule is to pick the conservative option. The window can be added
  later as a per-client option through a new ADR if real clients need it.
- **Rotation only for public clients.** Confidential clients' tokens are bound by client
  authentication ([RFC 6749] section 10.4), but rotation for all keeps one code path and one model.

## Consequences

A client that loses a rotation response loses its session and must sign in again. Browser SPAs
with several tabs must coordinate refreshes (for example with a lock in the Web Locks API).

## Derived from

[RFC 9700] section 4.14.2, [RFC 6749] section 10.4, OAuth 2.1 draft (`draft-ietf-oauth-v2-1-15`)
section on refresh tokens.

[RFC 9700]: https://www.rfc-editor.org/rfc/rfc9700
[RFC 6749]: https://www.rfc-editor.org/rfc/rfc6749
