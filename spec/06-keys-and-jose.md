# 06. Keys and JOSE

This file covers the instance's signing keys, the JWKS document, how we sign JWTs and how we
validate every JWT we receive. It follows the JOSE standards ([RFC 7515], [RFC 7517],
[RFC 7518], [RFC 7519], [RFC 8037]) and the JWT best current practice ([RFC 8725]).

## 1. Algorithms

**KEY-1.** Every active instance holds, at all times, exactly one `active` signing key for each of
these algorithms, plus one `pending` successor for each:

| `alg` | Key | Library path (ADR 0006) | Used for by default |
|---|---|---|---|
| `EdDSA` | Ed25519 (`kty: OKP`, `crv: Ed25519`) | `ed25519-dalek` | JWT access tokens |
| `ES256` | P-256 (`kty: EC`, `crv: P-256`) | `p256`, deterministic nonces per [RFC 6979] | ID tokens for clients created by an admin |
| `RS256` | RSA 3072-bit (`kty: RSA`) | `aws-lc-rs` | ID tokens for clients that ask for it, and for dynamically registered clients that omit `id_token_signed_response_alg` ([OIDC Registration] section 2) |

- *Conservative choice:* new RSA keys are 3072 bits. We accept 2048-bit RSA keys from clients and
  upstream providers ([RFC 7518] section 3.3 sets 2048 as the minimum), never smaller.
- RSA signing uses `aws-lc-rs`, not the `rsa` crate, because the `rsa` crate has an unfixed timing
  side channel on private-key operations ([RUSTSEC-2023-0071]). The `rsa` crate may be used for
  RSA signature *verification* only (public-key operations).
- All three signature schemes are deterministic for a given key and message (Ed25519 by
  construction, ECDSA with RFC 6979 nonces, RSASSA-PKCS1-v1_5 by construction). The simulation
  relies on this: the same seed gives byte-identical tokens.

**KEY-2.** We never issue or accept a JWT with `alg: none` ([RFC 8725] section 3.1). We never
sign ID tokens or access tokens with an HMAC algorithm (`HS256`, `HS384`, `HS512`), and we do not
support `client_secret_jwt` client authentication, so no HMAC JWT is ever accepted either.

**KEY-3.** We emit the algorithm name `EdDSA` for Ed25519 signatures, because that is what
relying-party libraries understand today. [RFC 9864] defines the fully specified name `Ed25519`;
accepting and emitting it is future work, tracked in ADR 0006. Until then an inbound JWT with
`alg: Ed25519` is rejected as an unknown algorithm.

## 2. Key lifecycle

```text
 created ──► pending ──(≥ 24h published, at rotation)──► active ──(next rotation)──► retiring ──(retire delay)──► retired
                                                            │
                                               compromise ──┴──► retired at once (emergency, KEY-9)
```

| State | In JWKS | Signs new tokens | Verifies our tokens |
|---|---|---|---|
| `pending` | Yes | No | Yes (nothing is signed with it yet) |
| `active` | Yes | Yes | Yes |
| `retiring` | Yes | No | Yes |
| `retired` | No | No | Only `id_token_hint` at logout (KEY-8) |

**KEY-4.** The key id (`kid`) is 16 random bytes from `Rng`, base64url without padding. It is never
derived from the key material (for example, it is not the JWK thumbprint) and never reused,
across algorithms and across instances.

**KEY-5.** Rotation schedule (instance settings, these are the defaults):

| Setting | Default | Rule |
|---|---|---|
| `key_rotation_interval` | 90d | Each algorithm rotates independently |
| `key_min_pending` | 24h | A pending key must have been in the JWKS at least this long before it may become active |
| `key_retire_delay` | 24h | Must be at least the longest access-token or ID-token lifetime allowed (1h) plus the JWKS `max-age` (5m) plus the inbound clock-skew allowance (60s); the process refuses a configuration that violates this |
| `key_destroy_delay` | 30d | Private key material of a retired key is destroyed in the `KeyStore` this long after retirement |

**KEY-6.** Rotation of one algorithm is one atomic append to the instance's key streams: the
pending key becomes `active`, the old active key becomes `retiring`, and a new pending key is
created (`signing_key.created`) in the same batch. So there is always a published successor.
The rotation job is driven by the scheduler and decides by comparing `Clock` time with the
`activated_at` timestamps (ARCH-11).

**KEY-7.** The signer picks the key by `(instance, alg)` and uses only the `active` key. A token
records its `kid` in the header. If the active key for an algorithm is missing (a bug), signing
fails with `server_error`; it never falls back to another algorithm or another instance's key.

**KEY-8.** Verification of our own tokens (introspection of JWT access tokens, userinfo,
`id_token_hint`) uses the instance's keys in states `pending`, `active` and `retiring`. For
`id_token_hint` at logout only, `retired` keys whose private part has not been destroyed are also
accepted, because a hint can be old. No other use accepts `retired` keys.

