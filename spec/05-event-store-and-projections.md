# 05. Event store, indexes and projections

## 1. The `EventStore` contract

```text
append(scope, batch: [StreamWrite], index_ops: [IndexOp]) -> Result<Committed, AppendError>
  StreamWrite = { stream, expected_version: NoStream | Exactly(u64), events: [Event] }
  IndexOp     = Claim{ index, key, owner } | Release{ index, key, owner } | Put{ index, key, value } | Delete{ index, key }
read_stream(scope, stream, from_version) -> [Event]
read_log(instance, from_position, limit) -> [Event]           (global order)
lookup(scope, index, key) -> Option<Value>
snapshot_get / snapshot_put(stream, version, bytes)            (optional optimisation)
```

**ES-1.** `append` is atomic: either every stream write and index operation in the batch commits,
or none does.

**ES-2.** For each `StreamWrite`, if the stream's current version differs from
`expected_version`, the whole batch fails with `AppendError::Conflict { stream, actual }` and
nothing is written.

**ES-3.** A `Claim` on a unique index fails the batch with `AppendError::Taken { index, key }` if
the key is held by a different owner. Claiming a key the same owner already holds succeeds
(idempotent).

**ES-4.** After `append` returns success, `read_stream` and `lookup` in any later call observe
the write (read-your-writes in one process and across processes on the same store).

**ES-5.** `position` values are assigned at commit and are strictly increasing in commit order
within one instance. `read_log` from position *p* returns every event with a position greater
than *p* that has committed, and never returns an event and later returns an event with a
smaller position. *Implementation note:* on PostgreSQL this needs care, because sequence values
are taken before commit; see section 6.2.

**ES-6.** At most 64 events and 64 index operations per batch. A batch spans at most two
streams, except the token-issuing batches in section 3.2, which may touch three.

**ES-7.** Every `AppendError` is typed: `Conflict`, `Taken`, `Unavailable` (retryable),
`Timeout` (outcome unknown), `Corrupt` (not retryable). After `Timeout`, the caller MUST treat the
outcome as unknown and re-read the stream before deciding again (the simulation injects this,
[19](19-verification.md)).

## 2. The `MetaStore` contract

- **Projection tables**: `put(table, scope, key, row)`, `get`, `scan(table, scope, range, limit)`,
  `delete`, plus `checkpoint_get(projection)` and `checkpoint_put(projection, position)` written in
  the same transaction as the rows.
- **Ephemeral key-value**: `incr(scope, key, ttl) -> u64`, `get`, `put_if_absent(scope, key, ttl)`,
  `delete`. Used for rate-limit and lockout counters and for replay caches (client-assertion
  `jti`, [13](13-machine-identity.md)).

**ES-8.** Ephemeral entries expire by comparing their stored expiry with `Clock` time on read,
not by a background timer alone (ARCH-11).

**ES-9.** Every key in both spaces starts with the tenant scope (TEN-13, TEN-14).

## 3. Command patterns

### 3.1 Single aggregate

Load (snapshot plus later events), decide, append with `Exactly(version)`. On `Conflict`, reload
and decide again, at most three times.

### 3.2 Token-issuing batches

Issuing tokens must be atomic with the state change that allows it. These are the batches:

| Operation | Streams written | Index ops |
|---|---|---|
| Consent | `auth_request` (`consented`, expected version) and `grant` (`created` with `NoStream`, or `scopes_changed` with expected version) ([07](07-oauth-core.md) OAUTH-10) | Claim (user, client) in `grants` when created |
| Redeem a code | `auth_request` (`code_redeemed`, expected version), new `token_family` (`created`, `NoStream`), and a read check of the `grant` | Claim `refresh_hash`; put `access_token_id → family`; for opaque access tokens put `opaque_access_hashes` |
| Rotate a refresh token | `token_family` (`rotated`, expected version) | Release old `refresh_hash` → keep it in `retired_refresh` index; claim new hash; put `access_token_id` |
| Reuse detected | `token_family` (`reuse_detected`, `revoked`) | Delete `refresh_hash` entries of the family |
| Client credentials, JWT bearer, token exchange | New `token_family` (`NoStream`), and `agent_session` read-checked by version when bound | Put `access_token_id`; claim `refresh_hash` if any |
| Device code redemption | `device_authorization` (`redeemed`), new `token_family` | As for a code |

