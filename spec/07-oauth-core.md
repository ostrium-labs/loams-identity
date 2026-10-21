# 07. OAuth core: clients, authorization endpoint, token endpoint

This file specifies the OAuth 2.0 authorization server behaviour ([RFC 6749]) with the OAuth 2.1
defaults ([draft-ietf-oauth-v2-1], revision 15) and the security BCP ([RFC 9700]). ID tokens and
OIDC request parameters are in [09](09-oidc.md); token formats, refresh families, introspection
and revocation are in [08](08-tokens.md); machine grants are in [13](13-machine-identity.md).

## 1. Endpoints

All paths are relative to the instance issuer (TEN-1).

| Path | Method | Purpose | Phase |
|---|---|---|---|
| `/.well-known/openid-configuration` | GET | Discovery ([09](09-oidc.md) section 7) | 1 |
| `/.well-known/oauth-authorization-server` | GET | Same metadata, [RFC 8414] location | 1 |
| `/oauth/authorize` | GET, POST | Authorization endpoint | 1 |
| `/oauth/token` | POST | Token endpoint | 1 |
| `/oauth/introspect` | POST | Introspection ([08](08-tokens.md)) | 1 |
| `/oauth/revoke` | POST | Revocation ([08](08-tokens.md)) | 1 |
| `/oauth/userinfo` | GET, POST | UserInfo ([09](09-oidc.md)) | 1 |
| `/oauth/jwks` | GET | JWKS ([06](06-keys-and-jose.md)) | 1 |
| `/oauth/logout` | GET, POST | End-session endpoint ([10](10-sessions-and-logout.md)) | 2 |
| `/oauth/device_authorization` | POST | Device authorization ([13](13-machine-identity.md)) | 3 |
| `/oauth/par` | POST | Pushed authorization requests ([20](20-later-standards.md)) | later |
| `/oauth/register` | POST | Dynamic client registration ([20](20-later-standards.md)) | later |

**OAUTH-1.** Every endpoint rejects requests over plain HTTP, except on loopback listeners in
development mode. The public listener sends `Strict-Transport-Security: max-age=63072000` when it
terminates TLS itself; when TLS ends at a proxy, the operator configures it there.

**OAUTH-2.** Request parameters MUST NOT appear more than once ([RFC 6749] section 3.1). A request
with a repeated parameter fails with `invalid_request`. Unknown parameters are ignored. Parameter
values are UTF-8; invalid UTF-8 fails with `invalid_request`. The query string or form body is at
most 8 KiB.

## 2. Client kinds

| Kind | Client type | Grant types it may hold | Authenticates at the token endpoint with | Notes |
|---|---|---|---|---|
| `web` | confidential | `authorization_code`, `refresh_token` | `client_secret_basic`, `client_secret_post`, `private_key_jwt` `[P3]` | Server-side web app |
| `spa` | public | `authorization_code`, `refresh_token` | `none` | Browser app; CORS on token, revoke and userinfo (section 9) |
| `native` | public | `authorization_code`, `refresh_token`, device code `[P3]` | `none` | Desktop, mobile and CLI apps; [RFC 8252] redirect forms |
| `service` | confidential | `client_credentials`, JWT bearer, token exchange (all `[P3]`) | `client_secret_basic`, `client_secret_post`, `private_key_jwt` | No browser flows; never receives a code. Exists only as the implicit client of a machine user; admins cannot create standalone `service` applications ([13](13-machine-identity.md) MACH-5a) |
| `resource_server` | confidential | none | `client_secret_basic`, `client_secret_post`, `private_key_jwt` `[P3]` | Only calls introspection; its identifiers are token audiences |

**OAUTH-3.** A client may use only the grant types listed for its kind and enabled in its
`grant_types`. Any other grant fails with `unauthorized_client`. A grant type the server does not
implement at all fails with `unsupported_grant_type`.

**OAUTH-4.** A client has exactly one `token_endpoint_auth_method` (DM-10). A request that
authenticates with a different method, or with more than one method at once, fails with
`invalid_client` ([RFC 6749] section 2.3).

## 3. Authorization endpoint

### 3.1 Parsing

**OAUTH-5.** The endpoint accepts GET with a query string and POST with
`application/x-www-form-urlencoded` ([OIDC Core] section 3.1.2.1). Both are treated the same.

### 3.2 Validation order

Validation runs in this order. Steps 1 to 4 can fail only with an **error page** rendered by us,
because until the redirect URI is known to belong to the client we must not redirect anywhere
([RFC 6749] section 4.1.2.1). From step 5 on, errors are **redirected** to the validated
redirect URI.

