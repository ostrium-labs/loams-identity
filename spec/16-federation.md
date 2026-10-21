# 16. Federation with upstream identity providers `[P5]`

Federation lets a user sign in to loams-identity through another identity provider (an
**upstream IdP**): a company's OpenID Connect provider, Google, Microsoft Entra ID, GitHub,
GitLab, or a hosted identity service such as Clerk or WorkOS through their OIDC endpoints. Here
loams-identity is the relying party towards the upstream and stays the OP towards its own
clients. Tokens issued to Loams always come from loams-identity, never from the upstream.

## 1. Scope

**FED-1.** Supported upstream kinds:

| Kind | Protocol | Notes |
|---|---|---|
| `oidc` | OpenID Connect authorization code flow with PKCE | The generic kind. Clerk, WorkOS, Okta, Auth0, Keycloak, Authentik, Dex and others use it |
| `oauth2` | OAuth 2.0 authorization code flow plus a provider-specific user API | Only through a preset that defines the user API (GitHub) |

Presets fill in the endpoints and claim mapping for common providers:

| Preset | Kind | Fixed settings |
|---|---|---|
| `google` | `oidc` | Issuer `https://accounts.google.com` |
| `microsoft` | `oidc` | Issuer `https://login.microsoftonline.com/<tenant id>/v2.0`; a specific tenant id is required (FED-9) |
| `gitlab` | `oidc` | Issuer `https://gitlab.com` or a self-managed URL |
| `github` | `oauth2` | GitHub's authorize, token, user and user-emails endpoints; the subject is the numeric user `id`, never the `login` name, which users can change |
| `generic_oidc` | `oidc` | Everything from discovery of the configured issuer |

**FED-2.** Not supported: SAML upstreams (ADR 0011; [20](20-later-standards.md)), LDAP, IdP-initiated
sign-in (a callback that does not answer a request we started is refused, FED-13), and
propagating logout to or from the upstream (section 8).

## 2. The `IdentityProvider` aggregate

| Field | Notes |
|---|---|
| `id`, `instance_id`, `org_id` | `org_id` is `null` for an instance-level IdP, offered to every org that enables it |
| `name`, `button_label`, `preset` | |
| `kind` | `oidc` or `oauth2` |
| `issuer` | For `oidc`. Must be `https`. Compared exactly with the discovery `issuer` and the ID token `iss` |
| `client_id` | Our client id at the upstream |
| `client_secret_enc` | Encrypted with the instance data key (DM-4); never returned by the API |
| `scopes` | Default `openid profile email` for `oidc`; preset values for `oauth2` |
| `claim_mapping` | Section 6 |
| `allow_creation` | JIT provisioning on (default `false`) |
| `allow_linking` | Offer linking to an existing account (default `true`) |
| `update_profile_on_login` | Overwrite mapped profile fields on each sign-in (default `false`) |
| `trusted_email_domains` | Domains for which this IdP's verified-email assertion is trusted (FED-17); default empty |
| `allowed_email_domains` | If non-empty, sign-ins whose email domain is not listed are refused |
| `trust_upstream_mfa` | Default `false` (FED-21) |
| `state` | `active`, `deactivated`, `removed` |

**FED-3.** Events: `identity_provider.created`, `identity_provider.configuration_changed`,
`identity_provider.secret_set`, `identity_provider.deactivated`, `identity_provider.reactivated`,
`identity_provider.removed` (all exported as **audit**; payloads never hold the secret in plain
form, EV-3, EV-5). These extend [04](04-event-catalog.md) section 3.13.

**FED-4.** An org's login policy lists which instance-level IdPs it enables, in addition to its own
org-level IdPs. The login page shows the buttons for the resolved org only when
`allow_external_idp` is on ([03](03-data-model.md) section 3).

## 3. The `IdpIntent` aggregate (ephemeral)

One upstream round trip.

| Field | Notes |
|---|---|
| `id`, `instance_id`, `org_id` | |
| `idp_id`, `expected_issuer` | Copied from the IdP when the intent starts |
| `auth_request_id` | The attached `AuthRequest`, if any |
| `binding_hash` | SHA-256 of the value of the intent cookie `__Host-loams_idp` (FED-25) |
| `state_hash` | SHA-256 of the `state` value sent upstream |
| `nonce` | Sent upstream; compared on return |
| `pkce_verifier_enc` | Encrypted with the instance data key; needed in plain form at the token request |
| `status` | `started` → `succeeded` or `failed` (terminal) |
| `result` | On success: the upstream subject, mapped claims, and the resolved user or the pending decision (link or create) |
| `expires_at` | 10m after start |

