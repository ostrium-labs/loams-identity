# 09. OpenID Connect: ID tokens, claims, request parameters, discovery, userinfo

This file adds the OpenID Connect layer ([OIDC Core], [OIDC Discovery]) on top of the OAuth
behaviour in [07](07-oauth-core.md). Only the authorization code flow is supported; the implicit
and hybrid flows are not (OAUTH section 3.2 step 5).

## 1. When OIDC applies

**OIDC-1.** A request is an OpenID Connect request when its granted scopes include `openid`. Only
then is an ID token issued, and only then are `nonce`, `prompt`, `max_age`, `id_token_hint`,
`acr_values` and `claims` interpreted. In a plain OAuth request these parameters are ignored,
except `prompt` and `max_age`, which still control re-authentication (they are useful to MCP
clients and harmless).

## 2. ID token

### 2.1 Claims

| Claim | Value | Always |
|---|---|---|
| `iss` | Instance issuer | Yes |
| `sub` | User id (a UUIDv7 string, stable, never reassigned) | Yes |
| `aud` | The client id, as a single string | Yes |
| `exp`, `iat` | NumericDate; `exp = iat + id_token_ttl` (default 15m) | Yes |
| `auth_time` | Time of the user authentication that the session last performed for this request | Yes (*conservative choice:* always present, whether or not `max_age` was sent) |
| `nonce` | Exactly the authorization request's `nonce` | When the request had one |
| `acr` | Section 4 | Yes |
| `amr` | Section 4, a JSON array | Yes |
| `sid` | The browser session id ([OIDC Front-Channel Logout] and [OIDC Back-Channel Logout] define it; we use it for logout, [10](10-sessions-and-logout.md)) | Yes |
| `org` | The user's org id (private claim, [15](15-loams-integration.md)) | Yes |
| Profile and email claims | Section 3 | Only if the client opts in (OIDC-4) |

**OIDC-2.** We do not add `azp` (there is only ever one audience), `at_hash` or `c_hash` (they are
optional in the code flow, [OIDC Core] section 3.1.3.6, and their hash function is unclear for
EdDSA). Relying parties must not require them.

**OIDC-3.** The ID token is signed with the instance's active key for the client's
`id_token_signed_response_alg` (default `ES256`; KEY-1, KEY-14). It is never encrypted in these
phases (`id_token_encryption_alg_values_supported` is not advertised).

**OIDC-4.** *Conservative choice:* by default the ID token carries no profile, email or phone
claims; the client gets them from userinfo, which is where [OIDC Core] section 5.4 places them in
the code flow. A client can set `id_token_profile_claims: true` to also receive, in the ID token,
the claims its granted scopes allow (some relying-party libraries read the profile only from the
ID token). This is a per-client field added to the application in [03](03-data-model.md).

### 2.2 On refresh

**OIDC-5.** A refresh in a family whose scopes include `openid` returns a new ID token
([OIDC Core] section 12.2) with the same `sub`, `aud`, `auth_time`, `acr`, `amr` and `sid` as the
family's, a fresh `iat` and `exp`, and no `nonce`.

## 3. Scopes and claims

| Scope | Claims (userinfo, and the ID token when OIDC-4 opts in) | Phase |
|---|---|---|
| `openid` | `sub` | 1 |
| `profile` | `name` (display name), `given_name`, `family_name`, `preferred_username` (the login name), `locale`, `updated_at` | 1 |
| `email` | `email`, `email_verified` | 1 |
| `phone` | `phone_number` (E.164), `phone_number_verified` | 2 |
| `offline_access` | none; controls refresh-token session binding (TOK-15) | 1 |

- `address` is not supported: we do not store addresses. The scope is dropped (OAUTH-24).
- A claim with no value is omitted, never sent as `null` or an empty string ([OIDC Core] section
  5.3.2).