1. **Instance.** Resolved by TEN-2. Unknown: plain `404`.
2. **`client_id`.** Present, known in this instance, the client is `active`, its org is
   `active`, and its kind is `web`, `spa` or `native`. Otherwise: error page "unknown or
   unavailable client".
3. **`redirect_uri`.** Required. *Conservative choice:* required even when the client has only
   one registered URI (OAuth 2.1 would allow omitting it; [OIDC Core] requires it). It must match
   a registered URI by OAUTH-8. Otherwise: error page "redirect URI not registered for this
   client". The page shows no link to the unvalidated URI.
4. **`request` and `request_uri`.** `request` (a request object) is not supported in these phases:
   with a valid client and redirect URI this becomes a redirected `request_not_supported` error.
   `request_uri` is accepted only once PAR exists ([20](20-later-standards.md)); until then it is a
   redirected `request_uri_not_supported`.
5. **`response_type`.** Must be exactly `code`. Anything else: `unsupported_response_type`. This
   rules out the implicit and hybrid flows.
6. **`response_mode`.** Absent, `query` or `form_post` ([OAuth Form Post]). `fragment` or any other
   value: `invalid_request`. Error responses from here on use the requested mode when it is valid,
   else `query`.
7. **`scope`.** Section 6.
8. **PKCE.** Section 4.
9. **`state`.** Optional, at most 1024 bytes, passed through unchanged. *Conservative choice:*
   longer values fail with `invalid_request`.
10. **OIDC parameters** (`nonce`, `prompt`, `max_age`, `id_token_hint`, `login_hint`, `acr_values`,
    `ui_locales`, `claims`): [09](09-oidc.md) section 5.
11. **Org hint.** The scope `urn:loams:org:<org_id>` or the parameter `org_hint` (TEN-11).
12. **`resource`** `[P3]`: each value an absolute URI with no fragment ([RFC 8707] section 2),
    matching a `resource_identifiers` entry of an active `resource_server` application in the
    same instance that the client is allowed to request. Otherwise: `invalid_target`.

**OAUTH-6.** On success the server appends `auth_request.created` and redirects the browser
(`303`) to the login UI with the auth request id (`/login?ar=<id>`, [11](11-login-and-passwords.md)).

**OAUTH-7.** The auth request is bound to the browser that started it. The authorization endpoint
sets a cookie `__Host-loams_ar` (`Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/`, lifetime 10m)
holding an HMAC of the auth request id under an instance key. The login UI and the consent page
refuse to act on an auth request whose id does not match this cookie, so an attacker cannot start
a request and make a victim's browser finish it. The details of the login steps are in
[11](11-login-and-passwords.md).

### 3.3 Redirect URI matching

**OAUTH-8.** The presented `redirect_uri` matches a registered one if and only if:

- the two strings are identical byte for byte (no case folding, no percent-decoding, no trailing
  slash or default-port normalization) ([RFC 9700] section 4.1.3); or
- the client is `native`, the registered URI uses `http` with the host `127.0.0.1` or `[::1]`, and
  the two are identical except for the port ([RFC 8252] section 7.3). The registered URI may omit
  the port. `localhost` gets no port flexibility (it must match exactly), because [RFC 8252]
  section 8.3 recommends the IP literal.

Registration rules are DM-11. No wildcards, no pattern matching, no prefix matching.

### 3.4 After authentication and consent

**OAUTH-9.** The login UI authenticates the user ([11](11-login-and-passwords.md)) and appends
`auth_request.authenticated`. The server then checks that the user is allowed for this client
(TEN-16). A user who is not allowed gets the same `access_denied` redirect, after the same delay
class, as a user who cancels.

**OAUTH-10.** Consent:

- A `first_party` client skips the consent page, unless `prompt=consent` is present.
- Otherwise, if an active grant for (user, client) covers every requested scope, consent is
  skipped unless `prompt=consent` is present.
- Otherwise the consent page lists the client's name, its org, and each requested scope in plain
  language. The user can approve or deny. Denial redirects with `access_denied`.
- Approving appends `auth_request.consented` and, in the same batch, `grant.created` (no active
  grant) or `grant.scopes_changed` (widening an existing grant). Consent never narrows an existing
  grant; narrowing is done by the user in their account settings.
- `offline_access` needs explicit consent (or `first_party`) per [OIDC Core] section 11.

### 3.5 The authorization response

**OAUTH-11.** The code is `lic_` followed by 32 bytes from `Rng`, base64url without padding
(ADR 0013). The server appends `auth_request.code_issued` with the code's SHA-256 and an expiry of
`code_ttl` (60s) and claims the hash in the `code_hashes` index in the same batch.

