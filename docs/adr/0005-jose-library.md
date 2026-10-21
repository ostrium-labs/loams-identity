# ADR 0005: `jsonwebtoken` 11 for verification and JWK types; our own signer over `KeyStore`

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

We sign ID tokens, access tokens and logout tokens, and we verify inbound JWTs: client
assertions (`private_key_jwt`), JWT bearer grants, upstream ID tokens, workload-identity tokens,
and our own tokens at introspection. Signing must go through the `KeyStore` trait because keys
may live in a KMS. Verification must use strict allowlists. Time checks must use the injected
`Clock` (ARCH-7).

## Candidates (checked on crates.io, 2026-10-21)

| Crate | Version, last release | Licence | Algorithms | Notes |
|---|---|---|---|---|
| `jsonwebtoken` | 11.1.0, 2026-09-16 | MIT | HS256/384/512, RS256/384/512, PS256/384/512, ES256, ES384, EdDSA | Selectable crypto backend (`rust_crypto` or `aws_lc_rs` features); `JwtSigner` and `JwtVerifier` traits; `jwk` module with `Jwk`, `JwkSet` and RFC 7638 thumbprints; no `none` variant in `Algorithm`; no JWE. About 61 million recent downloads |
| `josekit` | 0.10.3, 2025-05-20 | MIT OR Apache-2.0 | Broad JWS and JWE | **Hard dependency on `openssl`** (a C library); last release 17 months ago |
| `jwt-simple` | 0.15.0, 2026-09-25 | ISC | Broad, plus JWE and CWT | Pure Rust by default (through `superboring`); its own key types, so a KMS signer is harder to plug in |
| `biscuit` | 0.8.0, 2026-04-05 | MIT | JWS and JWE | Depends on `ring`; small community |

## Decision

- Use **`jsonwebtoken` 11** with the `rust_crypto` backend for **verification** and for JWK and
  JWKS types.
- **Sign** through our own implementation of its `JwtSigner` trait (or our own compact-JWS
  encoder if the trait does not fit a KMS round trip), which calls `KeyStore::sign`. JWS compact
  framing is not a cryptographic primitive; the signature itself comes from the crypto crates or
  the KMS (ADR 0006).
- Turn off `jsonwebtoken`'s built-in `exp` and `nbf` validation and check time claims ourselves
  against `Clock` ([06](../../spec/06-keys-and-jose.md)). Its `get_current_timestamp` is listed in
  the disallowed methods.
- Before calling the library, our code parses the header and enforces the rules the library
  does not: `typ`, refusal of `jwk`/`jku`/`x5u`/`x5c`, `crit`, and `kid` lookup only in the
  expected key set (SEC-14, SEC-15).
- Wrap all of this in a `jose` module in `core` with its own types, so the library can be swapped.

## Alternatives considered

- **`josekit`**: rejected because of the OpenSSL C dependency and the slower release cadence.
- **`jwt-simple`**: good, pure Rust, but its key model fits a KMS signer less well. Second choice.
- **Hand-written JOSE over RustCrypto**: more control, more risk. We hand-write only the header
  checks and the signing framing.
- JWE is not needed in Phases 1 to 3 (no encrypted ID tokens). If needed later, revisit.

## Consequences

- *Interoperability cost* `[P3]`: client assertions must carry the issuer identifier alone as
  `aud` ([06](../../spec/06-keys-and-jose.md) KEY-19, following `draft-ietf-oauth-rfc7523bis`).
  Older client libraries and some test suites send the token endpoint URL, which OIDC Core
  section 9 allowed, and will be refused. This is the conservative choice against audience
  injection; the owner may relax it per client later through an ADR.
- `jsonwebtoken` has no `Ed25519` identifier from RFC 9864, only `EdDSA`; we emit `EdDSA`.
- `jsonwebtoken` 11 depends on `ed25519-dalek` 2 and `p256` 0.13, while the newest releases are
  `ed25519-dalek` 3 and `p256` 0.14. Expect duplicate versions in the tree until it updates;
  `cargo-deny` bans duplicates with an exception list.

## Derived from

Crate documentation on docs.rs (`jsonwebtoken` 11.1.0 item list and `Algorithm` enum) and
crates.io metadata, checked 2026-10-21. [RFC 7515], [RFC 7517], [RFC 7519], [RFC 8725],
[RFC 9864].

[RFC 7515]: https://www.rfc-editor.org/rfc/rfc7515
[RFC 7517]: https://www.rfc-editor.org/rfc/rfc7517
[RFC 7519]: https://www.rfc-editor.org/rfc/rfc7519
[RFC 8725]: https://www.rfc-editor.org/rfc/rfc8725
[RFC 9864]: https://www.rfc-editor.org/rfc/rfc9864
