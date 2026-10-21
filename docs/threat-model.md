# Threat model (STRIDE)

Status: Phase A. This model covers Phases 1 to 3 in detail and notes Phases 4 to 6. It is
reviewed at the end of every phase. Mitigations point at requirement ids in `spec/`.

This document is not a substitute for an external security review, which should happen before
the first production deployment at `auth.loams.dev`.

## 1. What we protect

| Asset | Why it matters |
|---|---|
| Signing private keys | Whoever holds them can mint any token for any user of the instance. |
| User credentials: passwords, passkeys, TOTP secrets, recovery codes | Account takeover. |
| Machine credentials: private keys (held by clients), client secrets, PATs | Takeover of services and agents, which in Loams can read and write data. |
| Tokens in flight: codes, access, refresh, ID tokens, session cookies | Session hijack and replay. |
| The event log | Source of truth for every identity; tampering rewrites who can sign in. |
| Personal data (names, emails, IPs) | Privacy and legal duties (erasure). |
| Tenant isolation | One org or instance must never see or act in another. |
| Availability of login and token endpoints | Every Loams surface depends on them. |
| Audit trail | Needed to investigate incidents and to prove who did what. |

## 2. Actors

| Actor | Trust |
|---|---|
| Anonymous internet user | None. |
| Signed-in human user of an org | Their own account and what their org roles allow. |
| Org admin | Their org only. |
| Instance operator (runs the process) | Full trust over the instance, but should not see plaintext secrets or personal data without leaving an audit trail. |
| Relying party (client) | Only what its registration allows. A public client is untrusted code on a user's device. |
| Machine client or agent | Only its own grants; agents are short-lived and possibly compromised (prompt injection). |
| Upstream identity provider `[P5]` | Trusted only for the identities it asserts, only for the orgs that configured it. |
| Webhook or sync consumer | Receives exported events; not trusted with secrets. |
| Supply-chain participant (crate authors, CI) | Code we run; trusted only after review and pinning. |

## 3. Trust boundaries

```text
 [internet] ── TLS ──► public listener: /oauth/*, /login/*, /.well-known/*
                          │
 [private network] ─────► private listener: /api/v1/*, /system/v1/*, metrics
                          │
                     loams-identity process ── KeyStore (root key file or KMS)
                          │
                     event store (redb file or PostgreSQL)
                          │
                     outbox ──► EventSink: identity-sync (Loams), audit, webhooks
                          │
                     HttpClient ──► client jwks_uri, upstream IdPs, workload issuers
```

## 4. STRIDE by component

### 4.1 Login UI and browser session

| STRIDE | Threat | Mitigation |
|---|---|---|
| S | Credential stuffing and password spraying | Rate limits per IP and account, lockout with timed locks, breached-password option, passkeys and MFA (SEC-29, SEC-30, [12](../spec/12-passkeys-and-mfa.md)) |
| S | Phishing of passwords and OTPs | Passkeys bound to the RP ID; phishing-resistant `acr` value lets RPs demand them ([12](../spec/12-passkeys-and-mfa.md)) |
| S | Session cookie theft | `__Host-`, `Secure`, `HttpOnly`, `SameSite`; idle and absolute expiry; session list with remote end (SEC-24, [10](../spec/10-sessions-and-logout.md)) |
| S | Session fixation | A new session id is issued at every authentication step that raises assurance ([10](../spec/10-sessions-and-logout.md)) |
| T | CSRF on login, consent, logout, credential changes | CSRF tokens plus Origin checks (SEC-25) |
| T | Clickjacking of consent | `frame-ancestors 'none'` (SEC-26) |
| R | User denies a login or a consent | `session.factor_verified`, `grant.created` events with request ids and encrypted IP and user agent |
| I | User enumeration through messages or timing | Same page and timing for all failures (SEC-31, SEC-32) |
| D | Argon2id memory exhaustion | Semaphore and queue limits (SEC-33) |
| D | Victim lockout by an attacker | Timed locks by default, not permanent; passkey sign-in unaffected by password lock (DM-13, ADR 0024) |
| E | Script injection in the login UI | Auto-escaping templates, strict CSP (ADR 0019, SEC-26) |

### 4.2 Authorization endpoint

| STRIDE | Threat | Mitigation |
|---|---|---|
| S | Redirect URI manipulation to steal codes | Exact match (SEC-4), error page without redirect (SEC-5) |
| S | Mix-up attacks between authorization servers | `iss` in responses (SEC-6) |
| T | Code injection by an attacker who holds a stolen code | PKCE for every client (SEC-3); the only exception, confidential OIDC clients that opt in, is bound by `nonce` instead (OAUTH-14a) |
| T | CSRF against the client's callback | `state` passed through; PKCE |
| I | Leaking codes through Referer or logs | Codes in query only to registered URIs, `Referrer-Policy: no-referrer` on our pages, short lifetime (SEC-7), never logged (SEC-21) |
| E | Scope escalation by requesting unregistered scopes | Narrowing to `allowed_scopes` ([07](../spec/07-oauth-core.md)) |
| E | Cross-org login to a client of another org | TEN-16 |