**OAUTH-12.** The success response carries `code`, `state` (if one was sent) and `iss` (the
instance issuer, [RFC 9207]). Error responses carry `error`, an optional `error_description`,
`state` and `iss`. With `response_mode=query` the parameters are added to the redirect URI's query
(keeping any query it already has, [RFC 6749] section 3.1.2) and sent with `303`. With
`form_post` the server returns an HTML page that posts the parameters to the redirect URI
([OAuth Form Post]); the page has `Cache-Control: no-store`, a CSP that allows only its own
inline script by nonce, and a `<noscript>` submit button.

**OAUTH-13.** Error descriptions are fixed English strings from a table in code. They never contain
request values, user data or internal detail.

## 4. PKCE

**OAUTH-14.** Every authorization request, for every client kind, must carry `code_challenge` and
`code_challenge_method=S256` ([RFC 7636], [RFC 9700] section 2.1.1). A missing challenge, a
missing method (which means `plain`, [RFC 7636] section 4.3) or `plain` fails with
`invalid_request` and the description "PKCE with S256 is required". *Conservative choice:* this
applies to confidential clients too, although [RFC 9700] allows a confidential OIDC client to rely
on `nonce` instead. See section 11 for the conformance-suite consequence.

**OAUTH-15.** `code_challenge` must be exactly 43 characters of the base64url alphabet (the
encoding of a 32-byte SHA-256 output). Otherwise `invalid_request`.

**OAUTH-16.** At the token endpoint, `code_verifier` must be 43 to 128 characters from the
unreserved set `[A-Za-z0-9-._~]` ([RFC 7636] section 4.1). The server computes
`BASE64URL(SHA-256(ASCII(code_verifier)))` and compares it with the stored challenge in constant
time.

## 5. Token endpoint

### 5.1 Common rules

**OAUTH-17.** Only POST with `Content-Type: application/x-www-form-urlencoded`. Other methods get
`405`; other content types fail with `invalid_request`.

**OAUTH-18.** Every response, success or error, carries `Cache-Control: no-store` and
`Pragma: no-cache` ([RFC 6749] section 5.1), and `Content-Type: application/json`.

**OAUTH-19.** Client authentication happens first (section 7). Then the grant type is dispatched:
`authorization_code` (5.2), `refresh_token` (5.3), and in Phase 3 `client_credentials`,
`urn:ietf:params:oauth:grant-type:jwt-bearer`, `urn:ietf:params:oauth:grant-type:device_code` and
`urn:ietf:params:oauth:grant-type:token-exchange` ([13](13-machine-identity.md)). A missing
`grant_type` fails with `invalid_request`.

**OAUTH-20.** A successful response contains `access_token`, `token_type: "Bearer"`, `expires_in`,
`scope` (always, the scopes actually granted, so the client never has to guess,
[RFC 6749] section 5.1), and when applicable `refresh_token` and `id_token`.

### 5.2 `authorization_code`

Parameters: `code`, `redirect_uri`, `code_verifier`, and `client_id` (required for public clients;
if present for confidential clients it must equal the authenticated client).

**OAUTH-21.** Steps, in order:

1. Look up SHA-256(`code`) in `code_hashes` under this instance. Not found: `invalid_grant`.
2. Load the auth request. If its status is `redeemed`: this is a **replay**. Append
   `auth_request.code_replayed` and revoke the family created by the first redemption
   (`token_family.revoked`, reason `code_replayed`) in one batch, then fail with `invalid_grant`
   (DM-15).
3. If the code has expired (`now >= code_expires_at` by `Clock`), or the status is not
   `code_issued`: `invalid_grant` (DM-16).
4. If the auth request's `client_id` differs from the authenticated client: `invalid_grant`. The
   code is not consumed (so a stolen code presented by another client does not lock out the
   rightful one), and an observation is recorded.
5. If `redirect_uri` is not byte-identical to the one in the auth request: `invalid_grant`.
6. Verify PKCE (OAUTH-16). On mismatch: `invalid_grant`, and *conservative choice:* the code is
   burned (`auth_request.failed`), because a wrong verifier means someone other than the
   initiator holds the code.
7. Read-check the user, the client, the org, the grant and the session (ES-10); any not active:
   `invalid_grant`.
8. Build the tokens ([08](08-tokens.md), [09](09-oidc.md)) and append the redemption batch from
   [05](05-event-store-and-projections.md) section 3.2. On `Conflict` with another redemption of
   the same code, reload: the loser sees `redeemed` and follows step 2.

### 5.3 `refresh_token`

