# 17. Management API and CLI

The management API is how admins, the Loams control plane and the CLI change identity state:
orgs, users, machine users, projects, applications, keys and settings. It has a REST/JSON form
and a gRPC form with the same semantics (ADR 0020). A separate system API, for the process
operator only, creates and removes instances (TEN-5).

## 1. Listeners

| Listener | Serves | Default bind |
|---|---|---|
| Public | OAuth/OIDC endpoints, login UI, JWKS, discovery | `0.0.0.0:8443` (or behind a TLS proxy) |
| Private | Management REST (`/api/v1`), management gRPC, system API (`/system/v1` and its gRPC service), OpenAPI document, health, metrics | `127.0.0.1:8444` |

**API-1.** The management API and the system API are served only on the private listener. An
operator may bind the private listener to a private network address (a tailnet, a compose
network), never to a public address: the process refuses to start if the private listener's
address is not loopback, RFC 1918, carrier-grade NAT (100.64.0.0/10, which includes tailnets) or
IPv6 unique-local, unless the configuration sets `private_listener.allow_public = true` and logs
a warning on every start.

**API-2.** The self-service subset (`/api/v1/me/...`, section 6.10) is also served on the public
listener, because users manage their own credentials and sessions from the login UI. It accepts
only the login UI's session cookie with CSRF protection ([11](11-login-and-passwords.md)) or a
user access token, and only the `user.self` permission.

## 2. Authentication

**API-3.** Management calls authenticate with `Authorization: Bearer <token>`, where the token is
one of:

| Token | Accepted when |
|---|---|
| A JWT or opaque access token of this instance, for a human user | Phases 1 and 2: its `client_id` is in the instance setting `management_clients` (the built-in console and CLI clients are listed by default) and its `aud` contains that client id (TOK-5). Phase 3 on: either that rule, or its `aud` contains the management API's resource identifier `<issuer>/api`, a built-in `resource_server` created with the instance (TOK-6) |
| An access token of a machine user `[P3]` | Its `aud` contains `<issuer>/api` |
| A PAT `[P3]` | Its audience is `<issuer>/api` and the `pats` feature is on |

Tokens are validated as "our access token" per KEY-17 and then checked as introspection would
(TOK-20), so a revoked family or a deactivated user is refused at once.

**API-4.** The system API authenticates with a JWT signed by an operator key. Operator public
keys are listed in the process configuration (`system.operator_keys`, a JWKS file); they are not
in the event log. The JWT has `iss` = `sub` = the operator key's `kid`, `aud` =
`urn:loams-identity:system`, `exp` at most 5 minutes after `iat`, and a `jti` held in the replay
cache. The `cli` creates these JWTs from a local private key file. *Conservative choice:* no
tenant credential of any kind reaches the system API.

**API-5.** Failures answer `401` with `WWW-Authenticate: Bearer` per [RFC 6750] section 3, and
`403` for an authenticated caller without the permission, except for the wrong-org rule API-8.

## 3. Authorization

**API-6.** Every REST route and every gRPC method declares, in code, one permission from
[14](14-orgs-projects-roles.md) section 2.2 and an object resolver (how to find the target object
from the path or message). A route without a declaration answers `403`. A test enumerates the
axum router and every tonic service method and fails if any lacks a declaration, or if a declared
permission name is unknown.

**API-7.** The order inside a request is: authenticate (API-3), resolve the tenant scope from
the principal, resolve the target object's `instance_id` and `org_id` from an inline index or the
aggregate (not a projection, ES-14), then call `Authorizer::check`, then load and return or
change the object.

**API-8.** If the target object exists but is outside every scope the caller is authorized for,
the answer is `404`, the same as for an object that does not exist (TEN-17). `403` is used only
when the caller can see the object but lacks the specific permission (for example an
`org_admin` trying to remove an `owner`).

## 4. Conventions (REST)

### 4.1 Representation

- JSON, UTF-8, field names in `snake_case`. Unknown request fields are rejected with `400`, so a
  typo in a security setting cannot be silently ignored.
- Ids are canonical UUID strings. Timestamps are RFC 3339 with `Z`.
- Every resource representation carries `id`, `version` (the aggregate version), `created_at`
  and `updated_at`.
- Secrets (client secrets, PATs, webhook secrets) appear only in the response that creates them,
  in a field named `secret`, with `Cache-Control: no-store`. They can never be read again.

