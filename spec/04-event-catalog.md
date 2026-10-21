# 04. Event catalog

Every change to an aggregate is an event. This file defines the envelope every event shares, then
lists every event type with its payload and its consumers. Names and payloads are our own.

## 1. Envelope

```json
{
  "event_id": "0192f7c4-...",
  "instance_id": "0192f7c0-...",
  "org_id": "0192f7c1-...",
  "stream": { "type": "user", "id": "0192f7c3-..." },
  "version": 7,
  "position": 104233,
  "type": "user.password_changed",
  "schema": 1,
  "occurred_at": "2026-10-21T10:15:02.123456789Z",
  "actor": { "kind": "user", "id": "0192f7c3-...", "act": [] },
  "request_id": "0192f7c5-...",
  "causation_id": null,
  "data": { },
  "personal": { "key_id": "...", "nonce": "...", "ciphertext": "..." }
}
```

| Field | Rules |
|---|---|
| `event_id` | UUIDv7. Unique in the store. |
| `instance_id`, `org_id` | `org_id` is `null` only for `instance.*` and `signing_key.*` events. |
| `stream` | The aggregate type and id. |
| `version` | Position in the stream, from 1, without gaps. |
| `position` | Global order in the instance's log, assigned by the store at commit, strictly increasing but possibly with gaps. Not part of the hashed or signed content. |
| `type` | `<aggregate>.<past_tense_verb_phrase>`, lower case with underscores. |
| `schema` | Payload schema version of this type, from 1. |
| `occurred_at` | `Clock` time at decision. |
| `actor` | Who caused it: `kind` is `user`, `machine`, `system` (a background job) or `anonymous` (an unauthenticated request, for example a failed login); `act` is the actor chain when acting on behalf of someone. |
| `request_id` | The request that caused it; `null` for system jobs. |
| `causation_id` | The event that caused this one, for fan-out events (for example `token_family.revoked` caused by `grant.revoked`). |
| `data` | Payload fields that are not personal. |
| `personal` | Personal fields, encrypted with the subject's data key (DM-6); absent when there are none. |

**EV-1.** An event payload is immutable once appended. A change of shape is a new `schema`
version with an upcaster from every earlier version (ADR 0004).

**EV-2.** Readers MUST ignore unknown fields in `data`. Writers MUST NOT remove or change the
meaning of a field within one schema version.

**EV-3.** Payloads never contain a secret in plain form (DM-4). They may contain its hash only
where the aggregate needs it to decide later (for example `code_hash`).

## 2. Categories

| Category | Stored as | Replayed into state | Exported |
|---|---|---|---|
| Domain events | The aggregate's stream | Yes | Per the table's "Export" column |
| Observations | The instance's `audit` stream, append-only, no expected version | No | To audit sinks only |

Observations record things that do not change state but matter for audit and abuse detection,
such as a failed password check. Lockout counters are kept in the `MetaStore` (they change on
every failure and are not worth a domain event each), and only crossing the threshold appends the
domain event `user.locked`.

Export column values: **sync** goes to identity-sync and the audit sinks; **audit** goes to the
audit sinks only; **none** stays inside.

## 3. Domain events

### 3.1 Instance

| Type | `data` | `personal` | Export |
|---|---|---|---|
| `instance.created` | `slug`, `issuer`, `hosts`, `settings` | | audit |
| `instance.host_added` / `instance.host_removed` | `host` | | audit |
| `instance.settings_changed` | `changes` (a JSON merge patch of InstanceSettings) | | audit |
| `instance.default_org_set` | `org_id` | | audit |
| `instance.suspended` / `instance.resumed` | `reason` | | audit |
| `instance.removed` | | | audit |

### 3.2 Org

