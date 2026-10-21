# Architecture decision records

Each ADR records one non-obvious decision, the alternatives considered, and what it was derived
from (for `PROVENANCE.md`). Status values: Proposed (Phase A, awaiting the owner's review),
Accepted, Superseded.

All ADRs below are **Proposed** until the Phase A review gate.

| ADR | Decision |
|---|---|
| [0001](0001-clean-room-process.md) | Clean-room process and provenance wall |
| [0002](0002-event-sourcing.md) | Event sourcing with per-aggregate streams, inline indexes and asynchronous projections |
| [0003](0003-storage-backends.md) | redb first, PostgreSQL second, TiKV later |
| [0004](0004-event-serialization.md) | JSON event envelope with versioned payloads and upcasters |
| [0005](0005-jose-library.md) | `jsonwebtoken` 11 for verification and JWK types; our own signer over `KeyStore`; `josekit` rejected |
| [0006](0006-algorithms-and-crypto-backends.md) | Ed25519, ES256 and RS256 keys; RustCrypto, with RSA signing through aws-lc-rs |
| [0007](0007-passkeys-webauthn-rs.md) | Passkeys with `webauthn-rs` 0.5 behind a trait (MPL-2.0, OpenSSL flagged) |
| [0008](0008-totp-rs.md) | TOTP with `totp-rs` 6, explicit-time calls only |
| [0009](0009-argon2id-parameters.md) | argon2id parameters and bounded hashing |
| [0010](0010-protocol-types.md) | Own provider-side protocol types; `openidconnect` and `oauth2` only for federation; `oxide-auth` not used |
| [0011](0011-saml-deferred.md) | SAML deferred; `samael` C dependencies flagged |
| [0012](0012-token-formats.md) | JWT access tokens by default, opaque on request; opaque refresh tokens |
| [0013](0013-identifiers-and-token-strings.md) | UUIDv7 ids from `Clock` and `Rng`; prefixed token strings |
| [0014](0014-refresh-rotation-strict-reuse.md) | Refresh rotation with strict reuse detection and no grace window |
| [0015](0015-issuer-per-instance.md) | One issuer per instance, host-based routing, org as a claim |
| [0016](0016-runtime-crate-and-determinism-lint.md) | A `runtime` crate for real I/O, and a determinism lint |
| [0017](0017-claim-names.md) | Standard claims plus short Loams private claims; no vendor claim names |
| [0018](0018-crypto-shredding.md) | Crypto-shredding of personal data in the event log |
| [0019](0019-login-ui.md) | Server-rendered login UI with askama; CSRF and cookie design |
| [0020](0020-management-api.md) | REST first with OpenAPI, gRPC with our own protos; compatible surface later |
| [0021](0021-cache.md) | foyer directly for the cache; not Loams' `loams-cache` |
| [0022](0022-licence-policy-and-supply-chain.md) | Licence allowlist, exceptions, and the C-dependency register |
| [0023](0023-dst-harness.md) | Own simulation harness on a single-threaded runtime with paused time |
| [0024](0024-rate-limiting-and-lockout.md) | In-process rate limits and store-backed lockout with timed locks |
| [0025](0025-no-session-management-iframe.md) | No OIDC Session Management iframe and no front-channel logout |
| [0026](0026-message-catalogues.md) | Login UI message catalogues in Project Fluent |
| [0027](0027-common-password-blocklist.md) | A bundled common-password blocklist, licence checked before adding |

Template: [template.md](template.md).