**KEY-9.** Emergency rotation (an admin or operator command, with a reason) marks the compromised
key `retired` at once, activates the pending successor without waiting for `key_min_pending`, and
creates a new pending key. Tokens signed with the compromised key stop verifying immediately.
Relying parties that cached the JWKS may fail to verify new tokens until their cache refreshes;
this is accepted and documented in the operator guide.

## 3. The `KeyStore`

The trait from ARCH section 3. Private keys never leave it.

| Operation | Input | Output |
|---|---|---|
| `generate` | instance id, `alg` | key handle, public JWK |
| `sign` | key handle, `alg`, message bytes | signature bytes |
| `public_jwk` | key handle | public JWK |
| `destroy` | key handle | nothing; later `sign` calls fail with `KeyDestroyed` |
| `new_data_key` | instance id, purpose | data-key handle, wrapped key bytes (DM-6) |
| `unwrap_data_key` | wrapped bytes | an in-memory key wrapped in `secrecy` |
| `destroy_data_key` | data-key handle | nothing (crypto-shredding, ADR 0018) |
| `hmac` | key handle, message | 32-byte HMAC-SHA-256 (login-name and email index keys, DM-6) |

- The software implementation (`runtime`) seals each private key with AES-256-GCM under a root
  key read from a file named by `LOAMS_IDENTITY_ROOT_KEY_FILE` (32 random bytes, mode `0600`). The
  sealed blobs live in the store; the root key never does.
- **KEY-10.** `sign` checks that the handle's algorithm equals the requested `alg` and fails
  otherwise. A key generated for `ES256` can never produce an `RS256` signature.
- A KMS-backed implementation (cloud KMS or an HSM) is later work; the trait does not change.

## 4. JWKS document

Served at `/oauth/jwks` (named in discovery as `jwks_uri`).

```json
{
  "keys": [
    { "kty": "OKP", "crv": "Ed25519", "x": "...", "kid": "...", "use": "sig", "alg": "EdDSA" },
    { "kty": "EC", "crv": "P-256", "x": "...", "y": "...", "kid": "...", "use": "sig", "alg": "ES256" },
    { "kty": "RSA", "n": "...", "e": "AQAB", "kid": "...", "use": "sig", "alg": "RS256" }
  ]
}
```

**KEY-11.** The document contains only public members. Each key has `kid`, `use: "sig"` and
`alg`. Keys are ordered by `alg` (alphabetical), then by state (`active`, `pending`, `retiring`),
then by `created_at` (ARCH-9).

**KEY-12.** Response headers: `Content-Type: application/json` (more widely accepted by relying
parties than `application/jwk-set+json`), `Cache-Control: public, max-age=300`, a strong `ETag`
over the body, and `Access-Control-Allow-Origin: *` (the document is public). A request with a
matching `If-None-Match` gets `304`.

**KEY-13.** The JWKS is served from the `jwks_document` projection, which key events also
invalidate directly. *Conservative choice:* a newly created pending key must appear in the served
document before `key_min_pending` starts counting; the rotation job checks the projection's
checkpoint, not just the event.

## 5. Signing JWTs we issue

**KEY-14.** Every JWT we issue has a protected header with exactly `alg`, `kid` and `typ`:

| Token | `typ` |
|---|---|
| ID token | `JWT` (OIDC defines no type; `JWT` is what relying parties accept) |
| JWT access token | `at+jwt` ([RFC 9068] section 2.1) |
| Logout token `[P6]` | `logout+jwt` ([OIDC Back-Channel Logout] section 2.4) |

**KEY-15.** The payload is serialized with sorted keys and no insignificant whitespace (ARCH-9).
Base64url encoding is without padding ([RFC 7515] section 2).

**KEY-16.** Signing goes through our implementation of `jsonwebtoken`'s `JwtSigner` trait, which
calls `KeyStore::sign` (ADR 0005). The JWS signing input and signature encoding follow
[RFC 7515] section 5.1 and [RFC 7518] section 3 (for ES256 the signature is the 64-byte `R || S`
form, not DER).

## 6. Validating JWTs we receive

Inbound JWTs are: client assertions for `private_key_jwt` `[P3]`, JWT bearer grants `[P3]`, subject
and actor tokens in token exchange `[P3]`, upstream ID tokens `[P5]`, `id_token_hint`, and our own
access tokens at introspection and userinfo. Each **use** has a fixed profile:

