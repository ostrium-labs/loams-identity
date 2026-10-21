# 11. Login UI and passwords

The login UI is the set of server-rendered pages a person uses to sign in, consent, recover an
account and manage their own credentials. This file covers the pages, the flow state machine,
passwords, lockout, rate limits, anti-enumeration, CSRF, headers and first-run setup.

Passkeys, TOTP, email OTP and recovery codes are in [12](12-passkeys-and-mfa.md). Sessions and
logout are in [10](10-sessions-and-logout.md). Upstream identity providers are in
[16](16-federation.md).

## 1. Technology and page rules

**LOGIN-1.** Pages are rendered on the server with `askama` templates (ADR 0019), with
auto-escaping on for every template. No template marks user-controlled data as safe.

**LOGIN-2.** Every page works without JavaScript, except the WebAuthn ceremony, which needs the
browser's `navigator.credentials` API. Script is served as static files from the identity
provider's own origin; no inline script, no third-party script, no inline event handlers.

**LOGIN-3.** Every form that changes state uses `POST` and carries a CSRF token (section 6).
`GET` requests never change state, except the steps the protocol defines as `GET`
(`/oauth/authorize`, `/oauth/logout` under SES-27, and following an emailed link, which only
shows a form).

**LOGIN-4.** Pages never put a secret in a URL, except the one-time tokens of emailed links
(section 7), which are consumed by the form they show and are useless afterwards.

## 2. Pages

| Path | Purpose | Phase |
|---|---|---|
| `/login` | Identify: login name field, "sign in with a passkey" button, upstream IdP buttons | 1 (passkey 2, IdPs 5) |
| `/login/password` | Enter the password | 1 |
| `/login/passkey` | Passkey ceremony (conditional and modal) | 2 |
| `/login/mfa` | Second factor: choose and verify TOTP, email OTP, passkey, recovery code | 2 |
| `/login/mfa/setup` | Enrol a second factor when the policy requires one and the user has none | 2 |
| `/login/password/change` | Forced password change (`must_change`) | 1 |
| `/login/consent` | Consent to a client's requested scopes | 1 |
| `/login/select-account` | Continue as the current user or use another account (SES-9) | 1 |
| `/login/register` | Self-registration (off by default) | 1 |
| `/login/verify-email` | Enter or confirm an email verification | 1 |
| `/login/reset` | Request a password reset; `/login/reset/confirm` sets the new password | 1 |
| `/login/init` | Set the first credential for an invited user | 1 |
| `/login/logout-confirm` | Logout confirmation (SES-27) | 2 |
| `/login/error` | Error page for errors that cannot go back to the client | 1 |
| `/login/signed-out` | Signed-out page | 2 |
| `/account` | The user's own account: profile, password, sessions; credentials from Phase 2 | 1 |
| `/device` | Device-flow user-code entry ([13](13-machine-identity.md)) | 3 |
| `/setup` | First-run setup (section 9) | 1 |
| `/static/...` | Versioned CSS, script and image files | 1 |

## 3. The login flow

A login is either **attached** to an `AuthRequest` ([03](03-data-model.md) sections 2.7 and 4.3),
when it starts from `/oauth/authorize`, or **direct**, when someone opens `/login` or `/account`.
A direct login ends on `/account`.

### 3.1 Binding the flow to the browser

**LOGIN-5.** When `/oauth/authorize` creates an `AuthRequest` that needs the UI, it sets the
browser-binding cookie `__Host-loams_ar` defined in [07](07-oauth-core.md) OAUTH-7 (an HMAC of
the auth request id under an instance key, `SameSite=Lax`, 10m) and redirects (`303`) to
`/login?ar=<auth request id>` (OAUTH-6). `SameSite=Lax` is needed because the browser arrives
from the client's site.