- `preferred_username` is not a stable identifier; relying parties must key accounts by `iss` and
  `sub` ([OIDC Core] section 5.7). Our docs say so.
- `updated_at` is the time of the last profile event, in seconds.
- Machine users have no profile claims; their userinfo response is `sub`, `org` and `principal`.

**OIDC-6.** The `claims` request parameter ([OIDC Core] section 5.5) is **not supported** in these
phases (`claims_parameter_supported: false`). *Conservative choice:* a request carrying it is not
rejected (other servers may honour it); the parameter is ignored and an observation is recorded.
`acr` requirements are expressed with `acr_values` (section 4).

## 4. Authentication context: `acr` and `amr`

| `acr` value | Meaning | Reached by |
|---|---|---|
| `urn:loams:aal1` | One factor, roughly [NIST SP 800-63B] AAL1 | Password alone; a passkey without user verification alone; any combination whose second factor is an email or SMS code |
| `urn:loams:aal2` | Two factors, roughly AAL2 | Password plus TOTP or a recovery code; password plus a passkey without user verification used as a second factor `[P2]` |
| `urn:loams:aal2:phr` | Two factors with a phishing-resistant authenticator | A passkey with user verification (alone, or with a password) `[P2]` |

Email and SMS codes never raise `acr` above `urn:loams:aal1`, because NIST SP 800-63B-4 does not
allow email for out-of-band authentication and restricts the telephone network
([12](12-passkeys-and-mfa.md) MFA-1). They still satisfy an org's own `mfa: required` policy if
the org allows them, and they still add `otp` (or `sms`) and `mfa` to `amr`. An email code is
never a first factor.

| Factor | `amr` values ([RFC 8176]) |
|---|---|
| Password | `pwd` |
| TOTP, email OTP, recovery code | `otp` |
| SMS OTP `[P2]` | `sms` |
| Passkey | `pop`; plus `user` and `mfa` when the authenticator reported user verification |
| Two or more distinct factors | `mfa` is added |
| Upstream provider `[P5]` | No value by default (`fed` is not an RFC 8176 value). Only when the identity provider is configured with `trust_upstream_mfa: true` do we pass the upstream `amr` values that are registered in RFC 8176 ([16](16-federation.md) FED-21) |

**OIDC-7.** `acr` and `amr` are computed from the factors verified in the session within the
relevant window ([12](12-passkeys-and-mfa.md)), not from what the user has enrolled.

**OIDC-8.** `acr_values` is a space-separated list in order of preference ([OIDC Core] section
3.1.2.1). If the session's current level does not meet the first value we support, the login UI
asks for the missing factor (step-up, `[P2]`). If the user cannot meet any requested value (no
second factor enrolled), the login completes and the ID token reports the `acr` actually
achieved; the relying party must check it, as [OIDC Core] says `acr_values` is voluntary. An
unknown value is skipped. *Conservative addition:* a client may set `required_acr`; that is a
hard requirement enforced by the server (the user must enroll or the request ends with
`access_denied`). This is a per-client field added to the application in
[03](03-data-model.md).

## 5. Request parameters

### 5.1 `prompt`

| Value | Behaviour |
|---|---|
| absent | Use the session if it satisfies `max_age`, `acr_values` and `required_acr`; otherwise ask the user to sign in or step up |
| `none` | Never show UI. No active session: `login_required`. Session exists but the user must sign in again (`max_age`, `id_token_hint` mismatch): `login_required`. Step-up needed: `interaction_required`. Consent needed: `consent_required` |
| `login` | Require a fresh authentication within this request even if a session exists; `auth_time` becomes the new time |
| `consent` | Show the consent page even if a grant covers the scopes, also for `first_party` clients |
| `select_account` | Show the account page ("continue as …" or "use another account"). With one session per browser (Phase 1), choosing another account ends the current session first |
| `create` | Start at registration ([OIDC Prompt Create]). Only when the org allows registration; otherwise ignored and the normal login page is shown |