| Use | Expected `typ` | Allowed `alg` | Key set | `iss` must be |
|---|---|---|---|---|
| Our access token | `at+jwt` | `EdDSA`, `ES256`, `RS256` | Instance keys per KEY-8 | The instance issuer |
| `id_token_hint` | `JWT` or absent | `EdDSA`, `ES256`, `RS256` | Instance keys per KEY-8 | The instance issuer |
| Client assertion `[P3]` | absent, `JWT` or `client-authentication+jwt` | `EdDSA`, `ES256`, `ES384`, `RS256`, `PS256` | That client's JWKS | The client id |
| JWT bearer grant `[P3]` | absent or `JWT` | as client assertion | That machine user's registered machine keys only ([13](13-machine-identity.md)) | Per [13](13-machine-identity.md) |
| Workload-identity subject token in token exchange `[P3]` | absent or `JWT` | The issuer's advertised set intersected with `EdDSA`, `ES256`, `ES384`, `RS256`, `PS256` | The JWKS of the issuer named in the matching trust policy ([13](13-machine-identity.md)) | That trust policy's issuer |
| Upstream ID token `[P5]` | absent or `JWT` | The provider's advertised set intersected with `EdDSA`, `ES256`, `ES384`, `RS256`, `PS256` | The provider's JWKS | The provider's issuer |

**KEY-17.** The validation steps, in this order. A failure at any step rejects the JWT; the error
recorded internally names the step, the error returned to the caller does not.

1. **Size.** The compact string is at most 16 KiB. It has exactly three dot-separated parts.
   Five parts (JWE) are rejected: we accept no encrypted JWTs in these phases.
2. **Header decoding.** Base64url without padding, strict alphabet, no whitespace. The header is
   a JSON object with no duplicate member names. *Conservative choice:* duplicates are rejected
   rather than resolved to the last value ([RFC 7515] section 5.2 permits either).
3. **`alg`.** Present, a string, in the use's allowed list. Never `none` (KEY-2).
4. **Forbidden header members.** `jku`, `x5u`, `jwk` and `x5c` are rejected: we never take a key
   from the token itself ([RFC 8725] section 3.10). `crit` is rejected (we implement no critical
   extensions), and so is `b64` ([RFC 7797]).
5. **`typ`.** Matches the use's expected values, compared case-insensitively after removing an
   optional `application/` prefix ([RFC 7515] section 4.1.9). This stops one kind of JWT being
   used as another ([RFC 8725] section 3.11): our access token is never accepted as a client
   assertion, and an ID token is never accepted as an access token.
6. **Key selection.** If `kid` is present, look it up only in the use's key set. If `kid` is
   absent: for our own tokens, reject; for a client or provider key set, accept only if exactly
   one key in the set matches the `alg`. The key's `kty` and `crv` must match the `alg` (OKP and
   Ed25519 for `EdDSA`; EC and P-256 for `ES256`; EC and P-384 for `ES384`; RSA of at least 2048
   bits for `RS256` and `PS256`). If the key has its own `alg`, it must equal the header `alg`.
   If the key has `use`, it must be `sig`; if it has `key_ops`, it must include `verify`.
7. **Unknown `kid` with a remote key set.** For a `jwks_uri` client or an upstream provider,
   refetch the set once through `HttpClient` and `Cache`, at most once per 5m per key set, then
   retry the lookup once. The `kid` value is used only as a lookup key: never as a path, URL, file
   name or query fragment ("kid injection").
8. **Signature.** Verify with `jsonwebtoken` (rust_crypto backend) with its own time checks
   switched off (`validate_exp = false`, `validate_nbf = false`), because `jsonwebtoken` reads the
   system clock (ARCH-8, ADR 0005).
9. **Claims decoding.** A JSON object with no duplicate member names, at most 64 members.
10. **Time claims**, against `Clock`, with the skew allowance in KEY-18:
    `exp` is required and must be after `now - skew`; `nbf`, if present, must be before
    `now + skew`; `iat`, if required by the use, must be present and before `now + skew`.
11. **`iss`** equals the use's expected issuer exactly (string comparison, no normalization).
12. **`aud`** contains the expected audience (string or array, [RFC 7519] section 4.1.3), with the
    use-specific rule (for client assertions, KEY-19).
13. **Use-specific checks** (replay caches, `sub` rules, maximum age), defined in the spec of each
    use.

**KEY-18.** Clock skew. For JWTs from others (client assertions, bearer grants, upstream tokens)
we allow 60s on `exp`, `nbf` and `iat`. For our own tokens we allow none: we issued them with
our clock. *Conservative choice:* no larger skew is configurable; a client whose clock is off by
more than a minute must fix its clock.

**KEY-19.** Client assertion audience `[P3]`. *Conservative choice:* the `aud` claim must be the
instance's issuer identifier as its only value (a string, or an array with exactly one element).
The token endpoint URL is not accepted. This follows the update to [RFC 7523] in
[draft-ietf-oauth-rfc7523bis], which closes an audience-injection attack. Some older client
libraries send the token endpoint URL and will fail; this is recorded as a known interoperability
cost in ADR 0005.

