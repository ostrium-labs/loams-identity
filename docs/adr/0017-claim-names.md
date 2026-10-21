# ADR 0017: Standard claims plus short Loams private claims; no vendor claim names

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

Tokens must carry the org, the kind of principal, and for agents the owner and the agent session,
so Loams can build its `Subject` (Loams design 76 section 9.1) and its audit records. The clean
room (ADR 0001) rules out reusing Zitadel's claim names.

## Decision

- Use registered claims wherever one exists: `iss`, `sub`, `aud`, `exp`, `iat`, `nbf`, `jti`,
  `client_id`, `scope` ([RFC 9068], [RFC 8693]), `act`, `may_act` ([RFC 8693]), `auth_time`,
  `acr`, `amr` ([RFC 8176]), `sid`, `azp`, `nonce` (OIDC).
- Loams private claims, short names, defined in [15](../../spec/15-loams-integration.md):
  `org`, `principal`, `agent_owner`, `agent_session`, and `roles` `[P4]`.
- Private claims are "private claim names" in the sense of [RFC 7519] section 4.3: agreed between
  this issuer and Loams. They are documented as the Loams contract and versioned with it.
- `scope` (space-separated string, RFC 9068), not `scp`.

## Alternatives considered

- **Collision-resistant names** (`https://loams.dev/claims/org`). Safer against collisions with
  third-party claims, but verbose in every token and every Loams code path. The issuer controls
  all claims it emits, so collisions can only come from our own choices.
- **`scp`** to match Loams' own vended tokens (design 19 section 5.3). RFC 9068 names `scope`;
  Loams' verifier accepts both.

## Consequences

Loams must read `scope` from loams-identity tokens. Listed in the integration spec as a Loams-side
change.

## Derived from

[RFC 7519] section 4.3, [RFC 9068], [RFC 8693], [RFC 8176], OIDC Core section 2, Loams designs 19
section 5.3 and 76 section 9.1 (Loams-side contracts).

[RFC 7519]: https://www.rfc-editor.org/rfc/rfc7519
[RFC 9068]: https://www.rfc-editor.org/rfc/rfc9068
[RFC 8693]: https://www.rfc-editor.org/rfc/rfc8693
[RFC 8176]: https://www.rfc-editor.org/rfc/rfc8176