**OIDC-9.** `none` combined with any other value fails with `invalid_request` ([OIDC Core] section
3.1.2.1). An unknown value fails with `invalid_request`. The errors `login_required`,
`interaction_required`, `consent_required` and `account_selection_required` are redirected
(they come after redirect-URI validation, OAUTH section 3.2).

### 5.2 `max_age`

**OIDC-10.** A non-negative integer of seconds. If `now - auth_time > max_age`, the user must
authenticate again (with `prompt=none`: `login_required`). `max_age=0` means always
re-authenticate, like `prompt=login`. A malformed value fails with `invalid_request`.

### 5.3 `nonce`

**OIDC-11.** Optional in the code flow. At most 512 bytes. Stored with the auth request and copied
unchanged into the ID token of the code redemption. Never logged.

### 5.4 `id_token_hint`

**OIDC-12.** Validated as an `id_token_hint` per KEY-17 (signature with our keys including
retired-but-not-destroyed ones, KEY-8; `iss` must be the instance; `aud` must contain the
requesting client). *Conservative choice:* the hint's `exp` is not checked (a hint is usually
expired, and [OIDC Core] section 3.1.2.1 allows that), but its signature always is. If the current
session's user is not the hint's `sub`, the user must sign in as the hinted user; with
`prompt=none` this is `login_required`. An invalid hint fails with `invalid_request`.

### 5.5 `login_hint`, `ui_locales`, `display`, `claims_locales`

- **OIDC-13.** `login_hint` (at most 256 characters) prefills the login name. It is personal data:
  not logged, stored encrypted with the auth request (EV catalog `auth_request.created`). It never
  selects or reveals an account by itself.
- `ui_locales`: the first tag we have translations for; otherwise the instance default (`en`).
- `display`: only `page` is supported; other values are ignored.
- `claims_locales`: ignored (claims have no language variants).

### 5.6 Unsupported OIDC request features

`request`, `request_uri` (until PAR), `registration` and `claims` are covered in OAUTH section
3.2 step 4 and OIDC-6. The `registration` parameter fails with `registration_not_supported`.

## 6. Pairwise subjects

**OIDC-14.** Only `public` subject identifiers are supported in these phases
(`subject_types_supported: ["public"]`). `sub` is the user id, the same for every client in the
instance. Pairwise identifiers ([OIDC Core] section 8.1) are later work and would be a new ADR.

## 7. Discovery

**OIDC-15.** The metadata document is served at:

- `<issuer>/.well-known/openid-configuration` ([OIDC Discovery] section 4), and
- the [RFC 8414] location: for a host-form issuer `https://host`, at
  `https://host/.well-known/oauth-authorization-server`; for a path-form issuer
  `https://host/i/<slug>`, at `https://host/.well-known/oauth-authorization-server/i/<slug>`
  ([RFC 8414] section 3.1).

Both return the same JSON. The `issuer` value is exactly the instance issuer (TEN-3), and the
document is built from the `discovery_document` projection. Headers: `Content-Type:
application/json`, `Cache-Control: public, max-age=300`, `ETag`, `Access-Control-Allow-Origin: *`.

**OIDC-16.** Fields and values (Phase 1 unless marked):

