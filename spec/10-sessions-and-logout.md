# 10. Sessions and logout

A **session** is a browser's sign-in at the identity provider. It is what makes single sign-on
work: a second application that sends the same browser to `/oauth/authorize` finds an active
session and does not ask for the password again. This file covers how sessions are created,
grown, used, listed, expired and ended, and how logout reaches relying parties.

A session is not an agent session ([13](13-machine-identity.md) section 9) and not a token
family ([08](08-tokens.md)). The aggregate is defined in [03](03-data-model.md) section 2.8 and its
events in [04](04-event-catalog.md) section 3.8.

## 1. The session cookie

**SES-1.** The session cookie is named `__Host-loams_session`. It is set with the attributes
`Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/` and no `Domain` attribute. The `__Host-` prefix
makes browsers refuse the cookie unless it is `Secure`, has `Path=/` and has no `Domain`
([RFC 6265bis] section 4.1.3.2), so a sibling host cannot plant or overwrite it.

**SES-2.** The cookie value is `lis_` followed by the base64url encoding (no padding) of 32
bytes from `Rng` (ADR 0013). The store keeps only its SHA-256 (`cookie_hash`, DM-4) in the
session aggregate and in the `session_cookies` index (ES-13). A lookup hashes the presented
value and looks up `(instance, hash)`.

**SES-3.** A cookie value that does not parse (wrong prefix, wrong length, bad base64url) is
treated as absent. It is never looked up, and the response clears it.

**SES-4.** The cookie has a `Max-Age` equal to the time left until the session's
`absolute_expires_at`. The browser's expiry is a convenience only; the server decides expiry
from the stored timestamps (ARCH-11).

**SES-5.** In the path issuer form (TEN-1), several instances share one host. The cookie name
then carries the instance slug: `__Host-loams_session_<slug>`. `Path` stays `/`, because the
`__Host-` prefix requires it. Each instance reads only its own cookie name.

**SES-6.** The session cookie is never readable by script, never sent in a URL, and never
logged (ARCH-14). The session id (the aggregate id, not the cookie value) may appear in logs and
in the `sid` claim (section 6).

## 2. Creating a session

A session is created when a user completes the first factor of a login on the login UI
([11](11-login-and-passwords.md) section 2). It is not created by an unauthenticated visit.

**SES-7.** Creating a session appends `session.created` with `user_id`, `cookie_hash`,
`idle_expires_at = now + idle_ttl` and `absolute_expires_at = now + absolute_ttl`, using the
session policy of the user's org (defaults in [03](03-data-model.md) section 3: idle 12h,
absolute 30d). The same batch appends `session.factor_verified` for the factor just checked.

**SES-8.** A new session always gets a fresh cookie value. If the browser already carries a
session cookie for this instance, the old session is ended (`session.ended` with reason
`replaced`) in the same batch as the new one is created. This prevents session fixation: a
value planted before login never becomes an authenticated session ([OWASP Session]).

**SES-9.** *Conservative choice.* In Phases 1 to 4 a browser holds **at most one active session
per instance**. Switching account means signing out and signing in again. `prompt=select_account`
shows the current account with two choices, "continue" and "use another account", where the
second ends the current session first ([09](09-oidc.md)). Supporting several signed-in accounts
in one browser (an account chooser) is a later decision and needs its own ADR, because it
changes what `sid` means, what logout ends and what the login UI shows.

## 3. Factors and assurance

A session accumulates verified factors. Each verified factor appends `session.factor_verified`
with `method` (the factor kind: `password`, `passkey`, `totp`, `email_otp`, `sms_otp`,
`recovery_code`, `federated`), the time, and `details`: for a passkey the credential id and
whether the ceremony performed user verification (`uv: true` or `false`); for a federated factor
the IdP id. `amr` and `acr` are derived from these records ([09](09-oidc.md) section 4), so the
records must carry everything the derivation needs.

**SES-10.** The `amr` values a session can carry are taken from [RFC 8176]:

| Factor | `amr` values added |
|---|---|
| Password | `pwd` |
| TOTP | `otp` |
| Email OTP `[P2]` | `otp` |
| SMS OTP `[P2]`, off by default | `sms` |
| Passkey with user verification | `pop`, `user` (never `hwk` or `swk`: with `none` attestation the server cannot tell hardware from software keys) |
| Passkey without user verification (second factor only) | `pop` |
| Recovery code | `otp` (see MFA-30 in [12](12-passkeys-and-mfa.md)) |
| Upstream IdP `[P5]` | No RFC 8176 value; see [16](16-federation.md) FED-21 |