### 4.2 Concurrency

**API-9.** Responses for single resources carry `ETag: "<version>"`. `PATCH`, `PUT` and `DELETE`
on an existing resource require `If-Match` with that value; the server appends with
`Exactly(version)` (ES-2). A mismatch answers `412 Precondition Failed`; a missing `If-Match`
answers `428 Precondition Required`.

**API-10.** `PATCH` bodies are JSON Merge Patch ([RFC 7386]) with `Content-Type:
application/merge-patch+json`.

### 4.3 Idempotent creation

**API-11.** `POST` requests that create a resource accept `Idempotency-Key` (1 to 64 printable
ASCII characters). The server stores (principal, key) → (request body hash, response) in the
`MetaStore` ephemeral space for 24 hours. A repeat with the same body returns the stored response
with the same status; a repeat with a different body answers `422` with problem type
`idempotency_key_reused`. Without the header, creation is not idempotent.

### 4.4 Pagination and filtering

**API-12.** List endpoints take `page_size` (default 50, maximum 500) and `page_token`. They
return `items` and `next_page_token` (absent on the last page). The token is opaque to clients:
it encodes the last key and the tenant scope and is authenticated with an HMAC under an instance
key, so a token minted in one org or instance is refused (`400`) in another.

**API-13.** Lists are ordered by id (UUIDv7, so roughly creation order) unless the endpoint
documents another order. Filters are explicit query parameters per endpoint (for example
`state=active`, `login_name_prefix=`); there is no general query language.

### 4.5 Errors

**API-14.** Errors use problem details ([RFC 9457]), `Content-Type: application/problem+json`:

```json
{
  "type": "urn:loams:identity:problem:validation",
  "title": "The request is not valid",
  "status": 400,
  "detail": "redirect_uris[1] must use https",
  "instance": "urn:uuid:0192f7c5-...",
  "errors": [{ "field": "redirect_uris[1]", "code": "https_required" }]
}
```

`instance` is the request id. Problem types are a closed list in code: `validation`, `not_found`,
`conflict` (a unique name is taken, ES-3), `precondition_failed`, `precondition_required`,
`idempotency_key_reused`, `unauthenticated`, `forbidden`, `rate_limited`, `unavailable`,
`internal`. Details never contain secrets or another org's data.

### 4.6 Rate limits

**API-15.** Per principal: 50 requests per second sustained with bursts of 100 for reads, 10 per
second for writes; per source address before authentication: 20 per second. Over the limit:
`429` with `Retry-After` and problem type `rate_limited`, and an `observed.rate_limited`
observation. Limits are instance settings (ADR 0024).

## 5. gRPC

**API-16.** Protobuf package `loams.identity.v1` for management and `loams.identity.system.v1` for
the system API, our own definitions. One service per resource family (`InstanceService`,
`OrgService`, `UserService`, `ProjectService`, `ApplicationService`, `GrantService`,
`SessionService`, `SigningKeyService`, `AgentSessionService` `[P3]`, `EventService`,
`RoleService` `[P4]`, `WebhookService` `[P4]`, `IdentityProviderService` `[P5]`). Methods mirror
the REST operations one-to-one, with the same permission declarations (API-6).

**API-17.** Credentials in the `authorization` metadata entry. The idempotency key in
`idempotency-key` metadata. Expected versions in an `expected_version` field of update and
delete requests (required, as `If-Match` is for REST). Errors map to gRPC status codes:
validation → `INVALID_ARGUMENT`, not found and wrong org → `NOT_FOUND`, conflict →
`ALREADY_EXISTS`, precondition → `FAILED_PRECONDITION`, unauthenticated → `UNAUTHENTICATED`,
forbidden → `PERMISSION_DENIED`, rate limited → `RESOURCE_EXHAUSTED`, unavailable →
`UNAVAILABLE`, internal → `INTERNAL`, with field errors in `google.rpc.BadRequest` details.

**API-18.** `EventService.Subscribe(after_position, types)` is a server stream with the same
content and permission as the pull feed (LOAMS-16). Server reflection is off by default.

## 6. Resources

Paths are under `/api/v1` unless they start with `/system/v1`. The permission column names
[14](14-orgs-projects-roles.md) section 2.2.

### 6.1 Instances (system API)

