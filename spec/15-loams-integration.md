# 15. Loams integration

This file is the contract between loams-identity and Loams: what Loams can rely on in tokens,
how Loams discovers and verifies them, the event feed Loams turns into OpenFGA tuples, the
authorizer shape both sides share, the audit record shape, and how the service is deployed.

This file owns the Loams private claim names (ADR 0017). Other spec files refer to it.

## 1. Division of labour

| Concern | loams-identity | Loams |
|---|---|---|
| Who is this? (authentication, sessions, MFA, passkeys, federation) | Yes | No |
| Identity lifecycle (users, machine users, agents, orgs, memberships) | Yes, source of truth | Consumes events |
| Coarse roles (management roles, project role claims) | Yes | May read claims |
| Resource-level authorization (`can_*` on projects, environments, databases) | No | Yes, OpenFGA behind `Authorizer` |
| Resource tuples (project → org, environment → project) | No | Yes, control-plane outbox |
| Loams' own short-lived tokens (environment-bound, vended to sandboxes) | No (unless the owner decides otherwise, section 2.4) | Yes |

**LOAMS-1.** loams-identity never writes OpenFGA tuples itself. It exports events; Loams'
identity-sync service writes tuples. This keeps one writer per tuple family.

## 2. Token contract

### 2.1 Access tokens

JWT per [RFC 9068], signed with the instance's active EdDSA key by default
([06](06-keys-and-jose.md)).

Header: `{"typ": "at+jwt", "alg": "EdDSA", "kid": "<key id>"}`.

| Claim | Type | When | Meaning |
|---|---|---|---|
| `iss` | string | Always | The instance issuer (TEN-1) |
| `sub` | string | Always | The principal's user id (human or machine) |
| `aud` | string or array | Always | Resource identifiers of the target resource servers ([RFC 8707]) |
| `exp`, `iat`, `nbf` | NumericDate | Always | `nbf` equals `iat` |
| `jti` | string | Always | Unique token id (UUIDv7) |
| `client_id` | string | Always | The client that obtained the token |
| `scope` | string | When any scope was granted | Space-separated, [RFC 9068] section 2.2.3 |
| `auth_time`, `acr`, `amr` | | When a human authenticated in the flow | As in [09](09-oidc.md) |
| `sid` | string | Session-bound families only (TOK-14) | The browser session id; Loams does not need it |
| `act` | object | When an actor is involved | [RFC 8693] section 4.1 chain; each level has `sub`, `principal`, and for agents `agent_owner` |
| `may_act` | object | Tokens from an agent-bound application | [RFC 8693] section 4.4; names the agent allowed to act ([13](13-machine-identity.md) MACH-41) |
| `org` | string | Always | The subject's org id |
| `principal` | string | Always | `user`, `service_account` or `agent` |
| `agent_owner` | string | When `principal` is `agent` | The owner's user id |
| `agent_session` | string | When bound to an agent session | The agent session id ([13](13-machine-identity.md) MACH-39) |
| `roles` | object | `[P4]`, when selected | [14](14-orgs-projects-roles.md) ORG-21 |

**LOAMS-2.** The names `org`, `principal`, `agent_owner`, `agent_session`, `roles` and
`metadata` are private claim names ([RFC 7519] section 4.3) agreed between this service and
Loams. They are stable within API version 1; a change is a new name, never a new meaning for an
old one.

**LOAMS-3.** Loams' own vended tokens (Loams design 19 section 5.3) use `scp` and `env`. Tokens
from this service use `scope` ([RFC 9068]) and carry no `env`. Loams' verifier MUST accept
`scope` on tokens from this issuer. This is a change on the Loams side, recorded here so it is
not missed.

### 2.2 How Loams verifies an access token

**LOAMS-4.** A Loams resource server (gateway, API, MCP endpoint) verifies a JWT access token
from this service as follows. This list is what this service promises will work; Loams may add
checks.

1. Reject unless the header `typ` is `at+jwt` (case-insensitive, with or without the
   `application/` prefix, [RFC 9068] section 4). This stops an ID token being replayed as an
   access token.
