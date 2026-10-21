# 08. Tokens: access tokens, refresh-token families, introspection, revocation

## 1. Lifetimes

From the token policy in [03](03-data-model.md) section 3. A client may shorten these, never
lengthen them beyond the instance maximum.

| Token | Default | Maximum | Notes |
|---|---|---|---|
| Authorization code | 60s | 60s | Single use (DM-15) |
| Access token (user) | 15m | 1h | |
| Access token (agent) | `agent_token_ttl_cap`, default 15m | 1h | Also capped by the agent's own `token_ttl_cap` and by its agent session's `expires_at` ([13](13-machine-identity.md)) |
| ID token | 15m | 1h | [09](09-oidc.md) |
| Refresh token, idle | 14d | 90d | Measured from the last rotation |
| Refresh token, absolute | 30d | 365d | Measured from the family's creation |

**TOK-1.** An access token never outlives its family's absolute expiry, its session's absolute
expiry (session-bound families) or its agent session's expiry. The issued `exp` is the minimum of
all applicable limits, and `expires_in` in the response matches it.

## 2. Access tokens

### 2.1 Formats

**TOK-2.** Each client has `access_token_format` `jwt` (default) or `opaque` (ADR 0012).

- **JWT** follows [RFC 9068]: header `typ: at+jwt`, signed with the instance's active `EdDSA` key
  by default (KEY-1). A `resource_server` may set the algorithm its audience tokens are signed
  with (`EdDSA`, `ES256` or `RS256`), for resource servers whose libraries lack EdDSA.
- **Opaque** tokens are `lia_` plus 32 bytes from `Rng`, base64url without padding (ADR 0013).
  The server stores SHA-256 of the string in the `opaque_access_hashes` inline index, mapping to
  (family id, `jti`). Resource servers must introspect them.

### 2.2 Claims of a JWT access token

| Claim | Value | Always |
|---|---|---|
| `iss` | Instance issuer | Yes |
| `sub` | The user id (human or machine) | Yes |
| `aud` | Section 2.3 | Yes |
| `exp`, `iat`, `nbf` | NumericDate; `nbf` equals `iat` | Yes |
| `jti` | UUIDv7 string, recorded in `access_token_ids` | Yes |
| `client_id` | The client the token was issued to ([RFC 9068] section 2.2) | Yes |
| `scope` | Space-separated granted scopes for this token | Yes |
| `auth_time`, `acr`, `amr` | From the user authentication that started the family | When a user authenticated interactively |
| `sid` | Browser session id | Session-bound families only |
| `act` | Actor chain ([RFC 8693] section 4.1) | Token exchange with an actor `[P3]` |
| `org` | The subject's org id | Yes |
| `principal` | `user`, `service_account` or `agent` | Yes |
| `agent_owner` | The agent's owner user id | Agents `[P3]` |
| `agent_session` | The agent session id | Agent-session-scoped tokens `[P3]` |

The private claims `org`, `principal`, `agent_owner` and `agent_session` are the Loams contract;
their exact meaning and the consumer's checks are in [15](15-loams-integration.md) (ADR 0017).

**TOK-3.** Access tokens carry no personal data: no email, name, phone or login name. Resource
servers that need them call userinfo or the management API.

**TOK-4.** Claim serialization is deterministic (KEY-15). An access token is at most 4 KiB; a
token that would be larger (for example a long `act` chain) fails issuance with `server_error`
and an internal error that names the cause. `act` chains are at most 4 deep.

### 2.3 Audience

**TOK-5.** Phase 1 and 2: `aud` is an array of the client's `client_id` and, when the token's
scopes include `openid`, the instance issuer (so the token is accepted at userinfo, OIDC-21).
Resource servers that accept tokens issued to a known set of clients check `client_id` and
`aud` against their configuration.