`mfa` is added when the session holds factors from two different categories (something known,
something held) or one passkey with user verification. The category rules and the full table are
in [12](12-passkeys-and-mfa.md) section 6.

**SES-11.** The session's **assurance level** (`acr`) is computed from its factors, not stored:

| `acr` | Meaning | When |
|---|---|---|
| `urn:loams:aal1` | Single factor | Any one factor |
| `urn:loams:aal2` | Multi-factor | Two factors of different categories, where the second is not a code sent by email or SMS ([12](12-passkeys-and-mfa.md) MFA-1) |
| `urn:loams:aal2:phr` | Phishing-resistant multi-factor | A passkey with user verification was verified in this session (alone, or with a password) |

These names are ours. They follow the levels in [NIST SP 800-63B-4] without claiming
certification against it.

**SES-12.** A factor counts towards a request only if it was verified recently enough for that
request. `auth_time` is the time of the most recent factor that brought the session to its
current level. When an authorization request carries `max_age`, the login UI asks for a factor
again if `now - auth_time > max_age` ([OIDC Core] section 3.1.2.1). When `acr_values` or the
client's `required_acr` asks for a level the session does not have, the login UI asks for the
missing factor (step-up, `[P2]`; [12](12-passkeys-and-mfa.md) section 7 and [09](09-oidc.md)
OIDC-8).

**SES-13.** Sensitive actions on the user's own account (changing the password, adding or
removing a passkey, TOTP or recovery codes, changing the email address, ending other sessions)
require a factor verified within `reauth_for_sensitive` (default 15m). If the newest factor is
older, the UI asks for one before it shows the form. *Conservative choice:* the re-check needs a
factor of the session's current level, so a user at `aal2` re-authenticates with a second factor
too, not only the password.

## 4. Using a session

**SES-14.** On each request that presents a valid cookie, the server loads the session by
`(instance, cookie_hash)` and accepts it only if all of these hold:

1. `state` is `active`;
2. `now < idle_expires_at` and `now < absolute_expires_at`;
3. the user is `active` (DM-13) and the user's org is `active` (TEN-7);
4. the session's `instance_id` is the resolved instance (TEN-15).

Otherwise the request proceeds as unauthenticated, and the response clears the cookie.

**SES-15.** *Touch throttling.* Activity extends the idle expiry: `idle_expires_at = min(now +
idle_ttl, absolute_expires_at)`. To avoid a write per request, the server appends
`session.touched` only when `last_active_at` is more than 5m old. The idle window is therefore
accurate to within 5m, and the absolute expiry never moves.

**SES-16.** A session is used only by the login UI and the authorization endpoint. It is never
a credential for the token endpoint, userinfo or the management API. Those use tokens.

## 5. Expiry, listing and ending

**SES-17.** Expiry is decided by comparing timestamps (ARCH-11). A background sweeper appends
`session.expired` (`kind: idle` or `absolute`) for sessions past their expiry, so the fan-out
(ES-11) revokes session-bound token families and back-channel logout `[P6]` can be sent. A
session past its expiry is refused by SES-14 whether or not the sweeper has run.

**SES-18.** A user can list their own active sessions on the account page: creation time, last
activity, an approximate device description parsed from the stored user agent, and the
approximate location only if the operator enabled IP geolocation (off by default). These fields
are personal (DM-5) and shown only to the user and to org admins with the right to manage users.

**SES-19.** A user can end any one of their sessions, or all except the current one. A third
choice, **"sign out everywhere"**, ends every session including the current one and also revokes
the user's `offline_access` families ([08](08-tokens.md) TOK-15). An org admin can do the same for a
user in their org through the management API. Each ended session appends `session.ended` with the
reason `user` or `admin`; each revoked family appends `token_family.revoked` with reason `user` or
`admin` through the fan-out.

**SES-20.** Ending or expiring a session revokes every token family whose `session_id` is that
session ([03](03-data-model.md) section 2.9; the fan-out in ES-11). Families created with
`offline_access` are not bound to the session ([08](08-tokens.md)) and survive. Introspection of a
session-bound family's tokens reports inactive from the moment the session event is appended,
without waiting for the fan-out (ES-11).

**SES-21.** Deactivating, locking with `admin_unlock_only`, or removing a user ends all their
sessions (DM-14). A voluntary password change offers "sign out other sessions", checked by
default ([11](11-login-and-passwords.md) LOGIN-53). A password reset always ends all the user's
sessions and revokes all their token families, `offline_access` ones included (LOGIN-38).

## 6. The `sid` claim and participants