### 4.3 Token endpoint

| STRIDE | Threat | Mitigation |
|---|---|---|
| S | Client impersonation | Client authentication per registered method, constant-time compares (SEC-20), assertions with `jti` replay cache ([13](../spec/13-machine-identity.md)) |
| S | Stolen refresh token used by an attacker | Rotation with reuse detection kills the family (SEC-8); session binding for non-offline tokens |
| T | Race: parallel redemption of one code or refresh token | Optimistic concurrency on the stream; one winner (DM-15, ES-2) |
| T | Token issued under a grant being revoked at the same moment | Read checks in the issuing batch (ES-10) |
| R | Client denies having obtained tokens | `token_family.*` events with client id and request id |
| D | Brute force of codes or refresh tokens | 256-bit values; rate limits |
| E | Token exchange producing more rights than either party | Narrow-only rules, delegation policy, `act` chain ([13](../spec/13-machine-identity.md)) |

### 4.4 Keys and JWKS

| STRIDE | Threat | Mitigation |
|---|---|---|
| S | Forged tokens via `alg` confusion or header-supplied keys | SEC-12 to SEC-15 |
| T | Key substitution in storage | Private keys only in `KeyStore`; public JWKs in events; startup check that every active key handle signs a test message verifiable with its published JWK |
| I | Private key extraction from memory or backups | Keys sealed with a root key outside the database; KMS later; zeroize (SEC-17, SEC-21) |
| I | RSA private-key timing leak | RSA signing through aws-lc-rs, not the `rsa` crate (ADR 0006) |
| D | Rotation removes a key that live tokens still need | Retiring keys stay published past the longest token lifetime ([06](../spec/06-keys-and-jose.md)) |

### 4.5 Introspection, revocation, userinfo

| STRIDE | Threat | Mitigation |
|---|---|---|
| S | Unauthenticated probing of tokens | Introspection requires client authentication ([08](../spec/08-tokens.md)) |
| I | A resource server learns about tokens not meant for it | Introspection answers `active: false` unless the caller is in the token's audience or is its client ([08](../spec/08-tokens.md)) |
| T | Revocation not effective for JWT access tokens | Short lifetimes (15m default); introspection and state checks for callers that need immediacy; Loams gets revocation events through identity-sync ([15](../spec/15-loams-integration.md)) |

### 4.6 Event store and projections

| STRIDE | Threat | Mitigation |
|---|---|---|
| T | Direct database tampering by someone with store access | Out of scope for the code (operator trust), but the audit export to an external sink records the original events; a later phase may add a hash chain over each stream |
| T | Inconsistent state after a crash | Atomic batches, idempotent projections and fan-out, simulation with crash injection (ES-1, ES-12, VER-12) |
| I | Personal data in an immutable log | Crypto-shredding (DM-6, ADR 0018) |
| I | Cross-tenant reads by a bug | Scoped query API, cross-tenant suite (TEN-13, VER-16) |
| D | Event log growth from abusive traffic | Observations instead of domain events for failures; retention for ephemeral streams ([05](../spec/05-event-store-and-projections.md) section 7) |

### 4.7 Management and system APIs

| STRIDE | Threat | Mitigation |
|---|---|---|
| S | Stolen admin token | Private listener by default (SEC-36), short tokens, step-up for sensitive actions ([17](../spec/17-management-api.md)) |
| E | Org admin acting on another org | TEN-17 (404 for other orgs), Authorizer deny-by-default |
| E | `AllowAll` authorizer left on | Refuses to start unless loopback-only (ARCH-5) |
| R | Admin denies a change | Every management command records the actor in the event envelope |

### 4.8 Outbox and sinks

| STRIDE | Threat | Mitigation |
|---|---|---|
| S | Forged webhook calls to Loams' identity-sync | Signed deliveries (Standard Webhooks HMAC, timestamp window) ([15](../spec/15-loams-integration.md)) |
| I | Secrets or personal data exported | EV-5 strips secret fields; personal data only to sinks allowed to receive it |
| T | Lost or reordered deliveries leave OpenFGA out of date (a removed member keeps access) | At-least-once delivery, per-stream ordering by version, idempotent consumers, a pull feed for reconciliation ([15](../spec/15-loams-integration.md)) |
| D | A slow sink blocks the system | Outbox is asynchronous; per-sink backoff; lag metric and alert |

### 4.9 Outbound fetches (federation, client keys, workload identity)

| STRIDE | Threat | Mitigation |
|---|---|---|
| S | An attacker-controlled upstream IdP asserts a victim's email | No auto-linking on unverified email; linking needs the existing account's sign-in ([16](../spec/16-federation.md)) |
| E | SSRF into the private network | SEC-34 |
| D | Slow upstream stalls logins | Timeouts and caching (SEC-34, ADR 0021) |