**TOK-6.** `[P3]` With one or more `resource` parameters ([RFC 8707]), `aud` is exactly the
requested resource identifiers, plus the issuer when `openid` is in scope. A resource server
application may also be set as a client's default resource, used when the client sends no
`resource`. Phase 4 adds project ids as audiences ([14](14-orgs-projects-roles.md)).

### 2.4 Bearer use

**TOK-7.** Our endpoints that accept access tokens (userinfo, and the management API when called
with user tokens) read them from `Authorization: Bearer` ([RFC 6750] section 2.1). The form-body
method ([RFC 6750] section 2.2) is accepted at userinfo only (OIDC-20). The query-parameter method
is never accepted.

**TOK-8.** Failures follow [RFC 6750] section 3: `401` with
`WWW-Authenticate: Bearer realm="<issuer>", error="invalid_token"` for a missing, malformed,
expired or inactive token; `403` with `error="insufficient_scope"` and a `scope` attribute when
the token lacks a needed scope. A request with no token gets `401` with no `error` attribute.

## 3. Refresh tokens and families

### 3.1 When a refresh token is issued

**TOK-9.** A refresh token is issued only when the client has the `refresh_token` grant type and
the grant that produced the tokens is `authorization_code` or device code `[P3]`. Client
credentials, JWT bearer and token exchange produce no refresh token in these phases.

**TOK-10.** Format: `lir_` plus 32 bytes from `Rng`, base64url without padding. Only its SHA-256 is
stored (`current_refresh_hash`, and the `refresh_hashes` index).

**TOK-11.** Public clients receive refresh tokens; rotation (TOK-12) is what makes that acceptable
([RFC 9700] section 4.14.2). Sender-constrained refresh tokens with DPoP come later `[P6]`.

### 3.2 Rotation and reuse detection

**TOK-12.** Every successful refresh rotates: the presented token's hash moves from
`refresh_hashes` to `retired_refresh`, a new token is generated, `generation` increments, and
`token_family.rotated` is appended with the new hash and the new access token id, all in one
batch with an expected version (ES-1, ES-2). The presented token is never valid again.

**TOK-13.** *Conservative choice* (ADR 0014): there is **no grace window**. If two requests present
the same current refresh token, exactly one wins the append; the other then finds the token in
`retired_refresh` and triggers reuse detection, which revokes the whole family. Clients must not
refresh concurrently with the same token; browser apps with several tabs must coordinate (for
example through a single worker). This is documented for client developers.

### 3.3 Session binding

**TOK-14.** A family created without the `offline_access` scope is **session-bound**: it records the
browser session id, and it becomes unusable as soon as the session is `ended` or `expired`. Each
refresh read-checks the session (ES-10), and the fan-out appends `token_family.revoked` (reason
`session_ended`) for the record (ES-11). Its idle and absolute expiry are also capped by the
session's.

**TOK-15.** A family created with `offline_access` (granted per OAUTH-10) is **not session-bound**:
it survives logout, keeps its own idle and absolute expiry, and ends only by expiry or revocation.
Logout with "sign out everywhere" ([10](10-sessions-and-logout.md)) revokes these too.

### 3.4 The family state machine

States and transitions are in [03](03-data-model.md) section 4.5 (DM-17 to DM-19). The exact checks
on each refresh request, in order, are OAUTH-22. Summary of what each input does:

| Input | Family `active`, not expired | Family `revoked` or `expired` |
|---|---|---|
| Current refresh token, right client | Rotate (TOK-12) | `invalid_grant` |
| Current refresh token, wrong client | `invalid_grant`, no change | `invalid_grant` |
| Retired refresh token, right client | Revoke (`reuse_detected`), `invalid_grant` | `invalid_grant` |
| Retired refresh token, wrong client | `invalid_grant`, no change, observation recorded | `invalid_grant` |
| Unknown token | `invalid_grant` | not applicable |

*Conservative choice on the "retired token, wrong client" row:* we do not revoke, so a client
cannot revoke another client's family; but presenting someone else's retired token is recorded as
an observation, and an alert threshold on it is part of the operator guide.