**SES-22.** Every ID token issued from a session carries `sid`, the session's aggregate id as a
string ([OIDC Back-Channel Logout] section 2.1 and [OIDC RP-Initiated Logout]). Access tokens of
session-bound families carry it too, so resource servers can drop cached state on logout. The
`sid` is not the cookie value and cannot be used to authenticate.

**SES-23.** When an ID token is issued to a client for the first time in a session, the issuing
batch appends `session.participant_added` with the `client_id`. The participants list is what
logout uses to decide which relying parties to notify `[P6]`.

## 7. RP-initiated logout

The end-session endpoint is `/oauth/logout` and is advertised as `end_session_endpoint` in
discovery ([09](09-oidc.md)). It implements [OIDC RP-Initiated Logout].

### 7.1 Parameters

Accepted by `GET` (query) and `POST` (form body). `POST` is accepted without a CSRF token
because the request only starts a flow; the state change happens on the confirmation step or
under the conditions in SES-27.

| Parameter | Handling |
|---|---|
| `id_token_hint` | An ID token this instance issued. Verified for signature and `iss` with the instance's keys in states `pending`, `active` and `retiring`, and also `retired` keys whose private part is not yet destroyed ([06](06-keys-and-jose.md) KEY-8). An expired token is accepted as a hint ([OIDC RP-Initiated Logout] section 2 allows it), but a token with a bad signature or a different issuer is not. A hint only identifies the client and the session; it grants nothing else. |
| `client_id` | The client asking for logout. If both this and `id_token_hint` are present, `client_id` MUST equal the token's `aud` (single audience) or `azp`. |
| `post_logout_redirect_uri` | Where to send the browser afterwards. Honoured only under SES-25. |
| `state` | Echoed on the redirect to `post_logout_redirect_uri`. At most 512 characters. |
| `ui_locales` | Language of any page shown. |
| `logout_hint` | A hint of which user to log out. Used only to pick between the current session and none: if it does not identify the current session's user, it is ignored. |

**SES-24.** The client is **identified** when a valid `id_token_hint` names it, or when
`client_id` names an active client of this instance. Otherwise the request is anonymous.

**SES-25.** `post_logout_redirect_uri` is honoured only when the client is identified and the
value exactly equals (simple string comparison, no normalization) one of that client's
registered `post_logout_redirect_uris` ([03](03-data-model.md) section 2.5). Otherwise the
parameter is ignored and, after logout, the OP shows its own "you are signed out" page. It never
redirects to an unregistered URI. This closes the open-redirect path through logout
([RFC 9700] section 4.11).

**SES-26.** A mismatched `client_id` and `id_token_hint`, or an `id_token_hint` that fails
verification, gets an error page (not a redirect), with the error `invalid_request`.

### 7.2 Confirmation

**SES-27.** The OP ends the session without asking only when all of these hold:

1. a valid `id_token_hint` is present;
2. its `sid` equals the browser's current session id (or, if the token has no `sid`, its `sub`
   equals the session's user); and
3. the request is a top-level navigation (`Sec-Fetch-Mode: navigate` when the browser sends
   it).

In every other case (no hint, a hint for another session, a session already gone) the OP shows a
confirmation page at `/login/logout-confirm` asking "Sign out of {instance name}?" with a form
that posts back with a CSRF token ([11](11-login-and-passwords.md) section 8). This stops a third
party from logging users out by embedding a logout URL (logout CSRF), as [OIDC RP-Initiated Logout]
section 6 recommends asking the user.

**SES-28.** If there is no active session, the OP does not ask. It goes straight to the result of
SES-25 (redirect or the signed-out page).

### 7.3 Effects

**SES-29.** Logout appends `session.ended` with reason `logout`, clears the session cookie
(`Max-Age=0`, same name and attributes), and triggers the fan-out that revokes the session-bound
families (SES-20). If `post_logout_redirect_uri` is honoured, the response is a `303` to it with
`state` appended as a query parameter when given.

**SES-30.** RP-initiated logout does not revoke `offline_access` families ([08](08-tokens.md)
TOK-15). A client that wants those gone uses the revocation endpoint. The user can revoke them by
revoking the grant on the account page or with "sign out everywhere" (SES-19).

## 8. Back-channel logout `[P6]`

[OIDC Back-Channel Logout] lets the OP tell each participant's server that the session ended. The
full delivery rules are in [20](20-later-standards.md). The session side is fixed now so the data
model does not change later:

**SES-31.** When a session ends or expires, the fan-out worker reads its `participants` and, for
each client with a registered `backchannel_logout_uri`, enqueues one delivery through the outbox.
Delivery is at least once with retries; a failure never blocks the logout itself.