2. Reject unless `alg` is the one configured for this resource server (TOK-2: `EdDSA` by
   default, or `ES256` or `RS256` when the resource server chose it). Never accept `none` or
   `HS*`. Pinning one algorithm per resource server, rather than accepting any the issuer
   supports, removes algorithm confusion on the consumer side.
3. Find the key by `kid` in the JWKS at the discovery document's `jwks_uri`, and check that its
   `kty` and `crv` match the `alg`. Cache the JWKS; on an unknown `kid`, refetch at most once per
   5 minutes. Keys are published at least 24 hours before they sign (KEY-5), so an unknown `kid`
   is rare.
4. Check `iss` equals the configured issuer exactly (no normalization).
5. Check the audience, which depends on the phase (TOK-5, TOK-6):
   - **Phases 1 and 2.** `aud` is the requesting client's id (plus the issuer when `openid` was
     granted). Loams keeps a configured list of the client ids whose tokens it accepts (for
     example the console's and the CLI's) and checks that `client_id` is in the list and that
     `aud` contains that `client_id`.
   - **Phase 3 on.** Loams is registered as a `resource_server` and checks that `aud` contains
     one of its own resource identifiers. The client-id list is then only a further restriction.
6. Check `exp` and `nbf` with at most 60 seconds of skew.
7. Map to Loams' `Subject` (section 4.1).

**LOAMS-5.** Opaque access tokens (`lia_`) and PATs (`lip_`) are verified by introspection
([08](08-tokens.md)), with the resource server authenticating as a `resource_server`
application. Introspection responses carry the same claims as the JWT form.