| Field | Value |
|---|---|
| `issuer` | The instance issuer |
| `authorization_endpoint` | `<issuer>/oauth/authorize` |
| `token_endpoint` | `<issuer>/oauth/token` |
| `userinfo_endpoint` | `<issuer>/oauth/userinfo` |
| `jwks_uri` | `<issuer>/oauth/jwks` |
| `introspection_endpoint` | `<issuer>/oauth/introspect` |
| `revocation_endpoint` | `<issuer>/oauth/revoke` |
| `end_session_endpoint` | `<issuer>/oauth/logout` `[P2]` |
| `device_authorization_endpoint` | `<issuer>/oauth/device_authorization` `[P3]` |
| `scopes_supported` | `["openid", "profile", "email", "offline_access"]`, plus `"phone"` `[P2]` |
| `response_types_supported` | `["code"]` |
| `response_modes_supported` | `["query", "form_post"]` |
| `grant_types_supported` | `["authorization_code", "refresh_token"]`, plus `client_credentials`, JWT bearer, device code and token exchange URNs `[P3]` |
| `subject_types_supported` | `["public"]` |
| `id_token_signing_alg_values_supported` | `["ES256", "EdDSA", "RS256"]` (RS256 is required by [OIDC Discovery] section 3) |
| `acr_values_supported` | `["urn:loams:aal1", "urn:loams:aal2", "urn:loams:aal2:phr"]` |
| `token_endpoint_auth_methods_supported` | `["client_secret_basic", "client_secret_post", "none"]`, plus `"private_key_jwt"` `[P3]` |
| `token_endpoint_auth_signing_alg_values_supported` | `["EdDSA", "ES256", "ES384", "RS256", "PS256"]` `[P3]` |
| `introspection_endpoint_auth_methods_supported` | `["client_secret_basic", "client_secret_post"]`, plus `"private_key_jwt"` `[P3]` |
| `revocation_endpoint_auth_methods_supported` | `["client_secret_basic", "client_secret_post", "none"]`, plus `"private_key_jwt"` `[P3]` |
| `code_challenge_methods_supported` | `["S256"]` |
| `claims_supported` | `["sub", "iss", "aud", "exp", "iat", "auth_time", "nonce", "acr", "amr", "sid", "org", "name", "given_name", "family_name", "preferred_username", "locale", "updated_at", "email", "email_verified"]`, plus `phone_number` and `phone_number_verified` `[P2]` |
| `claim_types_supported` | `["normal"]` |
| `claims_parameter_supported` | `false` |
| `request_parameter_supported` | `false` |
| `request_uri_parameter_supported` | `false` (stated explicitly: its default in [OIDC Discovery] is `true`) |
| `display_values_supported` | `["page"]` |
| `ui_locales_supported` | The locales the login UI has |
| `prompt_values_supported` | `["none", "login", "consent", "select_account"]`, plus `"create"` when registration is enabled ([OIDC Prompt Create]) |
| `authorization_response_iss_parameter_supported` | `true` ([RFC 9207]) |
| `frontchannel_logout_supported` | `false` |
| `backchannel_logout_supported` | `false` until `[P6]` ([20](20-later-standards.md)) |
| `service_documentation`, `op_policy_uri`, `op_tos_uri` | From instance settings, omitted when unset |

Not advertised: `registration_endpoint` (until [20](20-later-standards.md)),
`check_session_iframe` (ADR 0025), encryption algorithms, `userinfo_signing_alg_values_supported`
(userinfo is plain JSON), `pushed_authorization_request_endpoint` and
`dpop_signing_alg_values_supported` (later).

**OIDC-17.** Field order in the JSON is fixed (sorted by name, ARCH-9). Arrays keep the order
given above.

## 8. UserInfo

**OIDC-18.** `GET` and `POST` on `/oauth/userinfo` ([OIDC Core] section 5.3).

**OIDC-19.** The access token is taken from `Authorization: Bearer`, or, for `POST` with
`application/x-www-form-urlencoded`, from the `access_token` form field ([RFC 6750] section 2.2).
A request with both, or with the token in the query string, fails with `invalid_request` (`400`).

**OIDC-20.** The token must be one of our access tokens (JWT per KEY-17, or opaque by index) and be
active by TOK-20.

**OIDC-21.** The token's scopes must include `openid`, and its `aud` must contain the instance
issuer (TOK-5); otherwise `403` with `error="insufficient_scope"`.

