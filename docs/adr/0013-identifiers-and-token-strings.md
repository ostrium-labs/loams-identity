# ADR 0013: UUIDv7 ids from `Clock` and `Rng`; prefixed token strings

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

Ids must be unique, sortable by creation time (useful for logs and pagination), storable as
native `uuid` in PostgreSQL, and generated deterministically under simulation. Token strings
should be recognizable by secret scanners and by humans reading an incident report.

## Decision

- **Ids:** UUIDv7 ([RFC 9562]), built with `uuid`'s builder from `Clock` milliseconds and 10
  bytes from `Rng`, never with `Uuid::now_v7` (ARCH-10). Rendered lower-case hyphenated.
- **Token strings:** a prefix plus base64url (no padding) of 32 bytes from `Rng`:

| Prefix | Token |
|---|---|
| `lic_` | Authorization code |
| `lir_` | Refresh token |
| `lia_` | Opaque access token |
| `lip_` | Personal access token |
| `lid_` | Device code |
| `lis_` | Session cookie value |
| `lrs_` | Password reset and invitation token |
| `lve_` | Email verification token |
| `lics_` | Machine-user client secret `[P3]` |
| `lirg_` | Registration access token `[P6]` |
| `whsec_` | Webhook signing secret `[P4]` (the Standard Webhooks convention) |

- Cookie names: `__Host-loams_session` (with `_<slug>` appended in the path issuer form, so two
  instances on one host do not share a cookie), `__Host-loams_ar` (authorization-request binding),
  `__Host-loams_csrf`, `__Host-loams_idp` (upstream login binding, `[P5]`).
- Lookups use the SHA-256 of the whole string (DM-4, ES-13).
- The prefixes are registered with GitHub secret scanning once the project is public, so leaked
  tokens are reported.

## Alternatives considered

- **ULID.** Equivalent ordering, but PostgreSQL has a native `uuid` type and Loams uses UUIDs.
- **UUIDv4.** Not time-ordered.
- **Tokens with an embedded id and a secret part** (`prefix_<id>_<secret>`). Allows a cheap lookup
  by id, but a hash lookup is equally cheap, and a single random value is simpler.

## Consequences

UUIDv7 leaks creation time to anyone who sees an id. Acceptable for identity objects; `sub`
values reveal when an account was created.

## Derived from

[RFC 9562]; `uuid` crate documentation; GitHub's public documentation on secret scanning partner
patterns (general practice).

[RFC 9562]: https://www.rfc-editor.org/rfc/rfc9562