| Type | `data` | `personal` | Export |
|---|---|---|---|
| `org.created` | `name` | | sync |
| `org.renamed` | `name` | | sync |
| `org.deactivated` / `org.reactivated` | `reason` | | sync |
| `org.removed` | | | sync |
| `org.domain_added` | `domain`, `verification_method`, `token_hash` | | audit |
| `org.domain_verified` | `domain` | | audit |
| `org.domain_verification_failed` | `domain`, `reason` | | audit |
| `org.domain_primary_set` | `domain` | | audit |
| `org.domain_removed` | `domain` | | audit |
| `org.settings_changed` | `changes` (merge patch of OrgSettings) | | audit |
| `org.member_added` | `user_id`, `roles` | | sync |
| `org.member_roles_changed` | `user_id`, `roles` (the full new set) | | sync |
| `org.member_removed` | `user_id` | | sync |

### 3.3 User

| Type | `data` | `personal` | Export | Phase |
|---|---|---|---|---|
| `user.created` | `kind`, `login_name_hmac`, `machine` (name, description, purpose, agent profile) | `login_name`, `human` profile | sync | 1 |
| `user.profile_changed` | | changed profile fields | audit | 1 |
| `user.login_name_changed` | `login_name_hmac` | `login_name` | audit | 1 |
| `user.email_changed` | `verified: false` | `email` | audit | 1 |
| `user.email_verified` | | `email` | audit | 1 |
| `user.phone_changed` / `user.phone_verified` | | `phone` | audit | 2 |
| `user.password_set` | `phc`, `must_change` | | audit | 1 |
| `user.password_reset_requested` | `code_hash`, `expires_at` | | audit | 1 |
| `user.locked` | `factor` (`password`, `otp`, `all`), `until` (or `null` for admin unlock) | | audit | 1 |
| `user.unlocked` | `factor`, `by` (`admin`, `expiry`, `reset`) | | audit | 1 |
| `user.deactivated` / `user.reactivated` | `reason` | | sync | 1 |
| `user.removed` | | | sync | 1 |
| `user.erased` | `data_key_destroyed: true` | | sync | 1 |
| `user.passkey_registered` | `credential_id`, `public_key`, `sign_count`, `backup_eligible`, `backup_state`, `transports`, `aaguid`, `name` | | audit | 2 |
| `user.passkey_used` | `credential_id`, `sign_count`, `backup_state` | | none | 2 |
| `user.passkey_removed` | `credential_id` | | audit | 2 |
| `user.totp_added` | `secret_enc`, `algorithm`, `digits`, `period` | | audit | 2 |
| `user.totp_confirmed` / `user.totp_removed` | | | audit | 2 |
| `user.totp_step_used` | `step` | | none | 2 |
| `user.recovery_codes_generated` | `hashes` (10) | | audit | 2 |
| `user.recovery_code_used` | `index` | | audit | 2 |
| `user.machine_key_added` | `key_id`, `public_jwk`, `alg`, `expires_at` | | audit | 3 |
| `user.machine_key_removed` | `key_id` | | audit | 3 |
| `user.client_secret_added` | `secret_id`, `hash`, `expires_at` | | audit | 3 |
| `user.client_secret_removed` | `secret_id` | | audit | 3 |
| `user.pat_created` | `pat_id`, `hash`, `scopes`, `audience`, `expires_at` | | audit | 3 |
| `user.pat_revoked` | `pat_id` | | audit | 3 |
| `user.agent_owner_changed` | `owner_user_id` | | sync | 3 |
| `user.agent_owner_lost` | `previous_owner_user_id` | | sync | 3 |
| `user.agent_settings_changed` | `token_ttl_cap`, `loams_project_ref` | | sync | 3 |
| `user.agent_trust_policy_set` / `user.agent_trust_policy_removed` | `policy_id`, `policy` | | audit | 3 |
| `user.metadata_set` / `user.metadata_removed` | `key` | `value` | audit | 4 |
| `user.external_id_linked` / `user.external_id_unlinked` | `idp_id` | `subject` | audit | 5 |

### 3.4 Project

| Type | `data` | Export | Phase |
|---|---|---|---|
| `project.created` | `name` | sync | 1 |
| `project.renamed` | `name` | sync | 1 |
| `project.deactivated` / `project.reactivated` / `project.removed` | | sync | 1 |
| `project.role_added` / `project.role_changed` | `key`, `display_name`, `group` | sync | 4 |
| `project.role_removed` | `key` | sync | 4 |
| `project.settings_changed` | `changes` | audit | 4 |

