# 03. Data model

The domain is a set of event-sourced **aggregates**. An aggregate is a consistency boundary: one
event stream, one version number, one owner of its invariants. This file lists each aggregate,
its fields, its states and its invariants. The events that change them are in
[04](04-event-catalog.md); how they are stored is in [05](05-event-store-and-projections.md).

The aggregate list and every field below are our own design, derived from what the standards
require a provider to remember (for example a code's PKCE challenge, [RFC 7636]) and from the
Loams contracts.

## 1. Common rules

**DM-1.** Every aggregate id is a UUIDv7 (ADR 0013). Ids are typed in Rust (`UserId`,
`OrgId`, ...) so one kind cannot be passed where another is expected.

**DM-2.** Every aggregate except `Instance` carries `instance_id`. Every aggregate except
`Instance` and `SigningKey` carries `org_id`. Both are set by the creating event and never change.

**DM-3.** Timestamps are UTC with nanosecond precision, taken from `Clock`, and stored as
`i64` nanoseconds since the Unix epoch. JSON renders them as RFC 3339 strings with a `Z` offset;
protocol claims render them as whole seconds (NumericDate, [RFC 7519] section 2).

**DM-4.** Secrets are never stored in plain form:

| Secret | Stored as |
|---|---|
| Password | argon2id PHC string (ADR 0009) |
| Client secret, PAT, refresh token, opaque access token, authorization code, device code, session cookie value, setup token, password reset and invitation token, email verification token, recovery code, email OTP | SHA-256 of the full token string (they are random with at least 256 bits of entropy, or are short-lived and attempt-limited); compared in constant time |
| TOTP shared secret | Encrypted with the user's data key (section 1, DM-6) |
| Signing private keys | Inside the `KeyStore` only; the aggregate holds a key handle |
| Upstream IdP client secret `[P5]` | Encrypted with the instance data key |

**DM-5.** Fields are classified. The class decides logging, export and encryption.

| Class | Examples | Logged | Exported to sinks | At rest in events |
|---|---|---|---|---|
| `id` | aggregate ids, client ids, key ids | Yes | Yes | Plain |
| `config` | org names, project names, redirect URIs, policies | Yes | Yes | Plain |
| `personal` | human names, email, phone, IP address, user agent, login name | No (ids only) | Only to sinks allowed to receive PII, per sink setting | Encrypted with the subject's data key |
| `secret` | hashes, encrypted secrets, key handles | Never | Never | Hashed or encrypted as in DM-4 |

**DM-6.** *Crypto-shredding* (ADR 0018). Each human user has a data key (a 256-bit key wrapped by
the `KeyStore`). Fields of class `personal` in that user's events are encrypted with it
(AES-256-GCM, nonce from `Rng`, the event id as associated data). Erasing the user destroys the
data key; the events stay, and their personal fields become unreadable. Projections that hold
personal fields are rebuilt or scrubbed after erasure. Login names are personal but also need a
uniqueness index; the index stores a keyed hash (HMAC-SHA-256 with an instance key) of the
normalized login name, not the name itself.

## 2. Aggregates

### 2.1 Instance

| Field | Type | Notes |
|---|---|---|
| `id` | InstanceId | |
| `slug` | string | `[a-z0-9-]{1,63}`, unique, used in the path issuer form |
| `issuer` | URL | TEN-1 |
| `hosts` | list of host names | Each unique across all instances |
| `state` | `active` \| `suspended` \| `removed` | |
| `default_org_id` | OrgId | Set at setup |
| `settings` | InstanceSettings | Section 3 |
| `data_key` | key handle | Wraps instance-level encrypted fields and the login-name HMAC key |

### 2.2 Org

| Field | Type | Notes |
|---|---|---|
| `id`, `instance_id` | | |
| `name` | string | 1 to 200 characters, unique per instance (case-insensitive) |
| `state` | `active` \| `deactivated` \| `removed` | TEN-7 |
| `domains` | list of OrgDomain | `{ name, verified, primary, verification: { method: dns_txt \| http, token_hash, last_checked_at } }` |
| `settings` | OrgSettings | Overrides; unset means inherit |
| `members` | map UserId → set of OrgRole | OrgRole is `owner`, `admin` or `member` (identity's coarse roles; [15](15-loams-integration.md) maps them to OpenFGA) |

**DM-7.** An active org always has at least one `owner`. A command that would remove the last
owner fails.

### 2.3 User

One aggregate for humans and machines; `kind` decides which parts are present.

| Field | Type | Notes |
|---|---|---|
| `id`, `instance_id`, `org_id` | | `id` is the `sub` claim |
| `kind` | `human` \| `machine` | Fixed at creation |
| `state` | `initial` \| `active` \| `locked` \| `deactivated` \| `removed` | Section 4.1 |
| `login_name` | string, personal | Unique per instance after normalization (NFC, lower case, trimmed) |
| `human` | HumanProfile, optional | `given_name`, `family_name`, `display_name`, `locale`, `email {address, verified}`, `phone {number, verified}` `[P2]` |
| `machine` | MachineProfile, optional | `name`, `description`, `purpose: service_account \| agent`, `agent: AgentProfile` when purpose is agent |
| `credentials` | Credentials | Section 2.3.1 |
| `lockout` | `{ password_failures, otp_failures, locked_until }` | Counters reset on success |
| `metadata` | map string → bytes `[P4]` | At most 64 entries, values at most 4 KiB |
| `external_ids` | list `{ idp_id, subject }` `[P5]` | Unique per (instance, idp, subject) |
| `data_key` | key handle | DM-6 (humans only) |

AgentProfile (`[P3]`): `owner_user_id` (a human user in the same org, accountable),
`loams_project_ref` (an opaque reference to the Loams project, stored and exported but not
validated here; [15](15-loams-integration.md) LOAMS-19), `token_ttl_cap` (default 15m, at most 1h), `trust_policies` (list of workload-identity rules,
[13](13-machine-identity.md) section 8), `status` is the user's `state`. Service accounts also
carry an optional `loams_project_ref`.

**DM-8.** An agent's owner must be an active human user of the same org. Deactivating or
removing the owner does not deactivate the agent automatically; it emits
`user.agent_owner_lost` and the org's admins must reassign or deactivate it. *Conservative
choice:* while an agent has no active owner, token issuance for it is refused.

#### 2.3.1 Credentials

| Credential | Fields | Kinds | Phase |
|---|---|---|---|
| Password | `phc`, `changed_at`, `must_change` | human | 1 |
| Passkeys | list `{ credential_id, public_key (COSE), sign_count, backup_eligible, backup_state, transports, aaguid, user_handle, name, created_at, last_used_at }` | human | 2 |
| TOTP | `{ secret_enc, algorithm, digits, period, confirmed, last_used_step }` | human | 2 |
| Recovery codes | list of `{ hash, used_at }`, 10 codes | human | 2 |
| Machine keys | list `{ key_id, public_jwk, alg, expires_at, created_at }` | machine | 3 |
| Client secret | `{ hash, created_at, expires_at }`, at most two at a time (rotation) | machine | 3 |
| PATs | list `{ pat_id, hash, scopes, audience, expires_at, created_at, last_used_at }` | machine | 3 |

**DM-9.** `last_used_step` for TOTP stores the last accepted time step; a code for that step or
an earlier step is rejected (replay protection, [RFC 6238] section 5.2).

### 2.4 Project

| Field | Type | Notes |
|---|---|---|
| `id`, `instance_id`, `org_id` | | |
| `name` | string | Unique per org |
| `state` | `active` \| `deactivated` \| `removed` | |
| `roles` | map RoleKey → `{ display_name, group }` `[P4]` | RoleKey: `[A-Za-z0-9_.:-]{1,100}` |
| `settings` | `{ assert_roles_in_tokens, require_role_for_login, require_org_grant_for_login }` `[P4]` | |

Every org gets a project named `default` at creation, so Phase 1 applications have a home.

### 2.5 Application (OAuth client)

| Field | Type | Notes |
|---|---|---|
| `id`, `instance_id`, `org_id`, `project_id` | | |
| `client_id` | string | Unique per instance. Generated: 24 characters of base32 from `Rng`, or chosen by the admin (`[a-zA-Z0-9._-]{3,128}`, and never a string that parses as a UUID, so it cannot collide with a machine user's implicit client id; [13](13-machine-identity.md) MACH-4) |
| `name` | string | |
| `kind` | `web` \| `spa` \| `native` \| `service` \| `resource_server` | Section 5 of [07](07-oauth-core.md) |
| `state` | `active` \| `deactivated` \| `removed` | |
| `client_type` | `confidential` \| `public` | Derived from `kind`: `web` and `service` are confidential, `spa` and `native` are public, `resource_server` is confidential (it only introspects) |
| `token_endpoint_auth_method` | `client_secret_basic` \| `client_secret_post` \| `private_key_jwt` \| `none` | `none` only for public clients |
| `secrets` | at most two `{ hash, created_at, expires_at }` | Confidential with a secret method only |
| `jwks` or `jwks_uri` | JWK set or URL | `private_key_jwt` only |
| `redirect_uris` | list of URI | Exact match ([07](07-oauth-core.md) OAUTH-8) |
| `post_logout_redirect_uris` | list of URI | Exact match |
| `grant_types` | set | `authorization_code`, `refresh_token`, `client_credentials` `[P3]`, `urn:ietf:params:oauth:grant-type:device_code` `[P3]`, `urn:ietf:params:oauth:grant-type:token-exchange` `[P3]`, `urn:ietf:params:oauth:grant-type:jwt-bearer` `[P3]` |
| `access_token_format` | `jwt` \| `opaque` | Default `jwt` (ADR 0012) |
| `id_token_signed_response_alg` | `EdDSA` \| `ES256` \| `RS256` | Default per ADR 0006 |
| `allowed_scopes` | set of scope strings | Requests for other scopes are narrowed or refused ([07](07-oauth-core.md)) |
| `first_party` | bool | First-party clients skip the consent screen |
| `lifetimes` | optional overrides of the token policy | Never longer than the instance maximums |
| `backchannel_logout_uri` | URI, optional `[P6]` | |
| `resource_identifiers` | list of URI `[P3]` | For `resource_server`: the values accepted as `resource` ([RFC 8707]) and used as `aud` |
| `id_token_profile_claims` | bool | Default `false`: profile, email and phone claims come from userinfo only ([09](09-oidc.md) OIDC-4) |
| `required_acr` | acr value, optional | The server enforces this level for every login to the client, whatever `acr_values` says ([09](09-oidc.md)) |
| `default_resource` | application id, optional `[P3]` | The `resource_server` used as the audience when the client sends no `resource` ([08](08-tokens.md) TOK-6) |
| `agent_user_id` | UserId, optional `[P3]` | Binds the application to one agent (the MCP flow, [13](13-machine-identity.md) MACH-41) |
| `pkce_exception` | `none` \| `nonce` | Default `none`. `nonce` only for confidential `web` clients with `openid` allowed ([07](07-oauth-core.md) OAUTH-14a) |
| `dev_mode` | bool | Allows `http://localhost` and `http://127.0.0.1` redirect URIs for non-native kinds; refused in instances with `production: true` |

**DM-10.** A public client has no secret and no keys and its `token_endpoint_auth_method` is
`none`. A confidential client never has `none`.

**DM-11.** A client's `redirect_uris` MUST NOT contain a fragment, and MUST use `https` except:
native clients may use loopback `http` with any port and private-use URI schemes
([RFC 8252] sections 7.1 and 7.3); `dev_mode` clients may use loopback `http`.

### 2.6 Grant

A user's standing authorization of one client.

| Field | Type | Notes |
|---|---|---|
| `id`, `instance_id`, `org_id` | | |
| `user_id`, `client_id` | | Unique pair per instance while active |
| `scopes` | set | The consented scopes |
| `state` | `active` \| `revoked` | |
| `created_at`, `updated_at` | | |

**DM-12.** Revoking a grant revokes every token family that references it (an event on each
family, see [05](05-event-store-and-projections.md) section 3.3 for how the fan-out is made
reliable). After `grant.revoked` is appended, no new token may be issued under the grant.

### 2.7 AuthRequest (ephemeral)

The state of one authorization transaction, from `/authorize` to the redeemed code.

| Field | Notes |
|---|---|
| `id`, `instance_id`, `org_id` (set once the user is known; until then the client's org) | |
| `client_id`, `redirect_uri`, `response_mode`, `scope`, `state`, `nonce` | As received and validated |
| `code_challenge`, `code_challenge_method` | Present unless the client has `pkce_exception: nonce` and sent none (OAUTH-14, OAUTH-14a) |
| `prompt`, `max_age`, `acr_values`, `login_hint`, `ui_locales`, `org_hint` | |
| `resource` | list `[P3]` |
| `status` | Section 4.3 |
| `user_id`, `session_id`, `auth_time`, `amr`, `acr` | Set at authentication |
| `consented_scopes` | Set at consent |
| `code_hash`, `code_expires_at` | Set when the code is issued |
| `issued_family_id` | Set when the code is redeemed and tokens are issued |
| `expires_at` | 10m after creation for the login part |

### 2.8 Session (browser single sign-on)

| Field | Notes |
|---|---|
| `id`, `instance_id`, `org_id`, `user_id` | |
| `cookie_hash` | SHA-256 of the cookie value |
| `state` | `active` \| `ended` \| `expired` |
| `factors` | list `{ method, at, details }`; method is one of the `amr` values in [09](09-oidc.md) |
| `created_at`, `last_active_at` (updated at most every 5m), `idle_expires_at`, `absolute_expires_at` | |
| `participants` | list `{ client_id, sid_issued_at }`: clients that received an ID token in this session (for logout) |
| `user_agent`, `ip` | personal, for the user's session list |

### 2.9 TokenFamily

All refresh tokens and access tokens descended from one code redemption, one token exchange or
one device-flow redemption.

| Field | Notes |
|---|---|
| `id`, `instance_id`, `org_id` | |
| `grant_id` | Optional; absent for client credentials and machine flows |
| `client_id`, `subject` (`user_id`), `act` (actor chain, [RFC 8693] section 4.1), `agent_session_id` `[P3]` | |
| `session_id` | Set when the family is bound to a browser session (no `offline_access`); [08](08-tokens.md) TOK-14 |
| `scopes`, `audience`, `auth_time`, `amr`, `acr` | Fixed at creation; refresh may narrow scopes, never widen |
| `generation` | Starts at 1; increments on each rotation |
| `current_refresh_hash` | Hash of the only refresh token that is valid now |
| `retired_refresh_hashes` | Hashes of earlier generations, kept to detect reuse until the family's absolute expiry |
| `access_token_ids` | `jti`s issued in this family (bounded: the last 32), for revocation and introspection |
| `state` | `active` \| `revoked` \| `expired`, with `revoked_reason` |
| `idle_expires_at`, `absolute_expires_at` | |

### 2.10 SigningKey

| Field | Notes |
|---|---|
| `id` | Also the `kid`. A random 16-byte value, base64url, never derived from key material (KEY-4) |
| `instance_id`, `alg`, `use` (`sig`) | |
| `key_handle` | Reference inside the `KeyStore` |
| `public_jwk` | |
| `state` | `pending` \| `active` \| `retiring` \| `retired` ([06](06-keys-and-jose.md) section 2) |
| `created_at`, `activated_at`, `retiring_at`, `retired_at` | |

### 2.11 DeviceAuthorization (ephemeral, `[P3]`)

`device_code_hash`, `user_code` (normalized; personal), `client_id`, `scope`, `status`
(`pending`, `approved`, `denied`, `redeemed`, `expired`), `interval`, `last_polled_at`,
`user_id` and `session_id` once approved, `expires_at`.

### 2.12 AgentSession (`[P3]`)

`id`, `instance_id`, `org_id`, `agent_user_id`, `owner_user_id`, `purpose` (free text, at most
200 characters), `expires_at` (at most 24h), `state` (`active`, `ended`, `expired`). Ending it
revokes every family whose `agent_session_id` matches.

### 2.13 Later aggregates

- `ProjectGrant` and `RoleAssignment` `[P4]`, and the `admins` maps on `Instance` and `Project`
  (management roles, from Phase 1): [14](14-orgs-projects-roles.md).
- `IdentityProvider` and `IdpIntent` `[P5]`: fields in [16](16-federation.md) sections 2 and 3.
- `WebhookTarget` `[P4]`: [15](15-loams-integration.md) section 5.

## 3. Settings

Instance settings hold defaults; org settings override field by field.

| Group | Fields and defaults |
|---|---|
| Login policy | `allow_password: true`, `allow_passkey: true` `[P2]`, `allow_registration: false`, `allow_email_otp: false` `[P2]`, `allow_sms_otp: false` `[P2]`, `allow_external_idp: false` `[P5]`, `mfa: none \| required \| required_for_local` (default `none` in Phase 1, `required_for_local` recommended once Phase 2 ships), `passkey_only: false` |
| Password policy | `min_length: 15` (8 when the org requires MFA for every login; [11](11-login-and-passwords.md) LOGIN-22), `max_length: 128`, `check_breached: false` (a local blocklist of common passwords is always on), no composition rules ([11](11-login-and-passwords.md) section 3) |
| Lockout policy | `password_failure_threshold: 10`, `otp_failure_threshold: 5`, `lock_duration: 15m`, `admin_unlock_only: false` ([11](11-login-and-passwords.md) section 4) |
| Token policy | `access_token_ttl: 15m` (max 1h), `id_token_ttl: 15m`, `refresh_idle_ttl: 14d`, `refresh_absolute_ttl: 30d`, `agent_token_ttl_cap: 15m` (max 1h), `code_ttl: 60s` |
| Session policy | `idle_ttl: 12h`, `absolute_ttl: 30d`, `reauth_for_sensitive: 15m` |
| Domain policy | `require_verified_org_domains: true`, `login_name_suffix: false` |
| Machine and delegation (org) `[P3]` | `allow_agent_client_secrets: false`, `delegation_rules` (which agents may act for which users; [13](13-machine-identity.md)) |
| Instance-only `[P3+]` | `management_clients` ([17](17-management-api.md)), `client_id_metadata_documents: false`, `dynamic_registration: false`, `require_pushed_authorization_requests: false` ([20](20-later-standards.md)) |
| Features | `device_flow`, `token_exchange`, `client_credentials`, `pats` (all `false` until Phase 3 ships, then `true` except `pats`) |

## 4. State machines

### 4.1 User

```text
            create (no credential)          set first credential
  (none) ──────────────────────► initial ──────────────────────► active
     │ create with credential                                    │  ▲
     └───────────────────────────────────────────────────────►  │  │ unlock / lock expires
                                                                 │  │
                       too many failures ──────────────────► locked
  active ── deactivate ──► deactivated ── reactivate ──► active
  any state except removed ── remove ──► removed (terminal)
```

**DM-13.** Only `active` users can authenticate or receive tokens. `locked` blocks the factor
that is locked (password or OTP), not the whole user: a locked password still allows a passkey
sign-in if the login policy allows passkeys without a password. *Conservative choice:* an
`admin_unlock_only` lock blocks every factor.

**DM-14.** Deactivating or removing a user ends all their sessions and revokes all their token
families and PATs (fan-out, [05](05-event-store-and-projections.md) section 3.3). Token
introspection for that user's tokens returns `active: false` from the moment the user event is
appended, without waiting for the fan-out, because introspection checks the user's state too.

### 4.2 Application

`active` ⇄ `deactivated` → `removed`. Deactivating a client refuses new authorization requests
and token requests for it and makes introspection of its tokens return inactive.

### 4.3 AuthRequest and authorization code

```text
 created ──► authenticated ──► consented ──► code_issued ──► redeemed (terminal)
    │              │               │              │
    └──────────────┴───────────────┴──────────────┴──► failed | expired (terminal)

 redeemed ── code presented again ──► (stays redeemed) + family revoked (replay)
```

**DM-15.** A code is redeemable at most once. The `code_issued → redeemed` transition and the
creation of the token family happen in one atomic append ([05](05-event-store-and-projections.md)
section 3.2). A second presentation of the same code fails with `invalid_grant` and revokes the
family created by the first redemption ([RFC 6749] section 4.1.2, [RFC 9700] section 4.5).

**DM-16.** A code expires `code_ttl` (60s) after issuance. An expired code fails with
`invalid_grant` and changes no state other than an `auth_request.code_rejected` audit event.

### 4.4 Session

`active → ended` (logout, user or admin action, user deactivation) or `active → expired` (idle
or absolute expiry, decided by comparing `Clock` time; ARCH-11).

### 4.5 TokenFamily

```text
 active ── rotate (present current refresh token) ──► active (generation + 1)
 active ── present a retired refresh token ─────────► revoked (reason: reuse_detected)
 active ── revoke (client, user, admin, grant, session end, user deactivation) ──► revoked
 active ── idle or absolute expiry ──────────────────► expired
```

**DM-17.** At any moment a family has at most one valid refresh token, the one whose hash is
`current_refresh_hash`.

**DM-18.** Presenting any refresh token of a revoked or expired family fails with
`invalid_grant`. Presenting a retired token of an active family revokes the family first and
then fails with `invalid_grant` ([RFC 9700] section 4.14.2).

**DM-19.** No access token is issued in a family after it is revoked. Access tokens issued
earlier stay cryptographically valid until they expire, but introspection reports them inactive.

### 4.6 SigningKey

See [06](06-keys-and-jose.md) section 2.

## 5. Invariants checked by property tests and the TLA+ model

| Id | Invariant |
|---|---|
| DM-I1 | A code yields tokens at most once. |
| DM-I2 | After a reuse of a retired refresh token, the family is revoked and no later request in that family succeeds. |
| DM-I3 | No token is issued under a revoked grant, a revoked family, an ended session (for session-bound families), a deactivated or removed user, client or org. |
| DM-I4 | At most one valid refresh token per family. |
| DM-I5 | Every aggregate's `instance_id` and `org_id` never change. |
| DM-I6 | No read in org scope returns a record of another org. |
| DM-I7 | An active org has at least one owner. |

## Sources

- [RFC 6749]: OAuth 2.0, sections 4.1.2 (code lifetime and single use), 10.4 (refresh tokens).
- [RFC 6238]: TOTP, section 5.2 (replay of a used step).
- [RFC 7519]: JWT, section 2 (NumericDate).
- [RFC 7636]: PKCE (what a code must remember).
- [RFC 8252]: OAuth 2.0 for Native Apps, sections 7.1 and 7.3 (redirect URI forms).
- [RFC 8693]: Token Exchange, section 4.1 (`act`).
- [RFC 8707]: Resource Indicators.
- [RFC 9700]: OAuth 2.0 Security Best Current Practice, sections 4.5 and 4.14.2.
- [RFC 9562], UUIDv7.
- Loams design 19 sections 5.1 and 5.3 (agent owner, TTL cap, principal kinds) and design 76
  section 10 (agents as machine users), as Loams-side contracts.
- Zitadel public docs, observable behaviour only: users are human or machine and belong to one
  org for life; machine users have a name and description and authenticate with keys, client
  secrets or PATs; projects hold roles with a key, display name and group; project settings can
  assert roles or require a role for login ([Users], [Projects], [Service users]). Field names,
  states and invariants are ours.

[RFC 6749]: https://www.rfc-editor.org/rfc/rfc6749
[RFC 6238]: https://www.rfc-editor.org/rfc/rfc6238
[RFC 7519]: https://www.rfc-editor.org/rfc/rfc7519
[RFC 7636]: https://www.rfc-editor.org/rfc/rfc7636
[RFC 8252]: https://www.rfc-editor.org/rfc/rfc8252
[RFC 8693]: https://www.rfc-editor.org/rfc/rfc8693
[RFC 8707]: https://www.rfc-editor.org/rfc/rfc8707
[RFC 9700]: https://www.rfc-editor.org/rfc/rfc9700
[RFC 9562]: https://www.rfc-editor.org/rfc/rfc9562
[Users]: https://zitadel.com/docs/concepts/structure/users
[Projects]: https://zitadel.com/docs/concepts/structure/projects
[Service users]: https://zitadel.com/docs/guides/integrate/service-accounts/authenticate-service-accounts