**LOGIN-6.** Every login page that receives `ar`, on every step including each `POST`, loads the
`AuthRequest` and proceeds only if the request belongs to the resolved instance (TEN-14), is not
terminal or expired, and the browser's `__Host-loams_ar` cookie equals the HMAC of that id
(compared in constant time). Otherwise the page shows `/login/error` with "This sign-in link has
expired. Go back to the application and try again." No detail says which check failed.

This binding stops an attacker from sending a victim a link to the attacker's own half-finished
authorization request (login CSRF and session swapping, [RFC 9700] section 4.4).

**LOGIN-7.** The cookie binds one auth request at a time. If a second tab starts another
authorization, the cookie now names the newer request, and the older tab shows the expired-link
page on its next step; the user restarts there. This is accepted as the cost of the simple
binding. A **direct** login (no `ar`) is bound instead by the pre-session cookie
`__Host-loams_csrf` (LOGIN-43).

The **browser-binding value** used elsewhere in the login UI (CSRF anchor, passkey ceremony
state, upstream IdP intents) is the `__Host-loams_ar` cookie value for attached logins and the
`__Host-loams_csrf` cookie value for direct ones.

### 3.2 States

The UI moves an attached `AuthRequest` through these steps. Each arrow is a page submit.

```text
 /oauth/authorize ─► [session usable and enough?] ─yes─► consent needed? ─no─► issue code, redirect
        │ no                                                    │ yes
        ▼                                                       ▼
   identify ─► password ─► [mfa needed?] ─yes─► mfa ─┐     /login/consent ─► issue code, redirect
        │          │            │ no                  │
        │          │            ▼                     ▼
        │          └─► [must change?] ─► change ─► (back to the consent check)
        └─► passkey (UV) ─────────────────────────────┘
```

**LOGIN-8.** "Session usable and enough" means a session that passes SES-14 and meets the
request's `prompt`, `max_age`, `acr_values` and the client's `required_acr` ([09](09-oidc.md)
OIDC-8, section 5, and SES-12). `prompt=login`
always goes to identify, keeping the login name of the current user pre-filled.
`prompt=none` never shows a page: if the session is not usable or consent is missing, the
request fails at once with `login_required`, `consent_required` or `interaction_required`
([OIDC Core] section 3.1.2.6).

**LOGIN-9.** When a factor is verified, the handler appends `session.factor_verified` (and
`session.created` if needed) and `auth_request.authenticated` in one batch. The `AuthRequest`
then records `user_id`, `session_id`, `auth_time`, `amr` and `acr`.

**LOGIN-10.** Before the code is issued, the user's org must be allowed for the client (TEN-16).
If it is not, the request ends with `access_denied`, sent to the client after the same pages and
the same delay class as a user who cancels ([07](07-oauth-core.md) OAUTH-9).

**LOGIN-11.** "Cancel" on any page ends an attached request with `access_denied` and redirects
to the client's validated `redirect_uri` with `error=access_denied` and `state`.

### 3.3 Identify

**LOGIN-12.** The identify page takes a login name (or an email address, see LOGIN-14). On
submit it normalizes the input (NFC, trim, lower case for the lookup), looks it up, and always
continues to the next page, whether or not a user was found. The next page is:

- `/login/passkey` when the org's policy is `passkey_only`;
- otherwise `/login/password`, which also offers "use a passkey instead" when passkeys are
  allowed ([12](12-passkeys-and-mfa.md)).

The choice depends only on policy, never on the user's own credentials, so the page sequence
does not reveal whether an account exists or what credentials it has. *Conservative choice:* a
user who has no password and only passkeys still sees the password page first and uses the
passkey link. The "sign in with a passkey" button on `/login` (discoverable credentials, no login
name needed) is the fast path for them.

**LOGIN-13.** The org used for policy is resolved as in [02](02-tenancy.md) section 4. A login
name ending in a verified org domain selects that org. Org domains are public, so this reveals
nothing about users.

**LOGIN-14.** Signing in with an email address is allowed when the instance's
`verified_emails` index ([05](05-event-store-and-projections.md) section 4) maps it to exactly
one active user in the resolved org (or the instance, if no org is named). If it maps to none or
several, the flow continues as for an unknown user. Phone numbers are not accepted as login
names.