Events: `idp_intent.started`, `idp_intent.succeeded`, `idp_intent.failed`. Ephemeral retention as
in [05](05-event-store-and-projections.md) section 7. Index: `idp_states` keyed by
`(instance, state hash)`, unique.

## 4. The sign-in flow

1. **Start.** The user picks an IdP button on `/login`. The form posts to `/login/idp/start` with
   the CSRF token (LOGIN-43). The handler creates an `IdpIntent` with `state` (32 bytes), `nonce`
   (32 bytes) and a PKCE verifier (32 bytes, `S256`), all drawn from `Rng`, sets the intent cookie
   (FED-25), and redirects (`303`) to the upstream's authorization endpoint with `response_type=code`, `client_id`,
   `redirect_uri`, `scope`, `state`, `nonce`, `code_challenge`, `code_challenge_method=S256`, and
   `login_hint` when the user typed a name.
2. **Callback.** The upstream redirects the browser to `/login/idp/callback`, the one callback URL
   of the instance. The handler validates (section 5), exchanges the code, validates the result,
   and resolves the user (section 7).
3. **Continue.** On success the handler appends `idp_intent.succeeded`, the session factor
   (`session.created` if needed, `session.factor_verified` with method `federated` and the IdP id
   in `details`) and `auth_request.authenticated`, then continues the login flow at the MFA and
   consent checks ([11](11-login-and-passwords.md) section 3.2).

**FED-5.** The `redirect_uri` sent upstream is exactly `<issuer>/login/idp/callback`. It is the
value registered at the upstream, and the same value is sent in the token request.

**FED-25.** The start step sets the cookie `__Host-loams_idp` (`Secure`, `HttpOnly`,
`SameSite=Lax`, `Path=/`, 10m) holding 32 random bytes whose SHA-256 is the intent's
`binding_hash`. It must be `Lax`, not `Strict`, because the callback is a navigation from the
upstream's site. The callback clears it.

## 5. Validating the callback and the upstream response

**FED-6.** The callback is accepted only if all hold:

1. A `state` parameter is present and its hash finds an `IdpIntent` in the resolved instance
   with status `started` and not expired.
2. The browser's `__Host-loams_idp` cookie hashes to the intent's `binding_hash` (binding to the
   browser, FED-25). For an attached login, the `__Host-loams_ar` cookie must also still name the
   intent's auth request (LOGIN-6).
3. If the upstream's metadata says `authorization_response_iss_parameter_supported: true`, the
   `iss` parameter is present and equals `expected_issuer` ([RFC 9207]).
4. There is a `code` and no `error`. An upstream `error` fails the intent and shows "Sign-in with
   {IdP} did not complete." without echoing the upstream's description.

Whatever the outcome, the intent becomes terminal in the same step, so a `state` value is used at
most once.