Parameters: `refresh_token`, optional `scope`, and `client_id` for public clients.

**OAUTH-22.** Steps:

1. Look up SHA-256(`refresh_token`) in `refresh_hashes`. If found, go to step 3.
2. Else look it up in `retired_refresh`. If found: **reuse**. Append `token_family.reuse_detected`
   and `token_family.revoked` (reason `reuse_detected`) and fail with `invalid_grant` (DM-18). If
   not found in either: `invalid_grant`.
3. The family's `client_id` must equal the authenticated client, else `invalid_grant` (without
   revoking: the presenter is not the holder, and revoking would let any client kill any family
   whose token it has seen).
4. The family must be `active` and not past `idle_expires_at` or `absolute_expires_at`.
5. `scope`, if present, must be a subset of the family's scopes, else `invalid_scope`. The new
   access token gets the requested subset; the family's scopes and the new refresh token's scopes
   stay unchanged ([RFC 6749] section 6).
6. Read-check user, client, org, grant and, for a session-bound family, the session (ES-10).
7. Rotate: issue a new refresh token and access token and append `token_family.rotated`
   ([08](08-tokens.md) section 3). The response contains the new `refresh_token`.

## 6. Scopes

**OAUTH-23.** `scope` is a space-separated list ([RFC 6749] section 3.3). Each value matches
`[\x21\x23-\x5B\x5D-\x7E]+`. At most 64 values and 2 KiB in total.

**OAUTH-24.** The server keeps the requested scopes that are both known to the instance and in the
client's `allowed_scopes`, and silently drops the rest. The granted set is always returned in the
token response (OAUTH-20). *Conservative choice:* a requested scope is never granted just because
it is unknown to us; it is dropped.

**OAUTH-25.** If no scope is requested, or none survives OAUTH-24, the request fails with
`invalid_scope`. A request without `openid` is a plain OAuth request: no ID token is issued, and
the userinfo endpoint is not usable with the resulting access token.

Known scopes in Phase 1: `openid`, `profile`, `email`, `offline_access`, and the org-hint scope
`urn:loams:org:<org_id>` (consumed by the server, never placed in tokens). Phase 2 adds `phone`.
Phase 4 adds project role scopes ([14](14-orgs-projects-roles.md)). Resource servers may register
their own scopes `[P3]`.

## 7. Client authentication

**OAUTH-26.** The method is recognized from the request:

| Method | Request carries |
|---|---|
| `client_secret_basic` | `Authorization: Basic` with `client_id:client_secret`, each form-urlencoded before base64 ([RFC 6749] section 2.3.1) |
| `client_secret_post` | `client_id` and `client_secret` in the body |
| `private_key_jwt` `[P3]` | `client_assertion_type=urn:ietf:params:oauth:client-assertion-type:jwt-bearer` and `client_assertion` |
| `none` | `client_id` in the body, nothing else |

**OAUTH-27.** Secret check: the client must be active; compute SHA-256 of the presented secret and
compare in constant time (`subtle`) with each of the client's unexpired stored hashes (at most
two, DM-4). The comparison runs over all stored hashes regardless of an early match.

**OAUTH-28.** `private_key_jwt` `[P3]`: validate the assertion as a "client assertion" per KEY-17
with the audience rule KEY-19, plus: `iss` and `sub` both equal the `client_id`; `exp` is required
and at most 5m after `iat`; `iat` is required; `jti` is required and is recorded with
`MetaStore::put_if_absent` under (instance, client, `jti`) until `exp` plus the skew allowance; an
already-present `jti` is a replay and fails ([RFC 7523] section 3, [OIDC Core] section 9).

**OAUTH-29.** Failure: HTTP `401` with `{"error": "invalid_client"}`. If the request used
`Authorization: Basic`, the response includes `WWW-Authenticate: Basic realm="<issuer>"`
([RFC 6749] section 5.2). The response does not say which part failed (unknown client, wrong
secret, expired secret, wrong method).

**OAUTH-30.** Failed client authentications are rate-limited per (instance, client id) and per
source address ([11](11-login-and-passwords.md) section 5, ADR 0024).

## 8. Error responses at the token endpoint

**OAUTH-31.** Errors are JSON `{"error": "...", "error_description": "..."}` with status `400`,
except `invalid_client` (`401`, OAUTH-29). Codes are exactly those of [RFC 6749] section 5.2:
`invalid_request`, `invalid_client`, `invalid_grant`, `unauthorized_client`,
`unsupported_grant_type`, `invalid_scope`; plus `invalid_target` ([RFC 8707]) and the device-flow
and token-exchange codes in [13](13-machine-identity.md). Internal failures return `500` with
`server_error` and the request id (ARCH-13).