**LOAMS-6.** Revocation latency. A JWT access token stays valid at a resource server until it
expires (15 minutes by default, and at most the agent's TTL cap). For faster cut-off, Loams keeps
a deny set fed by the sync events in section 3: principals that were deactivated or removed, and
the `jti`s listed in `token_family.revoked` and `token_family.access_token_revoked` (TOK-25).
Both are exported to the sync feed for this reason; `token_family.revoked` carries the family's
`access_token_ids` and the latest access-token `exp`, so the deny set can drop entries once they
have expired anyway.

### 2.3 Discovery for MCP clients and other resource-server clients

**LOAMS-7.** This service publishes authorization server metadata at
`/.well-known/oauth-authorization-server` ([RFC 8414]) and OpenID Provider metadata at
`/.well-known/openid-configuration`, with the same content where they overlap. That satisfies the
MCP authorization specification's requirement that an authorization server provide at least one
of the two ([MCP authorization]). The metadata includes:

- `code_challenge_methods_supported: ["S256"]`;
- `authorization_response_iss_parameter_supported: true` ([RFC 9207]);
- `scopes_supported` including `offline_access`;
- `client_id_metadata_document_supported: true` once Client ID Metadata Documents ship
  ([20](20-later-standards.md) section 4).

**LOAMS-8.** `[P3]` The authorization endpoint and the token endpoint accept the `resource`
parameter ([RFC 8707]), as the MCP specification requires of clients. A value must equal one of
the `resource_identifiers` of an active `resource_server` application in the client's org (or a
granted org's, Phase 4). The issued access token's `aud` is exactly the requested resources, plus
the issuer when `openid` is in scope (TOK-6). An unknown value fails with `invalid_target`. A
request with no `resource` uses the client's `default_resource` if it has one
([03](03-data-model.md) section 2.5), and otherwise falls back to the Phase 1 audience (TOK-5).

**LOAMS-9.** Loams, not this service, serves the protected resource metadata ([RFC 9728]) at its
own origin, with `authorization_servers` naming this service's issuer. Each Loams install that
accepts tokens from this service registers itself as a `resource_server` application whose
`resource_identifiers` are its canonical URIs (for example `https://loams.example.com` and
`https://loams.example.com/mcp`).

### 2.4 Environment narrowing (open decision)

Loams' own tokens are bound to one environment (`env`). This service does not know Loams'
environments. Until the owner decides otherwise, Loams keeps vending environment-bound tokens
itself, exchanging a token from this service for one of its own (Loams design 19 section 5.2,
flow 3). The alternative, where this service learns environments as resources and issues `env`
itself, would make it the only token issuer for Loams; that is an architecture decision for the
owner.

## 3. identity-sync: the event feed

### 3.1 What is exported

Events marked **sync** in [04](04-event-catalog.md) and in [14](14-orgs-projects-roles.md),
plus `token_family.revoked` and `token_family.access_token_revoked` (LOAMS-6), with secret
fields removed (EV-5). Personal fields are
not sent to the sync feed: it carries ids only.

### 3.2 Format

**LOAMS-10.** Each event is a [CloudEvents] 1.0 event in JSON format:

| Attribute | Value |
|---|---|
| `specversion` | `1.0` |
| `id` | The event's `event_id` |
| `source` | The instance issuer URL |
| `type` | `dev.loams.identity.` + the event type, for example `dev.loams.identity.org.member_added` |
| `subject` | `<stream type>/<stream id>`, for example `user/0192f7c3-...` |
| `time` | `occurred_at` |
| `datacontenttype` | `application/json` |
| `data` | The event's `data`, after EV-5 |
| `loamsinstance`, `loamsorg` | Instance and org ids (extension attributes) |
| `loamsversion` | The stream version (integer) |
| `loamsposition` | The global position (integer) |
| `loamsactor` | `<kind>:<id>` of the actor, or `system` |

### 3.3 Push delivery (webhooks)

**LOAMS-11.** A `WebhookTarget` `[P4]` (a smaller fixed target for identity-sync exists from
Phase 1, configured in the process configuration) has: `url`, `event_types` (patterns with a
trailing `*`), `org_filter` (one org, or the whole instance for instance admins), `secrets` (at
most two, for rotation), `include_personal` (false; instance admins only may set it true for audit
sinks), `state` (`active`, `paused`, `failing`).

**LOAMS-12.** Requests are `POST` with `Content-Type: application/cloudevents-batch+json`
(a JSON array of up to 100 events) and are signed per [Standard Webhooks]:

- `webhook-id`: the batch id (UUIDv7), stable across retries of the same batch;
- `webhook-timestamp`: Unix seconds at send time;
- `webhook-signature`: `v1,<base64 HMAC-SHA256(secret, webhook-id + "." + webhook-timestamp + "." + body)>`,
  one entry per active secret, space-separated.

Secrets are `whsec_` followed by base64 of 32 random bytes, shown once at creation.

**LOAMS-13.** Delivery is at least once. A `2xx` response acknowledges the whole batch. Anything
else, or no response within 10 seconds, is a failure; retries back off at 5s, 30s, 2m, 10m, 1h,
then every 6h for 3 days, after which the target is marked `failing`, a
`webhook_target.failing` event is appended and delivery stops until an admin resumes it. Events
are not dropped: the target's cursor stays where it was.

**LOAMS-14.** Order: within one stream, events are delivered in version order, and version
*n + 1* is never delivered before version *n* has been acknowledged. Events of different streams
may arrive in any relative order.

**LOAMS-15.** Receivers MUST verify the signature and reject a `webhook-timestamp` more than 5
minutes from their clock before parsing the body.

### 3.4 Pull feed

**LOAMS-16.** `GET /api/v1/events?after=<position>&limit=<1..1000>&types=<pattern,...>` returns
exported events of the caller's scope in position order, as a CloudEvents batch, with a `next`
cursor. `wait=<seconds, up to 30>` long-polls when nothing is new. The caller needs the
`events.read` permission (granted to `org_admin` and above, and to a machine user that an admin
gives it). This is what identity-sync uses for start-up catch-up and for nightly reconciliation,
and it lets a consumer run without an inbound webhook endpoint.

### 3.5 Consumer rules (normative for Loams' identity-sync)

**LOAMS-17.** Apply each event idempotently, keyed by (`subject`, `loamsversion`). Keep the last
applied version per stream and skip anything older. A duplicate write and a missing delete both
count as success.

**LOAMS-18.** Reconcile nightly: list memberships and machine users through the management API
([17](17-management-api.md)), read the corresponding tuples from OpenFGA, repair the drift and
emit a drift metric. Deprovisioning is the dangerous direction; the push path is the fast path
and reconciliation the backstop.

### 3.6 Event to tuple mapping

Loams' OpenFGA types are `user`, `agent`, `service_account`, `team`, `org`, `project`,
`environment` and the resource types below them (Loams design 76 section 6).

| Event | Tuple action by identity-sync |
|---|---|
| `org.member_added` | Write `org:O#owner@user:U`, `org:O#admin@user:U` or `org:O#member@user:U` for each role held |
| `org.member_roles_changed` | Write tuples for added roles, delete tuples for dropped roles |
| `org.member_removed` | Delete every `org:O#<role>@user:U` |
| `user.created` (purpose `agent`) | Write `agent:A#owner@user:<owner>`; if the agent names a Loams project, write `agent:A#project@project:P` |
| `user.created` (purpose `service_account`) | If it names a Loams project, write `service_account:S#project@project:P` |
| `user.agent_owner_changed` | Replace `agent:A#owner@user:*` with the new owner |
| `user.agent_settings_changed` (project changed) | Replace `agent:A#project@project:*` |
| `user.deactivated`, `user.removed`, `user.erased` (human) | Delete all tuples whose user is `user:U` (read by user, delete in batches of at most 100) |
| `user.deactivated`, `user.removed` (agent or service account) | Delete all tuples for `agent:A` or `service_account:S`; Loams also stops vending tokens for it |
| `user.reactivated` | Re-read the user's memberships through the management API and write them |
| `org.deactivated`, `org.removed` | No blind deletion: Loams' control plane removes the org subtree, because it knows the resources |
| `project.created` | Only if the instance setting `export_project_tuples` is on (default off): write `project:P#org@org:O`. Loams' control plane normally writes project tuples itself (design 76 section 8.2) |
| `agent_session.ended` | Delete any conditional tuples Loams wrote for that session |
| `role_assignment.*` `[P4]` | Not mapped by default; an optional mapping table may map a project role key to a relation on a Loams project |

**LOAMS-19.** The "Loams project" an agent or service account names is an opaque reference
(`loams_project_ref`) that this service stores and exports but does not validate; Loams'
control plane is its source of truth. Identity projects ([14](14-orgs-projects-roles.md)) are
containers for applications and roles and are not the same objects.

## 4. The authorizer shape

### 4.1 Loams' trait and Subject

Loams' authorizer (design 76 section 9.1) has `check`, `batch_check` and `list_objects`. Its
`Subject` is `{ kind: User | ServiceAccount | Agent, id, org, act: [Subject], session }`.

**LOAMS-20.** A token from this service maps to that `Subject` without loss:

| Subject field | From |
|---|---|
| `kind` | `principal` |
| `id` | `sub` |
| `org` | `org` |
| `act` | The `act` chain, outermost first, each level mapped the same way |
| `session` | `agent_session` |

Because every delegated token carries `act`, Loams can enforce "never exceeds either party's
rights" by checking the subject and each actor (MACH-34).

### 4.2 This service's own use

This service uses the same shape for its management API ([14](14-orgs-projects-roles.md)
section 2.3) and ships these implementations:

| Implementation | Use |
|---|---|
| `BuiltinAuthorizer` | Default; answers from the role holders in the aggregates |
| `OpenFgaAuthorizer` | Optional `[P4]`; a model file for the management permissions ships in the repository |
| `AllowAllAuthorizer` | Development only, refused unless every listener is on loopback (ARCH-5) |
| `RecordingAuthorizer` | Tests and audit |

The trait is defined in `loams-identity-core`, not imported from Loams, so neither repository
depends on the other. A shared crate is possible later.

## 5. Outbound connections

### 5.1 Who connects out

Webhook delivery, client `jwks_uri` fetches, workload-identity issuer fetches and (Phase 5)
upstream identity providers.

### 5.2 Transport

Through `HttpClient`, TLS with rustls and the platform's or a configured CA set, HTTP/1.1 or 2.

### 5.3 Outbound URL rules (SSRF protection)

**LOAMS-21.** Every outbound URL that a tenant can configure is checked at configuration time and
again at every connection:

1. Scheme `https`. `http` only to loopback, and only when the instance is not marked
   `production`.
2. Resolve the host once per connection attempt. Refuse if any resolved address is loopback,
   private (RFC 1918), link-local, carrier-grade NAT (100.64.0.0/10), multicast, unspecified, the
   cloud metadata address 169.254.169.254, or an IPv6 unique-local or link-local address, unless
   an instance admin set `allow_private_network` on that target.
3. Connect to the address that was checked (no second resolution), sending the original host name
   for SNI and `Host`.
4. Do not follow redirects.
5. Limit response bodies to 1 MiB for metadata and JWKS, 64 KiB for webhook responses; time out
   connect at 5 seconds and the whole request at 10 seconds.

## 6. Audit records

**LOAMS-22.** Audit sinks receive one record per exported event or observation, shaped to line up
with Loams' `_audit` stream (Loams design 76 section 11):

| Field | From |
|---|---|
| `timestamp` | `occurred_at` |
| `source` | `"loams-identity"` |
| `instance_id`, `org_id` | Envelope |
| `subject` | `{ kind, id }` of the actor (`actor` in the envelope) |
| `act` | The actor chain |
| `action` | The event type, for example `user.password_set` |
| `object` | `{ type: stream type, id: stream id }` |
| `outcome` | `success` for domain events; `failure` or `denied` for observations |
| `request_id` | Envelope |
| `agent_id`, `owner_id`, `session_id` | For agent actors: the agent, its owner and the agent session, when known |
| `details` | The sanitized `data` (EV-5) |
| `personal` | Only for sinks with `include_personal` |

Sinks: JSON lines to the log (always available), webhook targets (section 3.3), and the pull feed
with `types=*` for callers with `audit.read`. Iggy is a later sink.

## 7. Deployment with Loams

**LOAMS-23.** Ostrium Labs hosts one instance at `auth.loams.dev` for the console and the site.
Its OIDC endpoints and login UI are public. Its management and system APIs listen only on the
private listener, reachable over the tailnet. Ostrium hosts nothing else (the console plus
auth), and everything is Apache-2.0.

**LOAMS-24.** A self-hosted Loams install that trusts `auth.loams.dev` uses it only to sign
people in (it is a relying party). It still decides membership and authorization itself, so
Ostrium never sees the install's tuples or resources.

**LOAMS-25.** A self-hosted install that runs its own loams-identity uses the `identity` compose
profile: one loams-identity container with redb in a volume (single node) or PostgreSQL, the
private listener on the compose network only, and identity-sync running inside the `loams`
process reading the pull feed (no inbound webhook port needed). The light default (no profile)
keeps Loams' built-in single-org authentication.

**LOAMS-26.** Loams design 76 chose Zitadel and listed "a Rust-native IdP behind the same traits"
as its optional Phase 6. This project is that IdP. Loams depends only on the standards, on the
claims in section 2 and on the event feed in section 3, so the identity provider can be swapped
without changing Loams' authorization code.

## Sources

- [RFC 7519] section 4.3 (private claims); [RFC 8414]; [RFC 8693] sections 4.1 and 4.4;
  [RFC 8707]; [RFC 9068]; [RFC 9207]; [RFC 9728].
- [MCP authorization]: Model Context Protocol, Authorization, revision 2026-07-28 (authorization
  server discovery, PKCE, `resource`, `iss`, Client ID Metadata Documents).
- [CloudEvents]: CloudEvents 1.0 specification and its JSON format.
- [Standard Webhooks]: the Standard Webhooks specification (headers and signature scheme).
- [OpenFGA docs], for the tuple and relation terms.
- Loams design 76 sections 6, 8.1 (mapping and rules only), 9.1, 10 and 11, and design 19
  sections 5.2 to 5.4, as Loams-side contracts.

[RFC 7519]: https://www.rfc-editor.org/rfc/rfc7519
[RFC 8414]: https://www.rfc-editor.org/rfc/rfc8414
[RFC 8693]: https://www.rfc-editor.org/rfc/rfc8693
[RFC 8707]: https://www.rfc-editor.org/rfc/rfc8707
[RFC 9068]: https://www.rfc-editor.org/rfc/rfc9068
[RFC 9207]: https://www.rfc-editor.org/rfc/rfc9207
[RFC 9728]: https://www.rfc-editor.org/rfc/rfc9728
[MCP authorization]: https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization
[CloudEvents]: https://cloudevents.io/
[Standard Webhooks]: https://www.standardwebhooks.com/
[OpenFGA docs]: https://openfga.dev/docs