| Method and path | Purpose | Phase |
|---|---|---|
| `POST /system/v1/instances` | Create an instance with its first org and first `instance_owner` (by invitation email or a one-time setup link) | 1 |
| `GET /system/v1/instances`, `GET /system/v1/instances/{id}` | List and read | 1 |
| `POST /system/v1/instances/{id}/hosts`, `DELETE .../hosts/{host}` | Add or remove a host name | 1 |
| `POST /system/v1/instances/{id}/suspend`, `.../resume` | TEN-4 | 1 |
| `DELETE /system/v1/instances/{id}` | Remove | 1 |

### 6.2 The current instance

| Method and path | Permission | Phase |
|---|---|---|
| `GET /instance`, `PATCH /instance/settings` | `instance.read`, `instance.manage` | 1 |
| `GET /instance/admins`, `PUT /instance/admins/{user_id}`, `DELETE /instance/admins/{user_id}` | `instance.manage_admins` | 1 |
| `GET /instance/signing-keys` | `instance.manage_keys` | 1 |
| `POST /instance/signing-keys/rotate` `{ "alg": "EdDSA" }` | `instance.manage_keys` | 1 |
| `POST /instance/signing-keys/emergency-rotate` `{ "alg", "reason" }` (KEY-9) | `instance.manage_keys` | 1 |

### 6.3 Orgs, domains and members

| Method and path | Permission | Phase |
|---|---|---|
| `POST /orgs`, `GET /orgs` | `org.create`, `org.read` (lists only orgs the caller can read) | 1 |
| `GET /orgs/{org_id}`, `PATCH /orgs/{org_id}`, `DELETE /orgs/{org_id}` | `org.read`, `org.manage` | 1 |
| `POST /orgs/{org_id}/deactivate`, `.../reactivate` | `org.manage` | 1 |
| `GET /orgs/{org_id}/settings`, `PATCH /orgs/{org_id}/settings` | `org.read`, `org.manage` | 1 |
| `POST /orgs/{org_id}/domains`, `GET .../domains` | `org.manage`, `org.read` | 1 |
| `POST .../domains/{domain}/verify`, `.../primary`, `DELETE .../domains/{domain}` | `org.manage` | 1 |
| `GET /orgs/{org_id}/members`, `PUT /orgs/{org_id}/members/{user_id}` `{ "roles": [...] }`, `DELETE .../members/{user_id}` | `org.read`, `org.manage_members` or `org.manage_owners` when `owner` is involved | 1 |

### 6.4 Users

| Method and path | Permission | Phase |
|---|---|---|
| `POST /orgs/{org_id}/users` (human), `GET /orgs/{org_id}/users` | `user.manage`, `user.read` | 1 |
| `POST /orgs/{org_id}/machine-users` | `user.manage` | 3 |
| `GET /users/{id}`, `PATCH /users/{id}`, `DELETE /users/{id}` | `user.read`, `user.manage` | 1 |
| `POST /users/{id}/deactivate`, `.../reactivate`, `.../unlock`, `.../erase` | `user.manage` | 1 |
| `POST /users/{id}/password-reset` (sends the reset email; never returns a password) | `user.manage` | 1 |
| `POST /users/{id}/revoke-tokens`, `POST /users/{id}/end-sessions` | `user.manage` | 1 |
| `GET /users/{id}/passkeys`, `DELETE /users/{id}/passkeys/{credential_id}` | `user.read`, `user.manage` | 2 |
| `DELETE /users/{id}/totp`, `POST /users/{id}/recovery-codes/invalidate` | `user.manage` | 2 |
| `POST /users/{id}/machine-keys` (public JWK in), `GET`, `DELETE .../machine-keys/{key_id}` | `user.manage`, `user.read` | 3 |
| `POST /users/{id}/client-secrets` (secret out once), `DELETE .../client-secrets/{secret_id}` | `user.manage` | 3 |
| `POST /users/{id}/pats` (token out once), `GET`, `DELETE .../pats/{pat_id}` | `user.manage` | 3 |
| `PUT /users/{id}/agent/owner`, `PATCH /users/{id}/agent` (TTL cap, Loams project reference) | `user.manage` | 3 |
| `PUT /users/{id}/agent/trust-policies/{policy_id}`, `DELETE` | `user.manage` | 3 |
| `GET /users/{id}/grants`, `DELETE /grants/{grant_id}` | `user.read`, `user.manage` | 1 |
| `GET /users/{id}/sessions`, `DELETE /sessions/{session_id}` | `user.read`, `user.manage` | 1 |