### 3.5 Application

| Type | `data` | Export |
|---|---|---|
| `application.created` | the full client configuration (section 2.5 of [03](03-data-model.md)) except secrets | audit |
| `application.configuration_changed` | `changes` (merge patch) | audit |
| `application.secret_added` | `secret_id`, `hash`, `expires_at` | audit |
| `application.secret_removed` | `secret_id` | audit |
| `application.keys_set` | `jwks` or `jwks_uri` | audit |
| `application.deactivated` / `application.reactivated` / `application.removed` | | audit |

### 3.6 Grant

| Type | `data` | Export |
|---|---|---|
| `grant.created` | `user_id`, `client_id`, `scopes` | audit |
| `grant.scopes_changed` | `scopes` | audit |
| `grant.revoked` | `reason` (`user`, `admin`, `client_removed`, `user_removed`) | audit |

### 3.7 AuthRequest (ephemeral stream)

| Type | `data` | Export |
|---|---|---|
| `auth_request.created` | validated request parameters (section 2.7 of [03](03-data-model.md)), `pushed` (true when it came through PAR, [20](20-later-standards.md)); `login_hint` goes in `personal` | none |
| `auth_request.authenticated` | `user_id`, `session_id`, `auth_time`, `amr`, `acr` | none |
| `auth_request.consented` | `scopes`, `grant_id` | none |
| `auth_request.code_issued` | `code_hash`, `expires_at` | none |
| `auth_request.code_redeemed` | `family_id` | audit |
| `auth_request.code_replayed` | `family_id` | audit |
| `auth_request.failed` | `error` (protocol error code) | none |

### 3.8 Session

| Type | `data` | `personal` | Export |
|---|---|---|---|
| `session.created` | `user_id`, `cookie_hash`, `idle_expires_at`, `absolute_expires_at` | `user_agent`, `ip` | audit |
| `session.factor_verified` | `method` (`password`, `passkey`, `totp`, `email_otp`, `sms_otp`, `recovery_code`, `federated`), `details` (passkey credential id and `uv`, or the IdP id), `acr_contribution` | | audit |
| `session.touched` | `last_active_at`, `idle_expires_at` | | none |
| `session.participant_added` | `client_id` | | none |
| `session.ended` | `reason` (`logout`, `user`, `admin`, `user_deactivated`, `replaced`) | | audit |
| `session.expired` | `kind` (`idle`, `absolute`) | | audit |

### 3.9 TokenFamily

| Type | `data` | Export |
|---|---|---|
| `token_family.created` | `origin` (`code`, `refresh_exchange`, `client_credentials`, `jwt_bearer`, `device`, `token_exchange`, `pat`), `grant_id`, `client_id`, `subject`, `act`, `session_id`, `agent_session_id`, `scopes`, `audience`, `auth_time`, `amr`, `acr`, `refresh_hash` (or `null`), `access_token_id`, `parent_family_id` (vending and exchange, [13](13-machine-identity.md)), `federated_subject` (workload identity), `idle_expires_at`, `absolute_expires_at` | audit |
| `token_family.rotated` | `generation`, `refresh_hash`, `access_token_id`, `scopes` (may narrow) | none |
| `token_family.access_token_issued` | `access_token_id`, `expires_at` (for families without refresh, such as client credentials re-issuance) | none |
| `token_family.access_token_revoked` | `access_token_id`, `exp` ([08](08-tokens.md) TOK-25) | sync |
| `token_family.reuse_detected` | `generation_presented` | audit |
| `token_family.revoked` | `reason` (`reuse_detected`, `client`, `user`, `admin`, `grant_revoked`, `session_ended`, `user_deactivated`, `code_replayed`, `agent_session_ended`, `client_removed`), `access_token_ids`, `latest_access_token_exp` | sync |
| `token_family.expired` | `kind` (`idle`, `absolute`) | none |

### 3.10 SigningKey

| Type | `data` | Export |
|---|---|---|
| `signing_key.created` | `alg`, `key_handle`, `public_jwk` | audit |
| `signing_key.activated` | | audit |
| `signing_key.retiring` | | audit |
| `signing_key.retired` | | audit |