### 3.4 Password

**LOGIN-15.** The password page shows the login name typed, a password field with
`autocomplete="current-password"`, a "forgot password" link, and nothing else that depends on
the account. Paste and password managers MUST work: no `autocomplete="off"`, no paste blocking
([NIST SP 800-63B-4] section 3.1.1.2).

**LOGIN-16.** On submit:

1. Apply the rate limits (section 5). If a limit is hit, respond with the rate-limit page.
2. If the user was not found, or is not `active`, or the password factor is locked: run one
   argon2id verification against the **dummy hash** (LOGIN-27) and then fail.
3. Otherwise verify the password against the stored PHC string.
4. On failure: increment the counters (section 4), append the observation
   `observed.password_check_failed`, and show the password page again with the message
   "The login name or password is not correct." The message is the same for every failure
   reason in steps 2 and 4.
5. On success: reset the counters, rehash if the parameters changed (LOGIN-26), append the
   factor events (LOGIN-9), and continue.

**LOGIN-17.** The time taken by step 2 and by a failing step 3 MUST be statistically the same.
The test in [19](19-verification.md) measures both distributions over many runs and fails on a
significant difference.

**LOGIN-18.** A user whose `must_change` flag is set is sent to `/login/password/change` after a
successful password check, before any further step. The new password follows section 4 rules and
cannot equal the current one.

**LOGIN-53.** A voluntary password change from `/account` (re-authentication first, SES-13)
offers "sign out other sessions", checked by default. When checked, every other session of the
user ends (`session.ended`, reason `user`) and the families bound to them are revoked (TOK-14);
`offline_access` families are left alone unless the user also picks "sign out everywhere"
([10](10-sessions-and-logout.md) SES-19). A change never ends the session it was made in.

### 3.5 Consent

**LOGIN-19.** Consent is needed when the client is not `first_party` and the user has no active
grant for the client whose scopes cover the requested ones, or when `prompt=consent` is present.
The consent page shows the client's name, its org's name if different from the user's, the
requested scopes in plain words (one line per scope, from the message catalogue), and two
buttons, "Allow" and "Cancel".

**LOGIN-20.** "Allow" appends `auth_request.consented` and creates or widens the grant in the
code-redemption batch ([05](05-event-store-and-projections.md) section 3.2). "Cancel" ends the
request with `access_denied` (LOGIN-11). Consent is recorded only for scopes the client is
allowed to request ([07](07-oauth-core.md)).

### 3.6 Errors

**LOGIN-21.** Errors are sent back to the client only when the client and `redirect_uri` have
been validated ([RFC 6749] section 4.1.2.1). Otherwise `/login/error` shows a short message and a
request id. The page never echoes request parameters.

## 4. Passwords

### 4.1 Policy

**LOGIN-22.** Defaults (org settings may change them within the bounds below):

| Setting | Default | Bounds |
|---|---|---|
| `min_length` | 15 | At least 15 when the password may be the only factor; at least 8 when the org's policy is `mfa: required` ([NIST SP 800-63B-4] section 3.1.1.2) |
| `max_length` | 128 | At least 64; at most 256 |
| Composition rules | None | None may be configured (NIST: composition rules SHALL NOT be imposed) |
| Blocklist | On | Cannot be turned off |
| `check_breached` | Off | On or off |
| Expiry | None | Periodic expiry cannot be configured; a forced change follows evidence of compromise only |

The `min_length` default in [03](03-data-model.md) section 3 is 15, the single-factor minimum in
NIST SP 800-63B-4.

**LOGIN-23.** Before hashing and before length checks, the password is prepared with the
OpaqueString profile of [RFC 8265] (map non-ASCII spaces to U+0020, then Unicode NFC). This
meets NIST's NFC recommendation. Length is counted in Unicode code points after preparation.
Input larger than 1024 bytes is rejected before preparation, to bound the work. A password that
fails the profile's rules (for example one containing control characters) is rejected with "This
password contains characters that cannot be used."