**OIDC-22.** The response is `application/json`, unsigned, with `sub` (equal to the ID token's
`sub`), `org`, and the claims allowed by the token's scopes (section 3). It is built from the
user aggregate (decrypting personal fields with the user's data key, DM-6), not from a projection
(ES-14). Headers: `Cache-Control: no-store`, CORS for `spa` clients (OAUTH-32). Errors follow
TOK-8.

## 9. Conformance targets

**OIDC-23.** This file, with [06](06-keys-and-jose.md) to [08](08-tokens.md), is written to pass
these OpenID Foundation certification profiles ([OIDF certification]):

| Profile | Phase | Notes |
|---|---|---|
| Basic OP | 1 | Code flow. The conformance clients use the opt-in PKCE exception (OAUTH-14a, OAUTH-33) |
| Config OP | 1 | Discovery and JWKS |
| Form Post OP | 1 | `response_mode=form_post` with the code flow |
| RP-Initiated Logout OP | 2 | [10](10-sessions-and-logout.md) |
| Back-Channel Logout OP | 6 | [20](20-later-standards.md) |
| Dynamic OP | later | Needs dynamic registration ([20](20-later-standards.md)) |
| Implicit OP, Hybrid OP | never | Those flows are not supported by design |

Results go in `conformance/RESULTS.md` ([19](19-verification.md)).

## Sources

- [OIDC Core]: OpenID Connect Core 1.0 incorporating errata set 2, sections 2, 3.1.2.1, 3.1.2.6,
  3.1.3.6, 5.3, 5.4, 5.5, 5.7, 8.1, 11, 12.2.
- [OIDC Discovery]: OpenID Connect Discovery 1.0, sections 3 and 4.
- [OIDC Prompt Create]: Initiating User Registration via OpenID Connect 1.0.
- [OIDC Front-Channel Logout] and [OIDC Back-Channel Logout], for the `sid` claim.
- [RFC 6750]: Bearer Token Usage, section 2.2.
- [RFC 8176]: Authentication Method Reference Values.
- [RFC 8414]: Authorization Server Metadata, section 3.1.
- [RFC 9207]: Authorization Server Issuer Identification.
- [NIST SP 800-63B]: Digital Identity Guidelines, authenticator assurance levels.
- [OIDF certification]: OpenID Foundation certification profiles.
- Zitadel public docs, observable behaviour only: a provider of this kind supports the standard
  scopes, `offline_access` for refresh tokens in the code flow, the `prompt` values `none`,
  `login`, `select_account` and `create`, and reports `amr` per RFC 8176 ([Zitadel scopes],
  [Zitadel claims], [Zitadel endpoints]). Our claim names, the `acr` values, the ID-token claim
  policy and the discovery contents are ours; we use none of that product's claim or scope names.

[OIDC Core]: https://openid.net/specs/openid-connect-core-1_0.html
[OIDC Discovery]: https://openid.net/specs/openid-connect-discovery-1_0.html
[OIDC Prompt Create]: https://openid.net/specs/openid-connect-prompt-create-1_0.html
[OIDC Front-Channel Logout]: https://openid.net/specs/openid-connect-frontchannel-1_0.html
[OIDC Back-Channel Logout]: https://openid.net/specs/openid-connect-backchannel-1_0.html
[RFC 6750]: https://www.rfc-editor.org/rfc/rfc6750
[RFC 8176]: https://www.rfc-editor.org/rfc/rfc8176
[RFC 8414]: https://www.rfc-editor.org/rfc/rfc8414
[RFC 9207]: https://www.rfc-editor.org/rfc/rfc9207
[NIST SP 800-63B]: https://pages.nist.gov/800-63-4/sp800-63b.html
[OIDF certification]: https://openid.net/certification/
[Zitadel scopes]: https://zitadel.com/docs/apis/openidoauth/scopes
[Zitadel claims]: https://zitadel.com/docs/apis/openidoauth/claims
[Zitadel endpoints]: https://zitadel.com/docs/apis/openidoauth/endpoints