**API-19.** Admins can never read or set a user's password, see a TOTP secret or a recovery code,
or download a private key. They can only trigger resets, remove credentials and revoke.

**API-20.** `POST .../erase` performs crypto-shredding (DM-6): it requires the user to be
`removed` first, destroys the data key, and appends `user.erased`. It cannot be undone; the
request must carry `{ "confirm": "<user id>" }`.

### 6.5 Projects, roles and grants

| Method and path | Permission | Phase |
|---|---|---|
| `POST /orgs/{org_id}/projects`, `GET /orgs/{org_id}/projects` | `project.manage`, `project.read` | 1 |
| `GET /projects/{id}`, `PATCH /projects/{id}`, `DELETE /projects/{id}` | `project.read`, `project.manage` | 1 |
| `GET /projects/{id}/roles`, `PUT /projects/{id}/roles/{key}`, `DELETE .../roles/{key}` | `project.read`, `project.manage` | 4 |
| `GET /projects/{id}/admins`, `PUT`/`DELETE .../admins/{user_id}` | `project.manage` | 4 |
| `POST /projects/{id}/grants`, `GET`, `PATCH`/`DELETE /project-grants/{grant_id}` | `project.manage` | 4 |
| `GET /orgs/{org_id}/granted-projects` | `org.read` | 4 |
| `GET /projects/{id}/role-assignments`, `PUT /projects/{id}/role-assignments/{user_id}`, `DELETE` | `role_assignment.manage` (scoped to the caller's grant for granted-org admins, ORG-27) | 4 |

### 6.6 Applications

| Method and path | Permission | Phase |
|---|---|---|
| `POST /projects/{id}/applications`, `GET /projects/{id}/applications` | `application.manage`, `application.read` | 1 |
| `GET /applications/{id}`, `PATCH /applications/{id}`, `DELETE /applications/{id}` | `application.read`, `application.manage` | 1 |
| `POST /applications/{id}/deactivate`, `.../reactivate` | `application.manage` | 1 |
| `POST /applications/{id}/secrets` (secret out once), `DELETE .../secrets/{secret_id}` | `application.manage` | 1 |
| `PUT /applications/{id}/jwks` (inline set or `jwks_uri`) | `application.manage` | 3 |

**API-21.** Application creation and updates validate every field rule in
[03](03-data-model.md) section 2.5 and [07](07-oauth-core.md) (redirect URI forms, auth method
per kind, grant types per kind, MACH-4 for chosen client ids) and fail with field errors rather
than storing an invalid client.

### 6.7 Agent sessions `[P3]`

| Method and path | Permission |
|---|---|
| `POST /agent-sessions` `{ "agent_user_id", "purpose", "expires_at" }` | `agent_session.manage` on the agent |
| `GET /agent-sessions?agent_user_id=` | `agent_session.manage` |
| `DELETE /agent-sessions/{id}` (ends it, MACH-40) | `agent_session.manage` |

### 6.8 Events and audit

| Method and path | Permission | Phase |
|---|---|---|
| `GET /events?after=&limit=&types=&wait=` | `events.read` (LOAMS-16) | 1 |
| `GET /orgs/{org_id}/audit?after=&types=` | `audit.read` (from the `audit_view` projection) | 1 |

### 6.9 Webhook targets `[P4]` and identity providers `[P5]`

| Method and path | Permission |
|---|---|
| `POST /orgs/{org_id}/webhook-targets`, `GET`, `PATCH`/`DELETE /webhook-targets/{id}` | `org.manage` |
| `POST /instance/webhook-targets` (instance-wide) | `instance.manage` |
| `POST /webhook-targets/{id}/resume`, `.../rotate-secret`, `.../test` | `org.manage` |
| `POST /orgs/{org_id}/identity-providers`, `GET`, `PATCH`/`DELETE /identity-providers/{id}` | `org.manage` |

### 6.10 Self-service (`/api/v1/me`)

| Method and path | Phase |
|---|---|
| `GET /me`, `PATCH /me` (profile fields the org allows users to change) | 1 |
| `POST /me/password` (requires the current password or a recent re-authentication) | 1 |
| `POST /me/email` (sends a verification code) | 1 |
| `GET /me/sessions`, `DELETE /me/sessions/{id}`, `POST /me/sessions/end-others` | 1 |
| `GET /me/grants`, `DELETE /me/grants/{id}` (revoke an app's access) | 1 |
| `GET /me/passkeys`, `DELETE /me/passkeys/{id}`, TOTP and recovery-code management | 2 |
| `GET /me/agents` (agents the user owns), `POST /me/agents/{id}/deactivate` | 3 |

**API-22.** Changes to credentials through self-service require a fresh authentication: a factor
verified in the current session within `reauth_for_sensitive` (15 minutes by default,
[03](03-data-model.md) section 3).

## 7. OpenAPI and compatibility

**API-23.** The OpenAPI 3.1 document is generated with utoipa from the route definitions and
served at `/api/v1/openapi.json` on the private listener. A snapshot test (insta) fails on any
change to it, so every API change is reviewed. The protobuf files are checked with a breaking-
change linter in CI.

**API-24.** Version 1 changes are additive only (new fields, new endpoints, new enum values that
old clients may ignore). Removing or changing the meaning of anything needs `/api/v2` and
`loams.identity.v2`.

## 8. CLI

**API-25.** The `loams-identity` binary's subcommands:

| Command | Does |
|---|---|
| `setup` | First-run setup: creates the instance, the default org and its owner, the built-in management resource server and the console and CLI clients; writes the setup token as in [11](11-login-and-passwords.md) section 8 |
| `serve` | Runs the listeners |
| `migrate` | Creates or upgrades the store schema (PostgreSQL) or file format (redb); refuses to downgrade |
| `check` | Validates the configuration file and the reachability of the store, then exits |
| `keys list`, `keys rotate --alg <alg>`, `keys emergency-rotate --alg <alg> --reason <text>` | Signing keys |
| `operator-key generate` | Creates an operator key pair for the system API (API-4), printing only the public half |
| `projections rebuild <name>` | Drops and rebuilds a projection from position 0 (ES-15) |
| `events export --after <pos>` | Writes exported events as CloudEvents JSON lines |
| `admin <resource> <verb> ...` | Thin client over the management API, for example `admin orgs create --name Acme`, `admin apps create --project <id> --kind native ...`; authenticates with the device flow `[P3]` or a token file |

**API-26.** CLI output is human-readable by default and JSON with `--output json`. Secrets are
printed once, to standard output only, never to logs or standard error, followed by a one-line
reminder on standard error that the secret will not be shown again.

## 9. Zitadel-compatible API surface `[P6]`

An optional surface generated from Zitadel's published Apache-2.0 protobuf definitions, so that
existing SDKs can talk to this service, is a stretch goal for Phase 6. It is not specified here.
When it is scheduled, its specification is written from those published definitions and the
public documentation only, under the same clean-room rules (ADR 0001), and it maps onto the
operations in section 6 rather than adding new behaviour.

## Sources

- [RFC 6750]: Bearer Token Usage, section 3 (error responses).
- [RFC 7386]: JSON Merge Patch.
- [RFC 9457]: Problem Details for HTTP APIs.
- [RFC 9110]: HTTP Semantics, sections 13.1.1 (`If-Match`) and 15.5.13 (`412`); [RFC 6585]
  section 3 (`428`) and section 4 (`429`).
- The Idempotency-Key HTTP header field ([Idempotency-Key draft], IETF HTTPAPI working group),
  for the header name and semantics.
- Crate documentation: [utoipa], [tonic], [axum].
- Our own design for paths, permissions, the system API and the CLI. Loams design 76 section 12.3
  (management APIs never public on the hosted instance), as a Loams-side contract.

[RFC 6750]: https://www.rfc-editor.org/rfc/rfc6750
[RFC 7386]: https://www.rfc-editor.org/rfc/rfc7386
[RFC 9457]: https://www.rfc-editor.org/rfc/rfc9457
[RFC 9110]: https://www.rfc-editor.org/rfc/rfc9110
[RFC 6585]: https://www.rfc-editor.org/rfc/rfc6585
[Idempotency-Key draft]: https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/
[utoipa]: https://docs.rs/utoipa
[tonic]: https://docs.rs/tonic
[axum]: https://docs.rs/axum