## 9. CORS

**OAUTH-32.** The authorization and logout endpoints are browser navigations and send no CORS
headers. The token, revocation and userinfo endpoints answer CORS for `spa` clients:

- A preflight (`OPTIONS`) succeeds if the `Origin` is the origin of a redirect URI of some active
  `spa` client in this instance (an inline index `spa_origins`). It allows the methods `POST` (and
  `GET` for userinfo) and the headers `Authorization`, `Content-Type` and `DPoP`, with
  `Access-Control-Max-Age: 600`.
- On the actual request, if an `Origin` header is present it must be an origin of the
  authenticated client's (or the token's client's) redirect URIs; otherwise the request fails with
  `invalid_request`. The response then carries `Access-Control-Allow-Origin` with that origin and
  `Vary: Origin`.
- `Access-Control-Allow-Credentials` is never sent: these endpoints never use cookies.
- Introspection sends no CORS headers (server-to-server only). Discovery and JWKS allow `*`.

## 10. Response headers on HTML pages

Error pages, the form-post page, the login UI and consent pages send `X-Frame-Options: DENY` and
`Content-Security-Policy: frame-ancestors 'none'` (clickjacking), `Referrer-Policy: no-referrer`
(so codes and auth request ids do not leak through `Referer`), and
`X-Content-Type-Options: nosniff`.

## 11. Known conflicts with external test suites

**OAUTH-33.** If the OpenID conformance suite's Basic OP plan sends authorization requests without
PKCE for confidential clients, those tests fail under OAUTH-14. We do not relax OAUTH-14 to pass.
The conflict is recorded in `conformance/RESULTS.md` and reported to the owner, who may decide to
allow a per-client exception for confidential clients that send `nonce` (permitted by
[RFC 9700] section 2.1.1). The same applies to client assertions with the token endpoint as
audience (KEY-19).

## Sources

- [RFC 6749]: OAuth 2.0, sections 2.3, 2.3.1, 3.1, 3.1.2, 3.3, 4.1, 4.1.2.1, 5.1, 5.2, 6.
- [draft-ietf-oauth-v2-1]: The OAuth 2.1 Authorization Framework, revision 15 (code-only, PKCE,
  exact redirect matching, no implicit or password grants).
- [RFC 9700]: OAuth 2.0 Security Best Current Practice, sections 2.1.1, 4.1.3.
- [RFC 7636]: PKCE, sections 4.1, 4.3, 4.6.
- [RFC 7523]: JWT profile for client authentication, section 3.
- [RFC 8252]: OAuth 2.0 for Native Apps, sections 7.3, 8.3.
- [RFC 8414]: Authorization Server Metadata.
- [RFC 8707]: Resource Indicators.
- [RFC 9207]: Authorization Server Issuer Identification.
- [OAuth Form Post]: OAuth 2.0 Form Post Response Mode.
- [OIDC Core]: OpenID Connect Core 1.0, sections 3.1.2.1, 9, 11.
- Zitadel public docs, observable behaviour only: a provider of this kind requires an exact
  redirect URI match at the authorization and logout endpoints, accepts `client_secret_basic`,
  `client_secret_post`, `private_key_jwt` and `none` at the token endpoint, and supports
  `query` and `form_post` response modes ([Zitadel endpoints], [Zitadel grant types]). Paths,
  validation order, the binding cookie and the error behaviour are ours.

[RFC 6749]: https://www.rfc-editor.org/rfc/rfc6749
[draft-ietf-oauth-v2-1]: https://datatracker.ietf.org/doc/draft-ietf-oauth-v2-1/15/
[RFC 9700]: https://www.rfc-editor.org/rfc/rfc9700
[RFC 7636]: https://www.rfc-editor.org/rfc/rfc7636
[RFC 7523]: https://www.rfc-editor.org/rfc/rfc7523
[RFC 8252]: https://www.rfc-editor.org/rfc/rfc8252
[RFC 8414]: https://www.rfc-editor.org/rfc/rfc8414
[RFC 8707]: https://www.rfc-editor.org/rfc/rfc8707
[RFC 9207]: https://www.rfc-editor.org/rfc/rfc9207
[OAuth Form Post]: https://openid.net/specs/oauth-v2-form-post-response-mode-1_0.html
[OIDC Core]: https://openid.net/specs/openid-connect-core-1_0.html
[Zitadel endpoints]: https://zitadel.com/docs/apis/openidoauth/endpoints
[Zitadel grant types]: https://zitadel.com/docs/apis/openidoauth/grant-types