### 3.5 Revocation causes

Every cause appends `token_family.revoked` with a reason. Direct causes revoke in the request's own
batch; indirect causes revoke through the fan-out (ES-11), and correctness never waits for the
fan-out because every issuing and checking path read-checks the parent.

| Cause | Trigger | Families revoked | Reason |
|---|---|---|---|
| Refresh-token reuse | OAUTH-22 step 2 | That family | `reuse_detected` |
| Code replay | OAUTH-21 step 2 | The family from the first redemption | `code_replayed` |
| Client revokes a refresh token | Revocation endpoint (section 5) | That family | `client` |
| Client revokes an access token | Revocation endpoint | None; that `jti` only (TOK-25) | not applicable |
| User revokes an app in account settings | `grant.revoked` | All families of the grant | `grant_revoked` |
| Admin revokes a grant, a family or all of a user's tokens | Management API | As targeted | `admin` |
| Browser session ends or expires | `session.ended`, `session.expired` | Session-bound families of that session | `session_ended` |
| User deactivated, locked with `admin_unlock_only`, removed or erased | `user.*` | All the user's families | `user_deactivated` |
| Client deactivated or removed | `application.*` | All the client's families | `client_removed` |
| Org deactivated or removed | `org.*` | All families in the org | `admin` |
| Agent session ended `[P3]` | `agent_session.ended` | Families with that agent session | `agent_session_ended` |

Changing a user's password does not revoke families by itself. *Conservative choice:* the
password-change page offers "sign out other sessions" checked by default, and a password **reset**
(forgotten password) always ends all of the user's sessions and revokes all their families.

## 4. Introspection ([RFC 7662])

**TOK-16.** `POST /oauth/introspect`, form-encoded, parameters `token` and optional
`token_type_hint` (`access_token` or `refresh_token`). The hint only orders the lookups; an
unknown hint value is ignored ([RFC 7662] section 2.1).

**TOK-17.** The caller must authenticate as a confidential client: `web`, `service` or
`resource_server` (OAUTH-26 to OAUTH-29). Public clients are refused with `401 invalid_client`.

**TOK-18.** The caller learns about a token only if it is entitled to: the token was issued to the
caller (`client_id` matches), or the caller is a `resource_server` whose `client_id` or one of whose
`resource_identifiers` is in the token's `aud`. Refresh tokens can be introspected only by the
client they were issued to. Otherwise the answer is `{"active": false}`, indistinguishable from an
unknown token.

**TOK-19.** Lookup:

- A string starting with `lia_` or `lir_`: SHA-256 and the matching index.
- Otherwise: validate as "our access token" per KEY-17 with no clock skew (KEY-18), then look up
  `jti` in `access_token_ids`.

**TOK-20.** The token is active only if all of these hold, checked against the aggregates and
inline indexes, never asynchronous projections (ES-14): not expired by `Clock`; its family is
`active` and the `jti` is not revoked (TOK-25); the user, client and org are `active`; the grant
(if any) is `active`; the session (if session-bound) is `active`; the agent session (if any) is
`active`; and the agent has an active owner (DM-8).

**TOK-21.** An active response contains: `active: true`, `scope`, `client_id`, `token_type`
(`Bearer` for access tokens, `refresh_token` for refresh tokens), `exp`, `iat`, `nbf`, `sub`, `aud`,
`iss`, `jti`, and when present `auth_time`, `acr`, `amr`, `sid`, `act`, `org`, `principal`,
`agent_owner`, `agent_session`. It never contains `username` or other personal data (TOK-3).
*Conservative choice:* signed introspection responses ([RFC 9701]) are not offered in these
phases.

**TOK-22.** Responses carry `Cache-Control: no-store`. Introspection is rate-limited per caller.
Missing `token` fails with `invalid_request`.

## 5. Revocation ([RFC 7009])