**SES-32.** The logout token is a JWT signed with the instance's ID-token key for that client
(same `alg` as the client's ID tokens), with header `typ: logout+jwt` and claims:

| Claim | Value |
|---|---|
| `iss` | The instance issuer |
| `aud` | The client id |
| `iat`, `exp` | Now, and now + 2m |
| `jti` | A fresh UUIDv7 |
| `sub` | The user id |
| `sid` | The session id, when the client registered `backchannel_logout_session_required` or always (we always include it) |
| `events` | `{ "http://schemas.openid.net/event/backchannel-logout": {} }` |

It MUST NOT contain `nonce` ([OIDC Back-Channel Logout] section 2.4).

## 9. What is not implemented

**SES-33.** **OIDC Session Management** (the `check_session_iframe`) and **OIDC Front-Channel
Logout** are not implemented (ADR 0025). Both rely on the relying party's page embedding an OP
page in an iframe that can read the OP's cookie. Browsers now block or partition third-party
cookies (Safari and Firefox by default; Chrome partitions them in iframes for many users), so the
iframe sees no session and the mechanisms fail silently or log users out at random. Discovery
therefore does not advertise `check_session_iframe` or `frontchannel_logout_supported`.
Back-channel logout `[P6]` covers the same need server to server.

## 10. Tests that cover this file

- Session fixation: a cookie set before login is never accepted after login (SES-8).
- Idle and absolute expiry under clock jumps forward and backward in the simulator (SES-14,
  SES-15, SES-17).
- Logout CSRF: an embedded `/oauth/logout` without a valid hint shows the confirmation page and
  ends nothing (SES-27).
- Open redirect on logout: unregistered, near-match (trailing slash, different case, extra query)
  and unidentified-client `post_logout_redirect_uri` values never redirect (SES-25).
- Cross-instance: a session cookie from instance X is ignored by instance Y on a shared host
  (SES-5, SES-14).
- Ending a session revokes bound families and leaves `offline_access` families (SES-20, SES-30).

## Sources

- [RFC 6265bis]: Cookies: HTTP State Management Mechanism (draft), section 4.1.3.2, the
  `__Host-` prefix. Browser behaviour for the prefix is long-standing even though the document is
  still a draft.
- [RFC 8176]: Authentication Method Reference Values.
- [RFC 9700]: OAuth 2.0 Security Best Current Practice, section 4.11 (open redirection).
- [OIDC Core]: OpenID Connect Core 1.0, sections 2 (`acr`, `amr`, `auth_time`) and 3.1.2.1
  (`max_age`, `prompt`).
- [OIDC RP-Initiated Logout]: OpenID Connect RP-Initiated Logout 1.0.
- [OIDC Back-Channel Logout]: OpenID Connect Back-Channel Logout 1.0.
- [OIDC Session Management] and [OIDC Front-Channel Logout], for what is deliberately not built.
- [NIST SP 800-63B-4]: authenticator assurance levels and reauthentication.
- [OWASP Session]: OWASP Session Management Cheat Sheet (fixation, cookie attributes).
- Loams design 19 section 6 (session cookie attributes and 12h idle, 30d absolute defaults), a
  Loams-side contract.
- Zitadel public docs, observable behaviour only: its end-session endpoint accepts
  `id_token_hint`, `client_id`, `post_logout_redirect_uri` (exact match), `state`, `logout_hint`
  and `ui_locales`, checks the redirect only when the client is identified, and requires
  `client_id` and the hint to agree ([Zitadel endpoints]). These follow the OpenID specification;
  the confirmation rule, cookie design and single-session choice are ours.

[RFC 6265bis]: https://datatracker.ietf.org/doc/draft-ietf-httpbis-rfc6265bis/
[RFC 8176]: https://www.rfc-editor.org/rfc/rfc8176
[RFC 9700]: https://www.rfc-editor.org/rfc/rfc9700
[OIDC Core]: https://openid.net/specs/openid-connect-core-1_0.html
[OIDC RP-Initiated Logout]: https://openid.net/specs/openid-connect-rpinitiated-1_0.html
[OIDC Back-Channel Logout]: https://openid.net/specs/openid-connect-backchannel-1_0.html
[OIDC Session Management]: https://openid.net/specs/openid-connect-session-1_0.html
[OIDC Front-Channel Logout]: https://openid.net/specs/openid-connect-frontchannel-1_0.html
[NIST SP 800-63B-4]: https://pages.nist.gov/800-63-4/sp800-63b.html
[OWASP Session]: https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
[Zitadel endpoints]: https://zitadel.com/docs/apis/openidoauth/endpoints
