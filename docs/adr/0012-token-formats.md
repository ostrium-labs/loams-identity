# ADR 0012: JWT access tokens by default, opaque on request; opaque refresh tokens

- Status: Proposed
- Date: 2026-10-21

## Context

Loams gateways verify access tokens locally with no round trip (Loams design 19 section 5.3).
Other resource servers may prefer opaque tokens and introspection, which gives immediate
revocation and hides claims from the client.

## Decision

- **Access tokens:** JWT per [RFC 9068] (`typ: at+jwt`) by default; per resource server or per
  client, `opaque` tokens (`lia_` strings) that are validated only by introspection.
- **Refresh tokens:** always opaque (`lir_`), stored as SHA-256 hashes, rotated on every use
  (ADR 0014).
- **Codes, device codes, PATs, session cookies:** opaque, prefixed strings (ADR 0013).
- **ID tokens:** JWT per OIDC Core.

## Alternatives considered

- **Opaque by default.** Every Loams request would need introspection; rejected for latency.
- **JWT refresh tokens.** No benefit; the server must look them up anyway to rotate.
- **Encrypted (JWE) access tokens.** Not needed in Phases 1 to 3.

## Consequences

JWT access tokens cannot be revoked before expiry for verifiers that only check signatures; hence
the 15-minute default lifetime, introspection for callers that need immediacy, and identity-sync
events to Loams.

## Derived from

[RFC 9068], [RFC 7662], [RFC 6749] section 1.5, Loams design 19 section 5.3 (Loams-side).

[RFC 9068]: https://www.rfc-editor.org/rfc/rfc9068
[RFC 7662]: https://www.rfc-editor.org/rfc/rfc7662
[RFC 6749]: https://www.rfc-editor.org/rfc/rfc6749