### 3.11 DeviceAuthorization `[P3]` (ephemeral stream)

| Type | `data` | `personal` | Export |
|---|---|---|---|
| `device_authorization.created` | `client_id`, `scope`, `device_code_hash`, `interval`, `expires_at` | `user_code` | none |
| `device_authorization.approved` | `user_id`, `session_id` | | audit |
| `device_authorization.denied` | `user_id` | | audit |
| `device_authorization.redeemed` | `family_id` | | audit |

### 3.12 AgentSession `[P3]`

| Type | `data` | Export |
|---|---|---|
| `agent_session.started` | `agent_user_id`, `owner_user_id`, `purpose`, `expires_at` | sync |
| `agent_session.ended` | `reason` (`agent`, `owner`, `admin`, `expired`) | sync |

### 3.13 Later

Management roles (from Phase 1) add `instance.admin_added`, `instance.admin_roles_changed`,
`instance.admin_removed` and `project.admin_added`, `project.admin_roles_changed`,
`project.admin_removed` ([14](14-orgs-projects-roles.md)). Phase 4 adds `project_grant.*`,
`role_assignment.*`, `webhook_target.*` and `webhook_target.failing` ([14](14-orgs-projects-roles.md),
[15](15-loams-integration.md)). Phase 5 adds
`identity_provider.*` (FED-3) and `idp_intent.started`, `idp_intent.succeeded`, `idp_intent.failed` ([16](16-federation.md)).

## 4. Observations (audit stream)

| Type | `data` | `personal` |
|---|---|---|
| `observed.password_check_failed` | `user_id` (or `null` if unknown), `reason` (`wrong`, `locked`, `unknown_user`) | `login_name_typed`, `ip` |
| `observed.otp_check_failed` | `user_id`, `method` | `ip` |
| `observed.passkey_check_failed` | `user_id`, `reason` | `ip` |
| `observed.client_auth_failed` | `client_id`, `method`, `reason` | `ip` |
| `observed.rate_limited` | `limit` (name), `key_kind` | `ip` |
| `observed.token_rejected` | `endpoint`, `reason` (`expired`, `signature`, `revoked`, `wrong_issuer`, `wrong_audience`) | |

**EV-4.** Observations for unknown users record the typed login name only in `personal`,
encrypted with the instance data key, so a typo that is someone's password is not stored in
plain text.

## 5. Consumers

| Consumer | Reads | Notes |
|---|---|---|
| Inline indexes | Every domain event, inside the append transaction | [05](05-event-store-and-projections.md) section 4 |
| Read-model projections | Domain events by global position | Admin lists, session lists, JWKS document cache |
| Fan-out workers | `grant.revoked`, `user.deactivated`, `user.removed`, `session.ended`, `agent_session.ended`, `application.removed`, `org.deactivated` | Append the follow-on revocation events ([05](05-event-store-and-projections.md) section 3.3) |
| Outbox → identity-sync | Events marked **sync** | [15](15-loams-integration.md) section 3 |
| Outbox → audit sinks | Events marked **sync** or **audit**, and observations | [15](15-loams-integration.md) section 6 |

**EV-5.** Before any event leaves the store through a sink, the exporter removes every field of
class `secret` (hashes, encrypted secrets, key handles; DM-5) and replaces `personal` with
decrypted fields only for sinks configured to receive personal data. Each event type's Rust
payload struct marks its secret fields, and a test asserts that no exported JSON contains a
field so marked.

## Sources

- Our own design. The general pattern (event-sourced aggregates, projections, an outbox) is
  standard practice and is not taken from any one product.
- [RFC 7386]: JSON Merge Patch (the `changes` payloads).
- Zitadel public docs, for the observable fact that a product of this kind records sign-ins,
  membership changes and token creation as events usable for audit, and that token exchange with
  an actor records an extra impersonation event ([Token exchange]). Event names and payloads are
  ours.

[RFC 7386]: https://www.rfc-editor.org/rfc/rfc7386
[Token exchange]: https://zitadel.com/docs/guides/integrate/token-exchange