**FED-7.** The token request goes to the upstream's token endpoint with the code, the same
`redirect_uri`, the PKCE verifier, and client authentication (`client_secret_basic` by default,
`client_secret_post` when the upstream's metadata lists only that). Timeout 10s.

**FED-8.** For `oidc` upstreams the ID token in the token response MUST pass, per
[OIDC Core] section 3.1.3.7:

- signature verification with a key from the upstream's JWKS whose `kid` matches; the algorithm in
  the header MUST be in the allow-list `RS256`, `PS256`, `ES256`, `EdDSA` and match the key type.
  *Conservative choice:* `none` and the `HS*` family are refused, even though OIDC allows HMAC
  with the client secret;
- `iss` exactly equals `expected_issuer`;
- `aud` contains our `client_id`; if it contains other audiences, `azp` is present and equals our
  `client_id`;
- `exp` is in the future and `iat` is not more than 5m in the future, both by `Clock` time with
  60s skew allowed;
- `nonce` equals the intent's nonce.

**FED-9.** Microsoft's multi-tenant endpoints issue tokens with a tenant-specific issuer, so an
exact issuer match needs a fixed tenant. The `microsoft` preset therefore requires a tenant id.
Accepting several tenants means configuring one IdP per tenant.

**FED-10.** If the upstream offers a userinfo endpoint and the mapping needs claims missing from
the ID token, the handler calls userinfo with the access token. The userinfo `sub` MUST equal the
ID token's `sub` ([OIDC Core] section 5.3.4); otherwise the sign-in fails.

**FED-11.** For the `github` preset (`oauth2`), the handler calls the user API with the access
token. The upstream subject is the numeric `id`. The email is taken only from the user-emails API
entry that is both `primary` and `verified`.

**FED-12.** Upstream metadata and JWKS are fetched through `HttpClient` and cached in `Cache` for
24h. An ID token with an unknown `kid` triggers one JWKS refresh, at most once per 5m per IdP, so a
flood of forged tokens cannot make us hammer the upstream.

**FED-13.** A callback with no matching intent, or a reused `state`, is refused with
`/login/error`. IdP-initiated sign-in is not supported.

## 6. Claim mapping

**FED-14.** The mapping is declarative: each target field names one upstream claim (a top-level
claim name, or a dotted path for nested objects). No scripts. Defaults for `oidc`:

| Target | Upstream claim |
|---|---|
| Upstream subject | `sub` (always; not configurable) |
| `email` | `email` |
| Email verified by upstream | `email_verified` (a missing value means `false`) |
| `given_name`, `family_name`, `display_name` | `given_name`, `family_name`, `name` |
| `locale` | `locale` |
| Suggested login name | `preferred_username`, else `email` |

**FED-15.** Upstream groups or roles are not mapped in Phase 5. Mapping them to project roles is a
Phase 4 extension ([14](14-orgs-projects-roles.md)) and needs its own rules for removal.

## 7. Resolving the user

**FED-16.** The key is `(instance, idp_id, upstream subject)`, looked up in the `external_ids`
index (subject stored as an HMAC, since it is personal). If it finds a user:

- the user must be `active` and belong to the IdP's org, or to an org that enables the
  instance-level IdP;
- with `update_profile_on_login`, changed mapped fields append `user.profile_changed`;
- the sign-in continues.

If it finds nothing, the handler offers linking (FED-17, FED-18) or creation (FED-19), or refuses
when neither is allowed with "There is no account for this sign-in. Ask your administrator."

### 7.1 Linking

**FED-17.** *Conservative choice.* An external identity is never linked to an existing account
just because an email address matches. A link is made only when the person proves control of the
existing account by signing in to it, with its own credentials and, under the org's policy, its
second factor, in the same flow. The handler shows "An account already exists. Sign in to it to
connect {IdP}." only when all of these hold:

- the IdP has `allow_linking`;
- the upstream asserts the email as verified;
- the email's domain is in the IdP's `trusted_email_domains`;
- exactly one active user in the org has that email, verified.

Otherwise it does not mention any existing account, so the step reveals nothing about which
emails have accounts.

**FED-18.** A signed-in user can also link an IdP from `/account` (re-authentication first,
SES-13), which starts the same flow with the intent marked `link`. Linking appends
`user.external_id_linked`. Unlinking appends `user.external_id_unlinked` and is refused if the user
would have no other way to sign in (as MFA-36).

### 7.2 Just-in-time provisioning

**FED-19.** With `allow_creation`, an unknown upstream identity creates a user:

- in the IdP's org, or for an instance-level IdP in the org resolved for the login
  ([02](02-tenancy.md) section 4);
- refused if `allowed_email_domains` is non-empty and the email's domain is not listed;
- with the login name from the mapping, after normalization; if taken, the user picks another on
  a short form;
- with the email marked verified only when the upstream asserts it and the domain is in
  `trusted_email_domains`;
- with no password; the user may add one or a passkey later.

The batch appends `user.created` and `user.external_id_linked`, claiming the `login_names` and
`external_ids` index keys (ES-3), so a concurrent duplicate creation fails cleanly.

**FED-20.** Org membership of JIT users is `member` unless the org sets another default role.
Roles beyond that are not assigned in Phase 5.

## 8. Assurance, MFA and logout

**FED-21.** *Conservative choice.* With the IdP's `trust_upstream_mfa: false` (the default), a
federated factor adds no `amr` value (RFC 8176 has none for federation, so the claim is omitted
when it would be empty) and the session stays at `urn:loams:aal1`. Upstream `amr` and `acr` are not
copied, because we cannot verify how the upstream authenticated the user. With
`trust_upstream_mfa: true`, the upstream ID token's `amr` values that are registered in
[RFC 8176] are passed through, and when they include `mfa` the session reaches `urn:loams:aal2`
(never `urn:loams:aal2:phr`). The admin API warns when this is turned on.

**FED-22.** Under the org's `mfa: required_for_local`, a federated sign-in does not need a local
second factor. Under `mfa: required`, it does, unless FED-21's trust applies.

**FED-23.** Logout is not propagated. Ending a session here does not end the upstream session,
and an upstream logout does not end ours. The signed-out page says "You may still be signed in to
{IdP}." when the session's factors included a federated one. Upstream back-channel logout may be added
later.

## 9. Determinism and crates

**FED-24.** The relying-party side uses `openidconnect` 4 and `oauth2` 5 (ADR 0010), with:

- their asynchronous HTTP client trait implemented over our `HttpClient`, so the simulator scripts
  upstream responses and failures;
- the ID token verifier's time source set to a closure that reads `Clock`
  (`IdTokenVerifier::set_time_fn`), and the issue-time check set the same way;
- `state`, `nonce` and the PKCE verifier built from `Rng` bytes with the crates' constructors that
  take a caller-supplied value. The crates' `new_random` constructors are added to the
  disallowed-methods list (ARCH-8).

The implementer confirms each of these hooks in the crates' documentation before relying on it.
Any part of either crate that reads the system clock or the OS random source and cannot be
redirected is wrapped so the simulation implementation of the federation trait does not call it.

## 10. Tests that cover this file

- Mix-up: a callback carrying another IdP's `iss`, or an ID token from another issuer, is refused
  (FED-6, FED-8).
- Replay: a reused `state`, a callback in another browser, an expired intent (FED-6, FED-13).
- ID token: `alg: none`, `HS256`, unknown `kid`, wrong `aud`, multiple `aud` without `azp`, wrong
  `nonce`, expired (FED-8).
- Linking: an unverified or untrusted-domain email never triggers the linking prompt, and no
  account is linked without the existing account's credentials (FED-17).
- JIT race: two concurrent first sign-ins for one subject create one user (FED-19).
- Cross-tenant: an org-level IdP of org A cannot sign users into org B (FED-16).

## Sources

- [OIDC Core]: OpenID Connect Core 1.0, sections 3.1.2 (authorization request), 3.1.3.7 (ID
  token validation), 5.3.4 (userinfo `sub` check).
- [OIDC Discovery]: OpenID Connect Discovery 1.0 (upstream metadata).
- [RFC 6749]: OAuth 2.0 (authorization code flow).
- [RFC 7636]: Proof Key for Code Exchange (PKCE).
- [RFC 8176]: Authentication Method Reference Values.
- [RFC 9207]: Authorization Server Issuer Identification (mix-up defence).
- [RFC 9700]: OAuth 2.0 Security Best Current Practice, section 4.4 (mix-up attacks).
- Provider documentation for preset endpoints: [Google OIDC], [Microsoft identity platform],
  [GitHub OAuth apps], [GitLab OIDC].
- Crate documentation: [openidconnect] (`IdTokenVerifier`, `set_time_fn`, `set_allowed_algs`),
  [oauth2] (`AsyncHttpClient`).
- Loams design 19 section 6 (OIDC single sign-on with JIT provisioning and allowed email domains),
  a Loams-side contract.
- Zitadel public docs, observable behaviour only: a product of this kind brokers OIDC and OAuth
  providers with templates for Google, GitHub and others, offers automatic creation and linking
  settings and an automatic profile update setting, and notes that GitHub is OAuth-only
  ([Zitadel identity providers], [Zitadel identity brokering]). The linking rules, trust flags and
  flow design here are ours.

[OIDC Core]: https://openid.net/specs/openid-connect-core-1_0.html
[OIDC Discovery]: https://openid.net/specs/openid-connect-discovery-1_0.html
[RFC 6749]: https://www.rfc-editor.org/rfc/rfc6749
[RFC 7636]: https://www.rfc-editor.org/rfc/rfc7636
[RFC 8176]: https://www.rfc-editor.org/rfc/rfc8176
[RFC 9207]: https://www.rfc-editor.org/rfc/rfc9207
[RFC 9700]: https://www.rfc-editor.org/rfc/rfc9700
[Google OIDC]: https://developers.google.com/identity/openid-connect/openid-connect
[Microsoft identity platform]: https://learn.microsoft.com/en-us/entra/identity-platform/v2-protocols-oidc
[GitHub OAuth apps]: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps
[GitLab OIDC]: https://docs.gitlab.com/integration/openid_connect_provider/
[openidconnect]: https://docs.rs/openidconnect
[oauth2]: https://docs.rs/oauth2
[Zitadel identity providers]: https://zitadel.com/docs/guides/integrate/identity-providers/introduction
[Zitadel identity brokering]: https://zitadel.com/docs/concepts/features/identity-brokering