**ES-10.** A batch that issues a token under a grant, a session or an agent session MUST include
that aggregate in the batch with its expected version, even when it appends nothing to it
(a *read check*: `StreamWrite` with an empty `events` list). This makes "grant revoked" and
"token issued under the grant" mutually exclusive: if the revocation commits first, the issuing
batch fails with `Conflict` and the retry sees the revoked state. Users and clients are also
read-checked the same way.

### 3.3 Fan-out

Revoking a grant, ending a session, deactivating a user and similar commands affect many
families. The command appends one event (for example `grant.revoked`) and returns. A fan-out
worker reads the log and appends `token_family.revoked` with `causation_id` set, one family at a
time.

**ES-11.** Correctness does not wait for the fan-out. Every token-issuing batch read-checks the
parent aggregates (ES-10), and every introspection and userinfo request checks the state of the
user, client, grant, session (when bound) and family. The fan-out only makes the family records
say so explicitly and drives exports.

**ES-12.** Fan-out workers are idempotent: appending `token_family.revoked` to an already revoked
family is skipped. Their position is a checkpoint in the `MetaStore`. A crash between the append
and the checkpoint causes a repeat, which the idempotency absorbs.

## 4. Inline indexes

Maintained in the same transaction as the events (ES-1).

| Index | Key | Value | Unique |
|---|---|---|---|
| `instance_hosts` | host | instance id | Yes (global) |
| `instance_slugs` | slug | instance id | Yes (global) |
| `org_names` | (instance, normalized name) | org id | Yes |
| `org_domains` | (instance, domain) | org id | Yes |
| `login_names` | (instance, HMAC of normalized login name) | user id | Yes |
| `verified_emails` | (instance, HMAC of normalized email) | list of user ids | No |
| `client_ids` | (instance, client id) | application id | Yes |
| `grants` | (instance, user id, client id) | grant id | Yes (while active) |
| `session_cookies` | (instance, cookie hash) | session id | Yes |
| `code_hashes` | (instance, code hash) | auth request id | Yes |
| `refresh_hashes` | (instance, refresh hash) | family id | Yes |
| `retired_refresh` | (instance, refresh hash) | family id | Yes |
| `access_token_ids` | (instance, jti) | family id | Yes |
| `opaque_access_hashes` | (instance, hash of `lia_` token) | (family id, jti) | Yes |
| `spa_origins` | (instance, origin) | count of active `spa` clients with a redirect URI at that origin | No |
| `pat_hashes` | (instance, PAT hash) | (user id, pat id) | Yes |
| `machine_keys` | (instance, user id, key id) | user id | Yes |
| `passkey_credentials` | (instance, credential id) | user id | Yes |
| `external_ids` `[P5]` | (instance, idp id, upstream subject) | user id | Yes |
| `idp_states` `[P5]` | (instance, hash of the upstream `state`) | IdP intent id | Yes |
| `device_codes` / `user_codes` | (instance, hash or normalized code) | device authorization id | Yes |
| `signing_keys` | (instance, kid) | key id | Yes |
| `role_assignments` `[P4]` | (instance, org, principal id, project id) | assignment id | Yes |
| `request_uris` `[P2]` | (instance, hash of the PAR `request_uri`) | auth request id | Yes |

**ES-13.** An index key derived from a secret uses the secret's SHA-256 (DM-4), never the secret.

## 5. Projections

Read models built asynchronously from `read_log`, each with a checkpoint.

| Projection | Purpose | Lag tolerance |
|---|---|---|
| `jwks_document` | The JWKS JSON per instance | Seconds; also invalidated directly on key events |
| `discovery_document` | Discovery JSON per instance | Seconds |
| `users_list`, `orgs_list`, `projects_list`, `applications_list`, `grants_list` | Management API lists and search | Seconds |
| `user_sessions` | A user's active sessions | Seconds |
| `audit_view` | Recent audit events per org for the admin UI | Seconds |

**ES-14.** No security decision reads an asynchronous projection. Decisions read the aggregate
stream or an inline index. A test lists every call site of projection reads and fails if one is
reachable from a token, introspection, userinfo, login or consent handler.

