# 12. Passkeys and multi-factor authentication `[P2]`

This file covers WebAuthn passkeys, TOTP, one-time codes sent by email or SMS, recovery codes,
the org's MFA policy, step-up, and the rules for adding and removing factors.

The credential fields are in [03](03-data-model.md) section 2.3.1 and the events in
[04](04-event-catalog.md) section 3.3. Session factors, `amr` and `acr` are in
[10](10-sessions-and-logout.md) section 3. The login pages are in [11](11-login-and-passwords.md).

## 1. Factor categories

| Factor | Category | Phishing-resistant | Counts for the org's `mfa` policy | Highest `acr` it can help reach |
|---|---|---|---|---|
| Password | Knowledge | No | First factor only | `urn:loams:aal1` alone |
| Passkey with user verification | Possession plus knowledge or inherence (in one device) | Yes | Yes, alone | `urn:loams:aal2:phr` |
| Passkey without user verification | Possession | Yes | Yes, as a second factor | `urn:loams:aal2` (with a password) |
| TOTP | Possession | No | Yes, as a second factor | `urn:loams:aal2` (with a password) |
| Recovery code | Possession (look-up secret) | No | Yes, as a second factor | `urn:loams:aal2` (with a password) |
| Email OTP | Possession of the mailbox | No | Yes, as a second factor, when the org enables it | `urn:loams:aal1` |
| SMS OTP | Possession of the phone number | No | Yes, as a second factor, when the operator and org enable it | `urn:loams:aal1` |