**LOGIN-24.** A new password is rejected if it is, compared whole (not substrings), equal after
lower-casing to:

- an entry of the bundled blocklist of common and compromised passwords (at least the 100,000
  most common entries of an openly licensed list; the list's licence is recorded in
  `PROVENANCE.md` before it is bundled);
- the login name, the email address or its local part, the org name or the instance name;
- the current password.

The message names the reason in general terms ("This password is too common", "This password
contains your account name") without showing the list entry.

**LOGIN-25.** With `check_breached` on, a new password is also checked against a breached-password
range API using k-anonymity: the server sends only the first 5 hexadecimal characters of the
password's SHA-1 and compares the returned suffixes locally ([HIBP range]). The call goes through
`HttpClient` with a 2s timeout. If the call fails, the password is accepted and the failure is
logged; the check does not fail closed, because an outage of a third party should not block
password changes.

### 4.2 Hashing

**LOGIN-26.** Passwords are hashed with argon2id through the `argon2` crate (ADR 0009) with a
16-byte salt from `Rng`, and stored as a PHC string. Two parameter profiles exist:

| Profile | Memory | Iterations | Lanes | Use |
|---|---|---|---|---|
| `standard` (default) | 64 MiB (65536 KiB) | 3 | 4 | [RFC 9106] section 4, second recommended option |
| `small` | 19 MiB (19456 KiB) | 2 | 1 | Small hosts; the [OWASP Password Storage] minimum |

The profile is a process setting. After a successful verification, if the stored PHC string's
parameters differ from the active profile, the password is rehashed and `user.password_set` is
appended with the new PHC string (same password, `must_change` unchanged).

**LOGIN-27.** At startup the process computes one dummy PHC string by hashing a random 32-byte
value from `Rng` with the active profile. Every "no such user" path verifies against it
(LOGIN-16 step 2).

**LOGIN-28.** A semaphore bounds concurrent argon2 computations to the number of CPU cores
(configurable). A request that would wait longer than 2s for a permit gets the rate-limit page
(`503` with `Retry-After: 5`), and the wait counts as an attempt for rate limiting. Hashing runs
on a blocking thread pool, not the async executor. This caps memory at cores × 64 MiB and keeps
the server responsive under a flood of logins.

**LOGIN-29.** Password bytes live in `secrecy` wrappers and are zeroized after hashing
(ARCH-14).

## 5. Rate limits

Rate limits are checked before any credential work. They use `governor` in process for the
per-node burst limit and `MetaStore` ephemeral counters (ES-8) for limits that must hold across
nodes (ADR 0024).

**LOGIN-30.** Defaults:

| Key | Limit | Applies to |
|---|---|---|
| Client IP | 20 requests per minute, burst 10 | Every `POST` to `/login*`, `/device`, `/setup` |
| Login name (HMAC of the normalized name, known or unknown) | 10 attempts per 15 minutes | Password and OTP submits |
| Client IP | 5 per hour | Reset requests, registration, email verification resends |
| Email address (HMAC) | 3 per hour | Reset and verification emails sent |
| Instance | Configurable, off by default | All login `POST`s (emergency brake) |

The client IP is the TCP peer address, or the address from a forwarding header only when the
peer is in the configured trusted-proxy list.

**LOGIN-31.** A request over a limit gets HTTP `429` with `Retry-After` and the page "Too many
attempts. Try again in N minutes." The same page is used whether the key belongs to a real
account or not. The event `observed.rate_limited` is appended with the limit's name.

## 6. Lockout

**LOGIN-32.** Failures are counted per user and factor in `MetaStore` ephemeral counters:

- `password_failures`: consecutive failed password checks, reset on a successful password check;
- `otp_failures`: consecutive failed OTP checks (TOTP, email OTP, recovery codes), reset on a
  successful one;
- `consecutive_failures_total`: all failed checks of the factor since the last success, kept
  for 90 days.

For unknown login names, the same counters exist under the HMAC of the name, so the responses
match those of real accounts.

**LOGIN-33.** Defaults from the lockout policy ([03](03-data-model.md) section 3):

1. From the 4th consecutive failure on, each further check of that factor waits `2^(n-4)`
   seconds (capped at 60s) after the previous failure before it is evaluated. A submit that
   arrives sooner gets the rate-limit page with the remaining time. This slows guessing without
   locking anyone out.
2. At `password_failure_threshold` (10) or `otp_failure_threshold` (5) consecutive failures, the
   factor is locked for `lock_duration` (15m): `user.locked` with that factor and `until`.
3. At 100 consecutive failures of a factor without a success, the factor is locked until an admin
   unlocks it or the user completes a password reset (for the password factor):
   `user.locked` with `until: null`. This is the limit of [NIST SP 800-63B-4] section 3.2.2.
4. With `admin_unlock_only: true`, step 2 locks every factor until an admin unlocks (DM-13).

**LOGIN-34.** While a factor is locked, its check behaves like a failure (LOGIN-16 step 2),
including the dummy hash, and shows the same message. Locking does not change what the login page
shows. When a factor locks, the user is told by email (if they have a verified address) with the
time and approximate place of the attempts and a reset link. The email goes to the account
holder, not to the person typing, so it reveals nothing to an attacker.

**LOGIN-35.** A locked password does not block a passkey sign-in (DM-13), so an attacker who
locks a victim's password does not lock the victim out when the victim has a passkey. Lock
expiry is decided by comparing `until` with `Clock` time; `user.unlocked` with `by: expiry` is
appended lazily at the next successful check.

## 7. Account recovery and verification

### 7.1 Password reset

**LOGIN-36.** `/login/reset` takes a login name or email address. It always answers "If an
account exists for this name, we have sent an email with a link." When an active human user with a
verified email matches, the server appends `user.password_reset_requested` with the SHA-256 of a
reset token and `expires_at = now + 30m`, and sends the link `/login/reset/confirm?t=<token>`
through `Mailer`. The token is `lrs_` plus base64url of 32 bytes from `Rng`. A new request
replaces any earlier token.

**LOGIN-37.** The confirm page shows a new-password form. On submit the token is checked
(hash match, not expired, not used, user still active) and the new password must pass the policy
(section 4.1). The token is used once: the batch appends `user.password_set` and the user's
stream version guards against a second use.

**LOGIN-38.** A successful reset ends every session of the user (`session.ended`, reason
`user`), revokes all the user's token families including `offline_access` ones
(`token_family.revoked`, reason `user`, through the fan-out; [08](08-tokens.md)), clears the
password lockout counters and an indefinite password lock, and sends a notification email. It does **not** sign the user in. The user then signs in with the new
password and, if required, the second factor, so a reset by someone who controls the mailbox
does not bypass MFA.

### 7.2 Email verification and invitations

**LOGIN-39.** Email verification uses a link token `lve_` plus base64url of 32 bytes, valid for
24h, stored hashed on the user aggregate and used once; it appends `user.email_verified`. The
page also accepts an 8-digit code for cross-device use, valid for 10 minutes, limited to 5 tries
(the OTP counters).

**LOGIN-40.** An admin-created human user without a credential starts in state `initial`
([03](03-data-model.md) section 4.1). The invitation email carries a link to `/login/init` with a
token of the reset kind, valid for 72h. Setting the first password (or registering a passkey,
Phase 2) moves the user to `active` and, since the link proves control of the address, also
appends `user.email_verified`.

### 7.3 Self-registration

**LOGIN-41.** Self-registration is off by default (`allow_registration: false`). When on, the
form asks for an email address, a display name and a password (or a passkey, Phase 2), plus
acceptance of the org's terms links when configured. The login name is the email address.

**LOGIN-42.** Registration always answers "Check your email to finish creating your account."
If the address is new, the email holds a verification link and the user is created in `active`
state with `email.verified: false`; the sign-in completes only after verification. If the address
already belongs to an account, the email says so and links to sign-in and reset instead. The
person at the form cannot tell which happened.

## 8. CSRF, cookies and headers

**LOGIN-43.** Every state-changing `POST` carries a hidden `csrf` field. Its value is
`HMAC-SHA-256(instance CSRF key, anchor || form name)`, base64url, where the anchor is the session
id if a session exists, otherwise the browser-binding value (section 3.1). The pre-session cookie
`__Host-loams_csrf` holds 32 random bytes, is set on the first `GET` of any login page, and uses
`SameSite=Strict`, because it is only ever needed on same-site form posts. The server
recomputes and compares in constant time ([OWASP CSRF]).

**LOGIN-44.** In addition, every `POST` is refused with `403` when the `Origin` header is
present and is not the instance's issuer origin, or when `Sec-Fetch-Site` is present and is
not `same-origin` (or `none`, for a user-typed navigation). A `POST` with neither header and no
valid token is refused.

**LOGIN-45.** Response headers on every login page:

| Header | Value |
|---|---|
| `Content-Security-Policy` | `default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; form-action 'self' <origins>; frame-ancestors 'none'; base-uri 'none'` |
| `X-Frame-Options` | `DENY` |
| `Referrer-Policy` | `no-referrer` |
| `Cache-Control` | `no-store` |
| `X-Content-Type-Options` | `nosniff` |
| `Cross-Origin-Opener-Policy` | `same-origin` |
| `Permissions-Policy` | `publickey-credentials-get=(self), publickey-credentials-create=(self)` and every other feature off |
| `Strict-Transport-Security` | `max-age=31536000` (when served over TLS directly; otherwise set by the front proxy) |

`<origins>` is empty except on pages whose submit can end in a redirect to the client: there it
is the origin of the validated `redirect_uri`, because browsers apply `form-action` to the
redirects that follow a form submission. The `form_post` response page ([09](09-oidc.md))
likewise allows only that origin.

**LOGIN-46.** Every cookie the login UI sets uses the `__Host-` prefix, `Secure`, `HttpOnly`,
`Path=/` and no `Domain`. The session cookie and `__Host-loams_ar` use `SameSite=Lax`, because
they must arrive on navigations from a client's or an upstream IdP's site; `__Host-loams_csrf`
uses `SameSite=Strict`. No cookie holds a user id, an email address or any personal data.

## 9. Internationalization and branding

**LOGIN-47.** The page language is the first supported tag in `ui_locales`, then in
`Accept-Language`, then the instance default (`en`). Every visible string comes from a message
catalogue keyed by message id; templates contain no literal user-facing text. The catalogue
format is the implementer's choice (ADR required); Project Fluent's format is suggested.

**LOGIN-48.** Org branding is limited to a logo (PNG or WebP, at most 256 KiB, served from the
identity provider's own origin with `X-Content-Type-Options: nosniff`), a primary colour and a
background colour (six-digit hex values), each with an optional dark-mode variant, and the org's
display name. *Conservative choice:* no custom CSS, HTML, fonts or script, and no SVG logos, so
branding cannot inject content. The branding used is the client's org's before the user is
identified and the user's org's afterwards.

## 10. First-run setup

**LOGIN-49.** On start, if the store holds no instance, the process creates a setup token: 32
bytes from `Rng`, written base64url-encoded to `<data_dir>/setup-token` with file mode `0600`.
Only its SHA-256 and its expiry (now + 1h) are kept, in the `MetaStore`. The log states the
file's path, never its content. The token is printed to the terminal only when standard error is
an interactive TTY. An operator may supply their own with `--setup-token-file`. A restart before
setup replaces the token.

**LOGIN-50.** While no instance exists, `/setup` is served on every host and every other path
answers `503` "This server is not set up yet." Once an instance exists, `/setup` answers `404`
whatever the token.

**LOGIN-51.** The setup form asks for the token, the instance's public URL (pre-filled from
configuration), the org name, and the first owner's login name, email address and password. On
submit the token is compared in constant time with the stored hash and its expiry checked. One
append batch then creates the instance, its first signing keys, the default org, the `default`
project and the owner user with the `owner` org role (`instance.created`, `org.created`,
`project.created`, `user.created`, `user.password_set`, `org.member_added`,
`instance.default_org_set`), and claims the global index key `setup:done`. Two racing requests
cannot both succeed, because only one claim of `setup:done` commits (ES-3). The token's hash is
deleted after the commit.

**LOGIN-52.** The `cli` command `loams-identity setup` does the same non-interactively, reading
the same fields from flags and the password from a file or standard input. It needs access to the
data directory and does not use the token.

## 11. Tests that cover this file

- Enumeration: identical status, headers, page body and timing for unknown user, wrong password,
  locked factor and deactivated user (LOGIN-12, LOGIN-16, LOGIN-17, LOGIN-34).
- Login CSRF: a flow started in one browser cannot be completed in another (LOGIN-6).
- CSRF: missing, wrong-anchor and cross-origin tokens are refused on every `POST` (LOGIN-43,
  LOGIN-44).
- Lockout progression under the simulator's clock, including clock jumps (LOGIN-33, LOGIN-35).
- Reset token single use under concurrent submits (LOGIN-37).
- Setup race: two concurrent setups, one wins (LOGIN-51).
- Header presence on every page, checked by a crawler test over the page list (LOGIN-45).

## Sources

- [RFC 6749]: OAuth 2.0, section 4.1.2.1 (when errors go back to the client).
- [RFC 8265]: PRECIS profiles, the OpaqueString profile for passwords.
- [RFC 9106]: Argon2, section 4 (recommended parameters).
- [RFC 9700]: OAuth 2.0 Security Best Current Practice, section 4.4 (login CSRF, mix-up of
  sessions).
- [OIDC Core]: OpenID Connect Core 1.0, sections 3.1.2.1 and 3.1.2.6 (`prompt`, errors).
- [NIST SP 800-63B-4]: sections 3.1.1 (password length, composition, blocklist, NFC, paste) and
  3.2.2 (rate limiting, 100 consecutive failures).
- [OWASP Password Storage], [OWASP Authentication], [OWASP CSRF] cheat sheets.
- [HIBP range]: the k-anonymity range API design for breached-password checks.
- Crate documentation: [argon2], [askama], [governor].
- Loams design 19 section 6 (the setup token's generation, file delivery, expiry and single use),
  a Loams-side contract.
- Zitadel public docs, observable behaviour only: that a product of this kind offers per-org
  login settings (allow password, allow registration, force MFA), password complexity and
  lockout settings where 0 disables lockout and locked accounts need an admin unlock, and an
  "ignore unknown usernames" option ([Zitadel default settings]). The defaults, the delay
  schedule, the temporary lock and the page design here are ours.

[RFC 6749]: https://www.rfc-editor.org/rfc/rfc6749
[RFC 8265]: https://www.rfc-editor.org/rfc/rfc8265
[RFC 9106]: https://www.rfc-editor.org/rfc/rfc9106
[RFC 9700]: https://www.rfc-editor.org/rfc/rfc9700
[OIDC Core]: https://openid.net/specs/openid-connect-core-1_0.html
[NIST SP 800-63B-4]: https://pages.nist.gov/800-63-4/sp800-63b.html
[OWASP Password Storage]: https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
[OWASP Authentication]: https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
[OWASP CSRF]: https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html
[HIBP range]: https://haveibeenpwned.com/API/v3#SearchingPwnedPasswordsByRange
[argon2]: https://docs.rs/argon2
[askama]: https://docs.rs/askama
[governor]: https://docs.rs/governor
[Zitadel default settings]: https://zitadel.com/docs/guides/manage/console/default-settings