**ES-15.** Any projection can be dropped and rebuilt from position 0 and must reach a state equal
to the incrementally built one. The simulation checks this after every run.

## 6. Storage layouts

### 6.1 redb (embedded; Phase 1 first)

One redb database file per process. Tables:

| Table | Key | Value |
|---|---|---|
| `streams` | (instance id, stream type, stream id, version: u64 big-endian) | event bytes |
| `stream_heads` | (instance id, stream type, stream id) | current version |
| `log` | (instance id, position: u64 big-endian) | (stream type, stream id, version) |
| `log_head` | instance id | last position |
| `index` | (index name, instance id, key bytes) | value bytes |
| `snapshots` | (instance id, stream type, stream id) | (version, bytes) |
| `projection_*` | per projection | rows |
| `checkpoints` | projection name | position |
| `ephemeral` | (scope, key) | (value, expires_at) |

redb gives serializable write transactions with a single writer, so ES-1 to ES-5 follow from one
write transaction per `append`. Positions are assigned inside that transaction.

### 6.2 PostgreSQL (Phase 1 second)

```sql
CREATE TABLE events (
  instance_id uuid   NOT NULL,
  position    bigint NOT NULL,
  stream_type text   NOT NULL,
  stream_id   uuid   NOT NULL,
  version     bigint NOT NULL,
  event_id    uuid   NOT NULL UNIQUE,
  type        text   NOT NULL,
  schema      int    NOT NULL,
  occurred_at bigint NOT NULL,
  body        jsonb  NOT NULL,
  PRIMARY KEY (instance_id, position),
  UNIQUE (instance_id, stream_type, stream_id, version)
);
CREATE TABLE stream_heads (instance_id uuid, stream_type text, stream_id uuid, version bigint NOT NULL,
  PRIMARY KEY (instance_id, stream_type, stream_id));
CREATE TABLE log_heads (instance_id uuid PRIMARY KEY, position bigint NOT NULL);
CREATE TABLE idx (name text, instance_id uuid, key bytea, value bytea NOT NULL,
  PRIMARY KEY (name, instance_id, key));
```

- `append` runs in one transaction at `READ COMMITTED`: update `stream_heads` with a
  compare-and-set on `version` (zero rows updated means `Conflict`), insert index rows (a primary
  key violation on a `Claim` means `Taken`), then `UPDATE log_heads SET position = position + n
  ... RETURNING position` to assign positions, then insert events.
- Locking `log_heads` per instance serializes appends within an instance. That gives ES-5 (commit
  order equals position order) at the cost of write parallelism per instance. This is the
  conservative choice for Phase 1; a later ADR may relax it with a commit-time sequencer if
  benchmarks show the need.
- `READ COMMITTED` is enough because the compare-and-set and the unique keys carry the
  conflicts; `SERIALIZABLE` is not required.

### 6.3 TiKV (later)

Same logical layout as redb, with keys in a per-install keyspace and optimistic transactions.
Not specified further until it is scheduled.

## 7. Retention

| Data | Retention |
|---|---|
| Domain events of long-lived aggregates | Forever (crypto-shredding handles erasure, DM-6) |
| Ephemeral streams (`auth_request`, `device_authorization`) | 7 days after their terminal state, then deleted with their index entries |
| Token families | 30 days after their absolute expiry or revocation, then deleted with their index entries; their creation and revocation events are kept in the audit export |
| Observations | 90 days in the store; longer in an audit sink |
| Released unique names of removed orgs and users | Held for 30 days before reuse, so an old login name cannot be taken over at once |

**ES-16.** Deleting an ephemeral stream never changes a security decision: by then the stream is
in a terminal state and every index entry that could reach it is removed in the same
transaction.

## Sources

- Our own design. Optimistic concurrency per stream, inline unique indexes, an outbox and
  rebuildable projections are general event-sourcing practice.
- Crate documentation: [redb] (single-writer serializable transactions), [sqlx].
- PostgreSQL documentation on transaction isolation and sequences, for the reason sequences
  alone do not give commit-ordered positions.

[redb]: https://docs.rs/redb
[sqlx]: https://docs.rs/sqlx