**KEY-20.** Every rejected JWT produces an `observed.token_rejected` or
`observed.client_auth_failed` observation with the reason class, never the token.

## 7. JWK thumbprints

The JWK thumbprint ([RFC 7638], SHA-256) is used:

- to detect a duplicate key when a client or machine user registers keys (two keys with the same
  thumbprint are refused);
- as the `jkt` value for DPoP-bound tokens later ([RFC 9449] section 6, `[P6]`).

It is never used as a `kid` for our own keys (KEY-4).

## 8. Client and machine-user key sets `[P3]`

- A client using `private_key_jwt` registers either an inline JWKS (at most 10 keys) or a
  `jwks_uri` (`https` only, fetched through `HttpClient`, body at most 64 KiB, cached for the
  response's `max-age` clamped between 5m and 24h).
- Inline keys must be public keys only. A JWK containing private members (`d`, `p`, `q`, `dp`,
  `dq`, `qi`, `k`) is refused at registration and the request is logged without the key.
- Machine-user keys are covered in [13](13-machine-identity.md).

## Sources

- [RFC 7515]: JSON Web Signature, sections 2, 4.1.9, 5.1, 5.2.
- [RFC 7517]: JSON Web Key.
- [RFC 7518]: JSON Web Algorithms, sections 3 and 3.3 (RSA key size).
- [RFC 7519]: JSON Web Token, section 4.1.3.
- [RFC 7523]: JWT Profile for OAuth 2.0 Client Authentication and Authorization Grants.
- [draft-ietf-oauth-rfc7523bis]: Updates to audience values for OAuth 2.0 authorization servers
  (sole issuer-identifier audience for client authentication).
- [RFC 7638]: JWK Thumbprint.
- [RFC 7797]: JWS Unencoded Payload Option (the `b64` header).
- [RFC 8037]: CFRG curves in JOSE (EdDSA, OKP keys).
- [RFC 8725]: JWT Best Current Practices, sections 3.1, 3.10, 3.11.
- [RFC 9068]: JWT Profile for OAuth 2.0 Access Tokens, section 2.1.
- [RFC 9449]: DPoP, section 6.
- [RFC 9864]: Fully-Specified Algorithms for JOSE and COSE.
- [RFC 6979]: Deterministic DSA and ECDSA.
- [OIDC Registration]: OpenID Connect Dynamic Client Registration 1.0, section 2.
- [OIDC Back-Channel Logout]: section 2.4 (`logout+jwt`).
- [RUSTSEC-2023-0071]: Marvin attack advisory for the `rsa` crate.
- Crate documentation: [jsonwebtoken] (algorithms, `JwtSigner` and `JwtVerifier`, JWK types),
  `ed25519-dalek`, `p256`, `aws-lc-rs`.
- Zitadel public docs, observable behaviour only: a provider of this kind publishes keys with a
  `kid` for local validation and rotates them on a schedule or on demand ([Zitadel endpoints]).
  The lifecycle states, delays and validation steps are ours.

[RFC 7515]: https://www.rfc-editor.org/rfc/rfc7515
[RFC 7517]: https://www.rfc-editor.org/rfc/rfc7517
[RFC 7518]: https://www.rfc-editor.org/rfc/rfc7518
[RFC 7519]: https://www.rfc-editor.org/rfc/rfc7519
[RFC 7523]: https://www.rfc-editor.org/rfc/rfc7523
[draft-ietf-oauth-rfc7523bis]: https://datatracker.ietf.org/doc/draft-ietf-oauth-rfc7523bis/
[RFC 7638]: https://www.rfc-editor.org/rfc/rfc7638
[RFC 7797]: https://www.rfc-editor.org/rfc/rfc7797
[RFC 8037]: https://www.rfc-editor.org/rfc/rfc8037
[RFC 8725]: https://www.rfc-editor.org/rfc/rfc8725
[RFC 9068]: https://www.rfc-editor.org/rfc/rfc9068
[RFC 9449]: https://www.rfc-editor.org/rfc/rfc9449
[RFC 9864]: https://www.rfc-editor.org/rfc/rfc9864
[RFC 6979]: https://www.rfc-editor.org/rfc/rfc6979
[OIDC Registration]: https://openid.net/specs/openid-connect-registration-1_0.html
[OIDC Back-Channel Logout]: https://openid.net/specs/openid-connect-backchannel-1_0.html
[RUSTSEC-2023-0071]: https://rustsec.org/advisories/RUSTSEC-2023-0071.html
[jsonwebtoken]: https://docs.rs/jsonwebtoken/11.1.0/jsonwebtoken/
[Zitadel endpoints]: https://zitadel.com/docs/apis/openidoauth/endpoints
