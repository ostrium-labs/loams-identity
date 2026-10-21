# ADR 0006: Ed25519, ES256 and RS256 keys; RustCrypto, with RSA signing through aws-lc-rs

- Status: Proposed
- Date: 2026-10-21

## Context

- The owner asks for EdDSA or ES256 as the default, with RS256 for compatibility.
- OpenID Connect Discovery requires `RS256` in `id_token_signing_alg_values_supported`, and
  OpenID Connect Dynamic Client Registration makes `RS256` the default ID-token algorithm when a
  client does not register one.
- Loams' own tokens use Ed25519 (Loams design 19 section 5.3), and Loams gateways verify access
  tokens locally.
- Many RP libraries support ES256 and RS256 but not EdDSA.
- The RustCrypto `rsa` crate has an unpatched timing side channel in private-key operations
  (RUSTSEC-2023-0071, "Marvin attack"; no patched version as of 2026-09-12, latest 0.9.10).
- Deterministic simulation prefers deterministic signatures: Ed25519 and RSA PKCS#1 v1.5 are
  deterministic, and ECDSA can be made deterministic with RFC 6979 nonces.

## Decision

| Use | Default | Allowed per client |
|---|---|---|
| JWT access tokens (RFC 9068) | EdDSA (Ed25519) | ES256, RS256 (per resource server) |
| ID tokens, admin-created clients | ES256 | EdDSA, RS256 |
| ID tokens, dynamically registered clients that omit the algorithm `[P6]` | RS256 (required by OIDC Registration) | EdDSA, ES256 |
| Logout tokens `[P6]` | Same as the client's ID-token algorithm | |

- Every instance keeps one active key for each of Ed25519, P-256 and RSA. New RSA keys are
  3072-bit ([06](../../spec/06-keys-and-jose.md)); inbound RSA keys of at least 2048 bits are
  accepted. JWKS publishes all non-retired keys.
- **Backends:** `ed25519-dalek` (BSD-3-Clause) for Ed25519; `p256` (Apache-2.0 OR MIT) with
  RFC 6979 deterministic nonces for ES256; **aws-lc-rs** (ISC AND Apache-2.0 OR ISC, contains C
  and assembly) for RSA **signing**. RSA **verification** may use `rsa` (public-key operations are
  not what the advisory concerns) or aws-lc-rs; pick one in Phase 1 and test both paths against
  each other.
- HMAC algorithms are never used for tokens (SEC-13).

## Alternatives considered

- **RSA signing with the `rsa` crate.** Rejected while RUSTSEC-2023-0071 is open: a networked
  OP signs on request, which is the exposure the advisory warns about.
- **`ring` for RSA.** Also C and assembly; aws-lc-rs is the default provider of rustls too, so it
  adds no new C dependency to the tree, and it has a FIPS build.
- **aws-lc-rs for everything.** Fewer crates, but ECDSA there uses random nonces, which hurts
  deterministic simulation, and more of the core would rest on C.
- **EdDSA as the ID-token default.** Too many RP libraries lack it.
- **Emit `Ed25519` (RFC 9864 fully-specified) instead of `EdDSA`.** Not yet supported by most
  verifiers or by `jsonwebtoken`. Revisit when support spreads.

## Consequences

- aws-lc-rs needs a C compiler and CMake to build; it enters the C-dependency register (ADR 0022).
- The simulator's software `KeyStore` uses the same algorithms, so signatures are byte-stable per
  seed.

## Derived from

[OIDC Discovery] section 3, [OIDC Registration] section 2, [RFC 8037], [RFC 6979], [RFC 9864],
[RFC 7518], RustSec [RUSTSEC-2023-0071], crates.io metadata checked 2026-10-21, Loams design 19
section 5.3 (Loams-side contract).

[OIDC Discovery]: https://openid.net/specs/openid-connect-discovery-1_0.html
[OIDC Registration]: https://openid.net/specs/openid-connect-registration-1_0.html
[RFC 8037]: https://www.rfc-editor.org/rfc/rfc8037
[RFC 6979]: https://www.rfc-editor.org/rfc/rfc6979
[RFC 9864]: https://www.rfc-editor.org/rfc/rfc9864
[RFC 7518]: https://www.rfc-editor.org/rfc/rfc7518
[RUSTSEC-2023-0071]: https://rustsec.org/advisories/RUSTSEC-2023-0071.html