**MFA-1.** *Conservative choice.* Codes sent by email or SMS never raise the session's `acr`
above `urn:loams:aal1`. [NIST SP 800-63B-4] section 3.1.3 says email SHALL NOT be used for
out-of-band authentication and that the public telephone network is a restricted authenticator.
An org may still accept them for its own `mfa` policy (it is the org's choice), but a client that
asks for `urn:loams:aal2` gets a stronger factor (section 7).

## 2. Passkeys (WebAuthn)

Passkeys follow [WebAuthn Level 3]. The server side is an adapter over `webauthn-rs` behind the
`PasskeyVerifier` trait (ADR 0007). The simulation uses a scripted authenticator instead
([01](01-architecture.md) section 3).

### 2.1 Relying-party settings

**MFA-2.** The relying-party id (RP ID) is the instance's primary host (TEN-1), for example
`login.example.com`. The expected origin is the instance's issuer origin. In the path issuer
form, all instances on one host share the RP ID, so a passkey is bound to the host; the
credential lookup is still per instance (TEN-14), so a passkey registered in one instance never
authenticates in another.

**MFA-3.** An instance with several hosts MAY publish the related-origins document at
`/.well-known/webauthn` ([WebAuthn Level 3] section 5.11, related origin requests) listing its
other hosts, so a passkey made on the primary host works on the others. Off by default.

**MFA-4.** Public key algorithms offered, in order of preference: EdDSA (COSE `-8`), ES256
(`-7`), RS256 (`-257`).

**MFA-5.** Attestation conveyance is `none` by default. The server does not verify attestation
statements and records only the AAGUID the authenticator reports, for display. An instance may
set `attestation: direct` with an allow-list of AAGUIDs in a later phase (ADR required); that is
not part of Phase 2.

### 2.2 Ceremony state

**MFA-6.** Each ceremony has server-side state (the challenge and the expected parameters)
between the "begin" and "finish" requests. The state is stored in the `MetaStore` ephemeral space
under `(instance, ceremony id)`, with a 5m expiry, bound to the session id or the
browser-binding value ([11](11-login-and-passwords.md) section 3.1), and deleted when the finish request reads it, so each challenge is used at most once. The
challenge is 32 bytes. Storing the adapter's state outside process memory needs the crate's
state-serialization feature; ADR 0007 records that this feature is used and why.

**MFA-7.** The browser side is one small static script on the login UI's origin
(LOGIN-2). It calls `navigator.credentials.create` or `navigator.credentials.get` with the options
the server returned, and posts the result as JSON to the finish endpoint with the CSRF token
(LOGIN-43).

### 2.3 Registration

Registration happens from `/account` (re-authentication required, SES-13), from the invitation
page (LOGIN-40), during self-registration when passkeys are allowed, or from `/login/mfa/setup`.

**MFA-8.** The begin step returns creation options with:

| Option | Value |
|---|---|
| `rp` | `{ id: RP ID, name: instance display name }` |
| `user` | `{ id: the user id's 16 bytes, name: login name, displayName: display name or login name }`. The user handle holds no personal data. |
| `challenge` | 32 bytes from the ceremony state |
| `pubKeyCredParams` | MFA-4 |
| `excludeCredentials` | The user's registered credential ids, so the same authenticator is not added twice |
| `authenticatorSelection` | `residentKey: required`, `userVerification: required` |
| `attestation` | `none` |
| `timeout` | 300000 ms |

**MFA-9.** The finish step verifies, per [WebAuthn Level 3] section 7.1: the client data `type`
is `webauthn.create`, the challenge equals the stored one, the origin equals the expected
origin exactly, the RP ID hash matches, user presence (UP) and user verification (UV) flags are
set, the algorithm is one offered. It then appends `user.passkey_registered` with the fields of
[03](03-data-model.md) section 2.3.1, claiming the `passkey_credentials` index for the credential
id (ES-3). A credential id already registered to any user in the instance fails the
registration with "This passkey is already registered."

**MFA-10.** The user names the passkey (default: "Passkey" plus the date). The registration
sends a notification email when the user has a verified address.

### 2.4 Authentication

**MFA-11.** Two entry points:

- **Conditional mediation** (autofill). The `/login` page starts a conditional
  `navigator.credentials.get` when the browser supports it, and the login name field carries
  `autocomplete="username webauthn"`. Picking a passkey from the autofill list signs in without
  typing a name.
- **Modal**. The "sign in with a passkey" button on `/login`, and the "use a passkey instead" link
  on `/login/password`.

**MFA-12.** *Conservative choice.* Every authentication request uses an empty
`allowCredentials` list (discoverable credentials only), even after the user typed a login name.
Listing the user's credential ids would reveal whether the account exists and what it holds. A
passkey used as a second factor (MFA-16) is the only case with a non-empty list, because the user
has already passed the first factor.

**MFA-13.** The request options are `challenge` (32 bytes), `rpId`, `userVerification:
required` for sign-in, `timeout: 300000`.

**MFA-14.** The finish step looks up the credential id in the `passkey_credentials` index for the
resolved instance, loads the user, and verifies per [WebAuthn Level 3] section 7.2: type
`webauthn.get`, challenge, exact origin, RP ID hash, UP set, UV set (for sign-in), the signature
over the authenticator data and client data hash with the stored public key, and that the
returned user handle equals the user id. The user must be `active` (DM-13). On success the
factor record (`session.factor_verified`) carries the credential id and `uv: true` or `false`
from the authenticator data flags (the records of SES-10). Every failure shows
the same message, "That passkey could not be used. Try again or use another way to sign in.",
and appends `observed.passkey_check_failed` with the reason.

**MFA-15.** Signature counter and backup flags:

- If the stored counter and the new counter are both 0, accept (many synced passkeys never
  count).
- If the stored counter is non-zero and the new counter is not greater, reject the sign-in with
  the reason `counter_regression`, and email the user that a possibly cloned passkey was used.
  The passkey stays registered; the user may remove it.
- Otherwise accept and append `user.passkey_used` with the new counter.
- The backup-eligible flag (BE) is fixed at registration; an assertion whose BE flag differs
  from the stored value is rejected. The backup-state flag (BS) may change and is recorded.

**MFA-16.** A passkey may serve as a second factor after a password. Then UV is `preferred`, the
`allowCredentials` list holds the user's credential ids, and a result without UV adds `pop` only
([10](10-sessions-and-logout.md) SES-10).

## 3. TOTP

TOTP follows [RFC 6238] over HOTP ([RFC 4226]). Verification goes through `totp-rs`, using only
its functions that take the time as an argument (ADR 0008), with the time from `Clock`.

### 3.1 Enrolment

**MFA-17.** From `/account` (re-authentication required) or `/login/mfa/setup`:

1. Draw a 20-byte secret from `Rng`. Append `user.totp_added` with the secret encrypted under the
   user's data key (DM-6), `algorithm: SHA1`, `digits: 6`, `period: 30`. These parameters are
   chosen for compatibility: common authenticator apps ignore other values.
2. Show the `otpauth://totp/` URI with `issuer` set to the instance's display name and the label
   `<issuer>:<login name>`, as a QR code rendered as inline SVG markup (no `data:` URL, which the
   CSP forbids, LOGIN-45), and the secret in base32 for manual entry.
3. Ask for one code. A correct code appends `user.totp_confirmed`. Only a confirmed TOTP counts
   as a factor. An unconfirmed TOTP older than 10m is discarded at the next enrolment attempt.

**MFA-18.** A user has at most one TOTP. Adding a new one replaces the old one only after the new
one is confirmed.

### 3.2 Verification

**MFA-19.** A code is accepted if it equals the TOTP value for time step `T`, `T - 1` or `T + 1`,
where `T = floor(unix seconds / 30)` from `Clock`, and the matching step is greater than the
stored `last_used_step` (DM-9). On success the handler appends `user.totp_step_used` with that
step in the same batch as the session factor, so two concurrent submits of the same code cannot
both succeed (the second fails with a version conflict and, on retry, sees the used step).

**MFA-20.** Codes are compared in constant time. Input is trimmed and spaces removed; anything
other than six digits fails without computing.

**MFA-21.** Failures count towards `otp_failures` and the lockout rules in
[11](11-login-and-passwords.md) section 6, and append `observed.otp_check_failed`.

## 4. One-time codes by email and SMS

**MFA-22.** Email OTP is off by default (`allow_email_otp: false` in the login policy). When on,
and the user has a verified email address, `/login/mfa` offers "email me a code". The code is 8
digits from `Rng` (uniform, no modulo bias), valid for 10m, stored as SHA-256 in the `MetaStore`
ephemeral space under the user and the session or flow, at most 5 tries (the OTP counters), and
used once. Sending is limited to 3 per hour per address (LOGIN-30). The email states the
instance name and the code, never a link that signs in.

**MFA-23.** SMS OTP is off by default at two levels: the operator must configure an `SmsSender`
and the org must enable `allow_sms_otp`. It works like email OTP with 6 digits and a verified
phone number (`user.phone_verified`), and adds `sms` to `amr`. The UI labels it as the weakest
option.

**MFA-24.** Neither email nor SMS codes raise `acr` above `urn:loams:aal1` (MFA-1).

## 5. Recovery codes

**MFA-25.** When a user first confirms a second factor or a passkey, the server generates 10
recovery codes. Each is 12 characters from the Crockford base32 alphabet (60 bits, drawn from
`Rng`), shown grouped as `XXXX-XXXX-XXXX`. The batch appends `user.recovery_codes_generated` with
the SHA-256 of each normalized code (upper case, hyphens removed, `O` read as `0` and `I`/`L` as
`1`). The codes are shown once, with a "download as text" button.

**MFA-26.** A recovery code is accepted in place of the second factor, used once
(`user.recovery_code_used` with its index, guarded by the user stream's version), and counts
towards `otp_failures` when wrong. After using one, the user is asked to set up a new second
factor and to generate new codes. Generating new codes replaces all old ones.

**MFA-27.** When fewer than 3 unused codes remain, `/account` shows a warning.

## 6. `amr` and `mfa`

**MFA-28.** The session's `amr` is the union of the values its factors add
([10](10-sessions-and-logout.md) SES-10), plus `mfa` when either:

- a passkey with user verification was verified; or
- factors of two different categories in section 1 were verified (for example password and TOTP,
  or password and email OTP).

**MFA-29.** `acr` follows SES-11, with the limits of MFA-1.

**MFA-30.** A recovery code adds `otp`, because [RFC 8176] has no value for look-up secrets and
`otp` is the closest registered meaning.

## 7. Policy enforcement and step-up

**MFA-31.** The org login policy's `mfa` setting ([03](03-data-model.md) section 3):

| Value | Effect at sign-in |
|---|---|
| `none` | A second factor is asked only if the user has one confirmed (users who opt in are always asked). |
| `required` | Every sign-in must end with `mfa` in `amr`. A user without a second factor is sent to `/login/mfa/setup` after the first factor, in the same flow, and cannot continue without enrolling. |
| `required_for_local` | As `required` for local credentials; sign-ins through an upstream IdP rely on the IdP's own checks `[P5]` ([16](16-federation.md)). |

*Conservative choice:* there is no "skip for now" grace period under `required`.

**MFA-32.** Step-up. When an authorization request asks, through `acr_values` or the client's
`required_acr` ([09](09-oidc.md) OIDC-8), for a level the session lacks, the login UI asks for the
missing factor: a second factor for `urn:loams:aal2`, a passkey with UV
for `urn:loams:aal2:phr`. A verified factor appends `session.factor_verified`, and `auth_time`
moves to that time (SES-12).

**MFA-33.** What happens when the user cannot reach the level follows [09](09-oidc.md) OIDC-8:
`acr_values` is voluntary, so the login completes and the ID token reports the `acr` actually
reached; the client's `required_acr` is a hard requirement, so the user must enroll a suitable
factor in the same flow (`/login/mfa/setup`) or the request ends with an error to the client.
(The error code is the one OIDC-8 fixes; [OIDC Unmet Authentication Requirements] defines
`unmet_authentication_requirements` for this case.)

**MFA-34.** The ID token's `acr` is the level actually reached, and `amr` lists the methods
([09](09-oidc.md)).

## 8. Adding and removing factors

**MFA-35.** Adding or removing any factor, generating recovery codes, and changing the password
require re-authentication within `reauth_for_sensitive` (SES-13).

**MFA-36.** A user can never be left without a usable sign-in. A removal is refused when, after
it, the user would have no password and no passkey; and, under `mfa: required`, when the user
would have no second factor and no passkey with UV. The message says what to add first.

**MFA-37.** Every addition and removal sends a notification email to the user's verified
address, naming the factor kind and the time, with a link to `/account`.

**MFA-38.** An org admin can reset a user's second factors through the management API
([17](17-management-api.md)). That appends `user.totp_removed`, `user.passkey_removed` and new
empty recovery codes as needed, ends all the user's sessions, and forces enrolment at the next
sign-in when the policy requires MFA.

## 9. Tests that cover this file

- WebAuthn: wrong origin, wrong RP ID, replayed challenge, missing UV, user-handle mismatch,
  counter regression, BE flag change, credential registered in another instance (MFA-2, MFA-6,
  MFA-9, MFA-14, MFA-15).
- Enumeration: passkey options are identical for known and unknown login names (MFA-12).
- TOTP: steps `T - 2` and `T + 2` fail, the same step twice fails, concurrent submits of one code
  succeed once (MFA-19).
- Policy: `required` cannot be bypassed by abandoning `/login/mfa/setup` and returning to the
  client (MFA-31).
- Removal rules: no sequence of removals leaves a user without a sign-in (MFA-36), checked as a
  property test.

## Sources

- [WebAuthn Level 3]: W3C Web Authentication Level 3, Recommendation of 2026-08-25: sections 5
  (API, related origins), 6 (authenticator data, flags, signature counter) and 7 (registration and
  authentication verification steps).
- [RFC 4226]: HOTP. [RFC 6238]: TOTP, sections 4 and 5.2 (steps, skew, replay).
- [RFC 8176]: Authentication Method Reference Values.
- [OIDC Core]: OpenID Connect Core 1.0, sections 2 (`acr`, `amr`) and 3.1.2.1 (`acr_values`).
- [OIDC Unmet Authentication Requirements]: the `unmet_authentication_requirements` error code.
- [NIST SP 800-63B-4]: sections 2 (assurance levels), 3.1.3 (email and telephone out-of-band
  restrictions), 3.1.2 (look-up secrets).
- [Key URI Format]: the `otpauth://` URI convention used by authenticator apps.
- Crate documentation: [webauthn-rs], [totp-rs].
- Zitadel public docs, observable behaviour only: a product of this kind offers passkeys
  (device-bound and roaming), TOTP, U2F, and OTP by email and SMS as second factors, and a
  force-MFA setting with a local-only variant ([Zitadel default settings]). The factor-strength
  rules, `acr` names, recovery codes and removal rules are ours.

[WebAuthn Level 3]: https://www.w3.org/TR/webauthn-3/
[RFC 4226]: https://www.rfc-editor.org/rfc/rfc4226
[RFC 6238]: https://www.rfc-editor.org/rfc/rfc6238
[RFC 8176]: https://www.rfc-editor.org/rfc/rfc8176
[OIDC Core]: https://openid.net/specs/openid-connect-core-1_0.html
[OIDC Unmet Authentication Requirements]: https://openid.net/specs/openid-connect-unmet-authentication-requirements-1_0.html
[NIST SP 800-63B-4]: https://pages.nist.gov/800-63-4/sp800-63b.html
[Key URI Format]: https://github.com/google/google-authenticator/wiki/Key-Uri-Format
[webauthn-rs]: https://docs.rs/webauthn-rs
[totp-rs]: https://docs.rs/totp-rs
[Zitadel default settings]: https://zitadel.com/docs/guides/manage/console/default-settings
