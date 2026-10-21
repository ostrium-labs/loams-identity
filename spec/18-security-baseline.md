# 18. Security baseline

These requirements hold in every phase. The protocol files repeat the ones that belong to one
endpoint; this file is the full list, so a reviewer can check a release against it. Each
requirement names the test family in [19](19-verification.md) that covers it.

**SEC-0.** A security default in this file MUST NOT be weakened to make a test, a conformance
check or a client integration pass. The conflict is reported instead (owner's rule).

## 1. Protocol defaults

| Id | Requirement | Test family |
|---|---|---|
| SEC-1 | The only authorization `response_type` is `code`. Implicit (`token`, `id_token token`) and hybrid flows are not offered and are refused with `unsupported_response_type`. | protocol |
| SEC-2 | The resource owner password credentials grant is not implemented. | protocol |
| SEC-3 | PKCE with `S256` is required for every client, public and confidential, except confidential OIDC clients with the opt-in `pkce_exception: nonce`, which must then send `openid` and a `nonce` ([07](07-oauth-core.md) OAUTH-14a, owner decision 2026-10-21). `plain` is refused. A token request for a code issued with a challenge must carry a matching verifier, and a token request with a verifier for a code issued without a challenge is refused ([RFC 9700] section 2.1.1). | security: PKCE downgrade |
| SEC-4 | Redirect URIs are compared as exact strings after no normalization, except the port of a loopback redirect for native clients ([RFC 8252] section 7.3). | security: redirect confusion, fuzz |
| SEC-5 | Errors that cannot be tied to a registered redirect URI are shown on our own error page, never redirected ([RFC 6749] section 4.1.2.1). | security: open redirect |
| SEC-6 | Authorization responses carry `iss` ([RFC 9207]). | protocol |
| SEC-7 | Codes live 60 seconds, are single use, and a replay revokes the tokens issued from the first redemption (DM-15). | TLA+, property, security: code replay |
| SEC-8 | Refresh tokens rotate on every use; reuse of a retired token revokes the whole family (DM-18, ADR 0014). | TLA+, property, security: refresh reuse |
| SEC-9 | Post-logout redirect URIs are exact-match against the identified client's registered list, and are ignored when the client cannot be identified. | security: logout open redirect |
| SEC-10 | Token responses and every response that contains a token, code or credential carry `Cache-Control: no-store` ([RFC 6749] section 5.1). | protocol |
| SEC-11 | Bearer tokens are accepted only in the `Authorization` header, and in a form body only where [RFC 6750] section 2.2 allows it for userinfo POST. Never in a query string. | protocol |

## 2. Tokens and keys

| Id | Requirement | Test family |
|---|---|---|
| SEC-12 | `alg: none` is never produced and never accepted. | security: alg confusion, fuzz |
| SEC-13 | HMAC algorithms are never used for tokens we issue. Inbound HMAC-signed JWTs (client assertions with `client_secret_jwt`) are not supported. | security: alg confusion |
| SEC-14 | Every JWT verification uses an algorithm allowlist fixed by the use (for example our own access tokens: the instance's active algorithms), never the token header alone. A key is used only with the algorithm its JWK declares. | security: alg confusion |
| SEC-15 | Keys are never taken from token headers (`jwk`, `jku`, `x5u`, `x5c` are refused). `kid` is looked up only in the key set expected for the issuer of the token, and a `kid` is an opaque string compared exactly, never used as a path, a query or a URL. | security: kid injection, fuzz |
| SEC-16 | `iss` and `aud` are checked on every inbound token; a token from another instance or for another audience is rejected (TEN-15). | security: cross-tenant substitution |
| SEC-17 | Signing keys rotate on a schedule and on demand; private keys never leave the `KeyStore`; `kid` values are random. | key rotation tests |
| SEC-18 | Our JWTs carry an explicit `typ` (`at+jwt` for access tokens, [RFC 9068]; `logout+jwt` for logout tokens), and our verifiers check it, so an ID token cannot be used as an access token ([RFC 8725] section 3.11). | security: token substitution |

## 3. Secrets and data

| Id | Requirement | Test family |
|---|---|---|
| SEC-19 | Passwords are hashed with argon2id (ADR 0009). | unit |
| SEC-20 | Secret comparisons (hashes of tokens, codes, client secrets, CSRF tokens, setup token, OTPs) are constant time (`subtle`). | review, unit |
| SEC-21 | Secrets are held in `secrecy` wrappers, zeroized on drop, redacted in `Debug`, and never logged (ARCH-14). A log-scanning test runs every protocol test with logging at `trace` and fails if any generated secret appears in the output. | log scan |
| SEC-22 | Personal data in events is encrypted with the subject's data key (DM-6). | unit, erasure test |
| SEC-23 | Random values come from the `Rng` trait backed by the OS source in production; token values have at least 256 bits of entropy. | unit |

## 4. Web surface

| Id | Requirement | Test family |
|---|---|---|
| SEC-24 | Cookies set by the provider use the `__Host-` prefix, `Secure`, `HttpOnly` and `Path=/`. `SameSite=Lax` for cookies that must arrive on cross-site top-level navigations (the session, the authorization-request binding `__Host-loams_ar` of OAUTH-7, the upstream-login binding `__Host-loams_idp` of FED-25); `SameSite=Strict` for the CSRF cookie `__Host-loams_csrf`. | protocol |
| SEC-25 | Every state-changing POST in the login UI and the management API (when cookie-authenticated) checks a CSRF token bound to the session or pre-session, and rejects requests whose `Origin` (or `Sec-Fetch-Site`) is cross-site. | security: CSRF |
| SEC-26 | Login UI responses set `Content-Security-Policy` (no inline script except hashed WebAuthn glue, `frame-ancestors 'none'`), `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, and `Strict-Transport-Security` when served over TLS. | protocol |
| SEC-27 | CORS is allowed only on the token, userinfo, JWKS, discovery, introspection and revocation endpoints, only for origins derived from the registered redirect URIs of `spa` clients, and never with credentials (cookies). | protocol |
| SEC-28 | Request bodies are limited (64 KiB for protocol endpoints, 1 MiB for management API), parameters may not repeat ([RFC 6749] section 3.1), and unknown parameters are ignored. | fuzz |

## 5. Abuse resistance

| Id | Requirement | Test family |
|---|---|---|
| SEC-29 | Rate limits per client IP and per target account on login, OTP, token, introspection, device-code polling and password reset (ADR 0024). | rate-limit tests |
| SEC-30 | Account lockout after repeated failures with a timed lock by default, so an attacker cannot lock a victim out forever (ADR 0024). | lockout tests |
| SEC-31 | Login responses for an unknown user, a wrong password and a locked account are the same page, status and size class, and take the same time within noise (a dummy argon2id verification runs for unknown users). | security: timing / enumeration |
| SEC-32 | Password reset and registration do not reveal whether an email or login name exists. | security: enumeration |
| SEC-33 | Argon2id work is bounded by a semaphore sized to memory; excess login attempts queue briefly and then fail with `503`, not with memory exhaustion. | load test |

## 6. Outbound requests

| Id | Requirement | Test family |
|---|---|---|
| SEC-34 | Every outbound fetch (client `jwks_uri`, upstream IdP discovery and JWKS, workload-identity issuer JWKS, webhooks) goes through `HttpClient` with: `https` only, a 5s timeout, a 1 MiB response limit, no redirects to other hosts, and refusal of loopback, link-local, private and multicast addresses unless the instance setting `allow_private_network_fetch` is on (SSRF). The address is checked after DNS resolution and the connection is made to the checked address. | security: SSRF |

## 7. Tenancy

| Id | Requirement | Test family |
|---|---|---|
| SEC-35 | Every read and write path is tenant-scoped (TEN-13 to TEN-18), and the cross-tenant suite covers each one. | cross-tenant |

## 8. Operations and supply chain

| Id | Requirement | Test family |
|---|---|---|
| SEC-36 | The management and system APIs listen on a separate, private listener by default. Exposing them publicly is an explicit configuration choice that logs a warning. | config test |
| SEC-37 | Dependencies pass `cargo-deny` (licences per ADR 0022, advisories, bans, sources) and `cargo-audit` in CI; `cargo-vet` audits are recorded for crypto and protocol crates. | CI |
| SEC-38 | Every crate with C or assembly code is listed in the C-dependency register (ADR 0022) with a reason. | CI (a test compares the register with `cargo metadata`) |
| SEC-39 | Releases are built reproducibly in CI and signed; the release notes list the security defaults that changed. | release |
| SEC-40 | No feature flag or configuration can turn on `alg: none`, implicit flow, ROPC, PKCE `plain`, a PKCE exception for a public client, or wildcard redirect URIs. These options do not exist in the code. | review |

## Sources

- [RFC 6749] sections 3.1, 4.1.2.1, 5.1; [RFC 6750] section 2; [RFC 8252] section 7.3;
  [RFC 9207]; [RFC 9700]; [RFC 8725]; [RFC 9068].
- [OWASP Session Management Cheat Sheet], [OWASP CSRF Prevention Cheat Sheet],
  [OWASP SSRF Prevention Cheat Sheet].
- MDN documentation on cookie prefixes and `Sec-Fetch-Site` (general web platform facts).
- The owner's mission prompt (2026-10-21), section "Security defaults".

[RFC 6749]: https://www.rfc-editor.org/rfc/rfc6749
[RFC 6750]: https://www.rfc-editor.org/rfc/rfc6750
[RFC 8252]: https://www.rfc-editor.org/rfc/rfc8252
[RFC 9207]: https://www.rfc-editor.org/rfc/rfc9207
[RFC 9700]: https://www.rfc-editor.org/rfc/rfc9700
[RFC 8725]: https://www.rfc-editor.org/rfc/rfc8725
[RFC 9068]: https://www.rfc-editor.org/rfc/rfc9068
[OWASP Session Management Cheat Sheet]: https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
[OWASP CSRF Prevention Cheat Sheet]: https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html
[OWASP SSRF Prevention Cheat Sheet]: https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html