**TOK-23.** `POST /oauth/revoke`, form-encoded, `token` and optional `token_type_hint`.
Confidential clients authenticate (OAUTH-26); public clients send `client_id` and are identified by
it ([RFC 7009] section 2.1).

**TOK-24.** Outcomes:

| Token | Result |
|---|---|
| Unknown, expired, already revoked, or malformed | `200`, empty body ([RFC 7009] section 2.2) |
| A refresh token issued to the caller | The family is revoked (reason `client`); `200` |
| An access token issued to the caller | That access token is revoked (TOK-25); the family stays active; `200` |
| A token issued to another client | `400 invalid_request` with description "token was not issued to this client"; nothing changes ([RFC 7009] section 2.1 requires refusing) |
| `token_type_hint` with an unsupported value | Ignored; the server searches all types |

**TOK-25.** Revoking a single access token appends `token_family.access_token_revoked` with its
`jti`. Introspection and userinfo treat that `jti` as inactive from then on. JWT access tokens
remain cryptographically valid until `exp` for resource servers that validate locally; that is the
reason for short access-token lifetimes and for Loams' revocation feed ([15](15-loams-integration.md)).

**TOK-26.** Revocation responses carry `Cache-Control: no-store` and CORS for `spa` clients
(OAUTH-32).

## 6. Token hygiene

- **TOK-27.** Tokens and codes appear only in response bodies, `Location` headers (codes) and
  `Authorization` headers. They never appear in logs, traces, metrics labels, error descriptions
  or events (DM-4, ARCH-14).
- **TOK-28.** Every token string is compared by hashing and an index lookup, never by scanning.
  The few direct comparisons (PKCE, client secrets) are constant time.
- **TOK-29.** All token prefixes are registered with secret-scanning services where possible, so
  leaked tokens can be found; the prefixes are listed in ADR 0013.

## Sources

- [RFC 6749]: OAuth 2.0, sections 1.5, 6, 10.4.
- [RFC 6750]: Bearer Token Usage, sections 2 and 3.
- [RFC 7009]: Token Revocation, sections 2.1 and 2.2.
- [RFC 7662]: Token Introspection, sections 2.1 and 2.2.
- [RFC 8693]: Token Exchange, section 4.1.
- [RFC 8707]: Resource Indicators.
- [RFC 9068]: JWT Profile for OAuth 2.0 Access Tokens, sections 2.1 and 2.2.
- [RFC 9700]: OAuth 2.0 Security BCP, section 4.14.2 (refresh token protection).
- [RFC 9701]: JWT Response for Token Introspection (not adopted).
- [OIDC Core]: section 11 (offline access).
- Loams design 19 section 5.3 (short-lived agent tokens, no refresh tokens for agents) as a
  Loams-side contract.
- Zitadel public docs, observable behaviour only: introspection requires client authentication
  and reports revocation and logout state rather than just the signature; revoking a refresh
  token also invalidates its access tokens ([Zitadel endpoints]). The family model, the
  entitlement rule, the reuse rule and every default here are ours.

[RFC 6749]: https://www.rfc-editor.org/rfc/rfc6749
[RFC 6750]: https://www.rfc-editor.org/rfc/rfc6750
[RFC 7009]: https://www.rfc-editor.org/rfc/rfc7009
[RFC 7662]: https://www.rfc-editor.org/rfc/rfc7662
[RFC 8693]: https://www.rfc-editor.org/rfc/rfc8693
[RFC 8707]: https://www.rfc-editor.org/rfc/rfc8707
[RFC 9068]: https://www.rfc-editor.org/rfc/rfc9068
[RFC 9700]: https://www.rfc-editor.org/rfc/rfc9700
[RFC 9701]: https://www.rfc-editor.org/rfc/rfc9701
[OIDC Core]: https://openid.net/specs/openid-connect-core-1_0.html
[Zitadel endpoints]: https://zitadel.com/docs/apis/openidoauth/endpoints