### 4.10 Machine identity and agents

| STRIDE | Threat | Mitigation |
|---|---|---|
| S | A compromised agent keeps using its credentials | Short TTL cap (15m default, 1h max), no refresh tokens for agents, immediate deactivation with state checks (DM-14), agent sessions that end ([13](../spec/13-machine-identity.md)) |
| S | Replay of a client assertion | `jti` replay cache, short assertion lifetime |
| E | An agent acting for a user beyond the user's consent | Delegation only from an explicit user grant to that agent or an org policy; `act` recorded; narrow-only ([13](../spec/13-machine-identity.md)) |
| E | Workload-identity trust policy too broad (for example any repository) | Policies need issuer, audience and an exact or anchored subject pattern; wildcards limited to the last path segment ([13](../spec/13-machine-identity.md)) |
| R | Agent actions not attributable | `principal`, `agent_owner`, `agent_session` and `act` claims; audit fields ([15](../spec/15-loams-integration.md)) |

### 4.11 Supply chain and build

| STRIDE | Threat | Mitigation |
|---|---|---|
| T | Malicious or vulnerable crate | `cargo-deny`, `cargo-audit`, `cargo-vet`, pinned versions, minimal dependencies (SEC-37) |
| T | Compromised CI action | Actions pinned by commit SHA; least-privilege tokens |
| I | Licence contamination (AGPL code) | Clean-room process (ADR 0001), licence allowlist (ADR 0022) |
| T | C dependencies with memory-safety bugs (OpenSSL via webauthn-rs, AWS-LC) | C-dependency register (SEC-38); keep them behind traits so they can be replaced |

## 5. Top threats

Ranked by likelihood times impact for a public deployment such as `auth.loams.dev`.

| Rank | Threat | Main mitigations |
|---|---|---|
| 1 | Account takeover through credential stuffing and phishing of passwords and OTPs | Rate limits, lockout, passkeys first, MFA policy, phishing-resistant `acr` |
| 2 | Cross-tenant data or token leakage through a scoping bug | Scoped query API, `iss` and instance checks, generated cross-tenant suite |
| 3 | Signing key compromise (backup leak, operator mistake, RSA timing) | KeyStore isolation, root key outside the DB, rotation, aws-lc-rs for RSA, KMS later |
| 4 | Stolen refresh token or session cookie used silently | Rotation with family revocation, session binding, short access tokens, session list |
| 5 | Compromised or prompt-injected agent misusing its access | Short TTLs, no refresh, agent sessions, narrow-only delegation, fast deactivation and sync |
| 6 | Redirect URI or logout redirect abuse leading to code theft or phishing | Exact match, error page, RFC 9207 `iss`, PKCE |
| 7 | Revocation not reaching Loams in time (sync lag) | Short tokens, at-least-once sync with reconciliation, introspection for sensitive calls |
| 8 | JWT validation flaws (alg confusion, `kid` injection, type confusion) | Allowlists, no header keys, `typ` checks, fuzzing |
| 9 | Supply-chain compromise or licence contamination | Pinning, deny/audit/vet, clean-room process |
| 10 | Denial of service on login (argon2id cost, lockout of victims) | Bounded hashing, timed locks, per-IP limits |

## 6. Out of scope for now

- A malicious operator with root on the host and the root key (they can do anything; KMS and
  hardware keys narrow this later).
- Side channels on shared hardware beyond what the crypto crates already address.
- Denial of service at the network layer (handled by the front proxy or gateway).

## Sources

- [RFC 6819]: OAuth 2.0 Threat Model and Security Considerations.
- [RFC 9700]: OAuth 2.0 Security Best Current Practice.
- [RFC 8725]: JSON Web Token Best Current Practices.
- [OpenID Connect Core 1.0] section 16 (Security Considerations).
- [WebAuthn Level 3] section 13 (Security Considerations).
- [NIST SP 800-63B]: Digital Identity Guidelines, Authentication.
- Microsoft's STRIDE categories as a general method.
- RustSec advisory [RUSTSEC-2023-0071] (RSA timing).

[RFC 6819]: https://www.rfc-editor.org/rfc/rfc6819
[RFC 9700]: https://www.rfc-editor.org/rfc/rfc9700
[RFC 8725]: https://www.rfc-editor.org/rfc/rfc8725
[OpenID Connect Core 1.0]: https://openid.net/specs/openid-connect-core-1_0.html
[WebAuthn Level 3]: https://www.w3.org/TR/webauthn-3/
[NIST SP 800-63B]: https://pages.nist.gov/800-63-4/sp800-63b.html
[RUSTSEC-2023-0071]: https://rustsec.org/advisories/RUSTSEC-2023-0071.html
