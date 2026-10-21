# 01. Architecture

This file fixes the crate layout, the trait boundaries and the rules that keep the core
deterministic. It is our own design. The sources are the owner's method rules and general
engineering practice, not another identity provider.

## 1. Shape of the system

loams-identity is one Rust binary (`loams-identity`) that serves:

- the OAuth 2.0 / OpenID Connect provider endpoints (public);
- the login UI (public, server-rendered HTML);
- the management API, REST and gRPC (private by default; see [17](17-management-api.md));
- a health and metrics listener (private).

State lives in an event store. The event log is the source of truth. Everything a request reads
is either the replayed state of one aggregate or a projection built from the log
([05](05-event-store-and-projections.md)).

```text
             public                                    private
  browser ── login-ui ─┐                      ┌── api-rest ── admin, Loams control plane
  RP/client ── oidc ───┼── core (commands, ───┼── api-grpc
                       │   state machines)    └── cli
                       │        │
                       │   EventStore / MetaStore / KeyStore / Cache / Clock / Rng ...
                       │        │
                       └── store-redb | store-postgres ── outbox ── EventSink (identity-sync, audit, webhooks)
```

## 2. Crates

All crates are prefixed `loams-identity-`. One Cargo workspace.

| Crate | Purpose | May do I/O? |
|---|---|---|
| `core` | Domain types, aggregates, commands, protocol state machines, the trait definitions in section 3. | No |
| `events` | Event envelope, event payload types, schema versions and upcasters ([04](04-event-catalog.md)). | No |
| `store-redb` | `EventStore` and `MetaStore` on redb (embedded, single node, development). | Yes |
| `store-postgres` | `EventStore` and `MetaStore` on PostgreSQL through sqlx. | Yes |
| `oidc` | HTTP handlers for the OAuth 2.0 and OIDC endpoints. Parses requests, calls `core`, renders responses. | Through traits only |
| `authn` | Credential verification: passwords (argon2id), passkeys (WebAuthn), TOTP, email OTP, recovery codes. | Through traits only |
| `federation` | Upstream identity provider client (OIDC relying party), account linking, JIT provisioning. | Through traits only |
| `api-rest` | Management API over HTTP/JSON with an OpenAPI document. | Through traits only |
| `api-grpc` | Management API over gRPC. | Through traits only |
| `login-ui` | Server-rendered login, consent, MFA and logout pages. | Through traits only |
| `runtime` | Real implementations of the I/O traits: system clock, OS random source, SMTP mailer, SMS sender, software key store, foyer cache, webhook and log event sinks, HTTP client. | Yes |
| `cli` | The `loams-identity` binary: `serve`, `setup`, `migrate`, `keys`, `admin` subcommands, and configuration loading. | Yes |
| `sim` | Deterministic simulation harness: fake implementations of every I/O trait with fault injection, a seeded workload driver, invariant checkers. | No real I/O |
| `conformance` | Glue for the OpenID conformance suite: test-plan configuration, client registrations, result parsing. | Yes (test only) |

The `runtime` crate is an addition to the owner's list, approved by the owner on 2026-10-21. It keeps every real I/O implementation in
one place, so the core crates can be checked mechanically for I/O (ADR 0016).

**ARCH-1.** `core` and `events` MUST NOT depend on `tokio` networking or file APIs, `sqlx`,
`redb`, `reqwest`, `hyper`, `lettre`, `std::net`, `std::fs` or `std::process`.

**ARCH-2.** `oidc`, `authn`, `federation`, `api-rest`, `api-grpc` and `login-ui` MUST reach the
outside world only through the traits in section 3. They may depend on `axum` and `tonic` for
request and response types, but they do not open sockets themselves. The `cli` crate binds
listeners and wires real implementations.

**ARCH-3.** Dependency direction is one-way: `core` ← `events` ← (`authn`, `oidc`, `federation`,
`api-*`, `login-ui`) ← `cli`. Store crates depend on `core` and `events`. `sim` depends on
everything except `runtime`, `store-postgres` and `cli`.

## 3. Trait boundaries

Every external dependency sits behind a trait with at least two implementations: a real one and
one for tests or embedding.

| Trait | Responsibility | Real implementation | Test or embedded implementation |
|---|---|---|---|
| `Clock` | Current time (wall clock, UTC, nanosecond resolution) and monotonic time. | System clock (`runtime`) | `SimClock`: manual, with skew and jumps (`sim`) |
| `Rng` | Cryptographically secure random bytes. | OS source through `getrandom` (`runtime`) | `SeededRng`: ChaCha20 from a seed (`sim`) |
| `EventStore` | Append events to aggregate streams with optimistic concurrency, maintain inline indexes, read streams, read the global log. | `store-postgres`, `store-redb`; TiKV later | In-memory store with fault injection (`sim`) |
| `MetaStore` | Projection tables with checkpoints, and an ephemeral key-value space with expiry (rate-limit counters, replay caches). | `store-postgres`, `store-redb` | In-memory (`sim`) |
| `Cache` | Read-through cache for derived, non-authoritative data (JWKS documents, client metadata, upstream IdP metadata). | foyer hybrid cache (`runtime`) | In-memory map with an injected clock (`sim`) |
| `KeyStore` | Create signing and encryption keys, sign, decrypt data keys, list public keys. Private key material never leaves it. | Software keys sealed with a root key (`runtime`); cloud KMS later | Deterministic software keys from the seed (`sim`) |
| `Mailer` | Send one templated email. | SMTP through lettre (`runtime`) | Recording mailer (`sim`, tests) |
| `SmsSender` | Send one SMS. `[P2]` | HTTP provider adapter (`runtime`) | Recording sender (`sim`) |
| `EventSink` | Deliver exported events (audit, identity-sync, webhooks) at least once. | Webhook sender, log sink, Iggy later (`runtime`) | Recording sink with drops, duplicates and reordering (`sim`) |
| `Authorizer` | Decide whether a principal may perform a management action. | OpenFGA client (`runtime`), built-in evaluator over identity's own memberships | Allow-all (development, loopback only), recording authorizer |
| `HttpClient` | Outbound HTTP for federation, JWKS fetches of client keys, webhooks. | reqwest with rustls (`runtime`) | Scripted responses with latency and failures (`sim`) |
| `PasswordHasher` | Hash and verify passwords. | argon2id (`authn`) | Fast deterministic fake for simulation only (`sim`) |
| `PasskeyVerifier` | WebAuthn registration and assertion ceremonies. `[P2]` | webauthn-rs adapter (`authn`) | Scripted authenticator (`sim`) |

**ARCH-4.** No trait method may hide a second, untracked side effect. For example `Mailer::send`
sends one message and nothing else.

**ARCH-5.** The `AllowAll` authorizer MUST refuse to start unless every listener is bound to a
loopback address. The process logs a warning on every start in this mode.

**ARCH-6.** Every trait has a contract test suite in `core` (behind a `contract-tests` feature)
that each implementation runs. A new implementation is not merged until it passes.

The `Authorizer` here uses the same shape as Loams' authorizer (design 76 section 9.1): `check`,
`batch_check` and `list_objects` over a subject, a relation and an object. See
[15](15-loams-integration.md). This service does not decide resource-level permissions for Loams.
It only uses the authorizer to protect its own management API.

## 4. Determinism

The whole system must be able to run under deterministic simulation: the same seed gives the
same sequence of events, tokens and responses.

**ARCH-7.** Code in `core`, `events`, `oidc`, `authn`, `federation`, `api-*` and `login-ui` MUST
NOT call `SystemTime::now`, `Instant::now`, `chrono::Utc::now`, `time::OffsetDateTime::now_utc`,
`rand::thread_rng`, `rand::rng`, `OsRng`, `getrandom`, `uuid::Uuid::new_v4`, `Uuid::now_v7`,
`std::thread::spawn` or `tokio::spawn`. Time comes from `Clock`, randomness from `Rng`, and
background work is registered with a scheduler that the runtime or the simulator drives.

**ARCH-8.** The rule in ARCH-7 is enforced in CI by `clippy::disallowed_methods` and
`clippy::disallowed_types` in a shared `clippy.toml`, and by a test that greps the listed crates
for the names above. A third-party crate that reads the clock or the OS random source internally
may be used only behind a trait whose simulation implementation does not call it (see ADR 0005
for `jsonwebtoken`, ADR 0007 for `webauthn-rs`, ADR 0008 for `totp-rs`). The disallowed list also
names the `new_random` constructors of `openidconnect` and `oauth2` (CSRF tokens, nonces, PKCE
verifiers), which read the OS random source; the `federation` crate builds those values from
`Rng` instead ([16](16-federation.md)).

**ARCH-9.** Iteration order that reaches output (JSON maps, event payloads, claim sets, JWKS key
order) MUST be deterministic. Use ordered maps (`BTreeMap`) or sort before serializing.

**ARCH-10.** Identifiers are UUIDv7 ([RFC 9562]) built from `Clock` time and `Rng` bytes
(ADR 0013). Secrets (codes, tokens, salts, challenges) are drawn from `Rng`.

**ARCH-11.** Every timeout, expiry and lifetime is compared against `Clock` time, never against a
timer firing. A timer may trigger work (for example expiring sessions), but the decision whether
something has expired is always made by comparing timestamps.

## 5. Concurrency model

- Each command targets one aggregate (sometimes two, see [05](05-event-store-and-projections.md)
  section 3). The handler loads the aggregate, decides, and appends events with an expected
  version. A version conflict means another writer won; the handler reloads and decides again, at
  most three times, then fails with a retryable error.
- Command handlers are pure functions from (state, command, time, random bytes) to (events or a
  typed error). This is what the property tests and the TLA+ model check.
- Projections and the outbox run as background tasks registered with the scheduler.

## 6. Errors

**ARCH-12.** Every crate defines typed errors with `thiserror`. Library code MUST NOT call
`unwrap`, `expect` or `panic!` outside tests; CI denies `clippy::unwrap_used`,
`clippy::expect_used` and `clippy::panic` in non-test code.

**ARCH-13.** Protocol errors are a closed enum per endpoint that maps one-to-one to the error
codes the standards define (for example `invalid_grant`). Internal errors map to `server_error`
and never leak internal detail to the client. The internal detail goes to the log with a
request id that the response also carries.

**ARCH-14.** Log lines and error messages MUST NOT contain secrets: passwords, codes, tokens,
client secrets, private keys, OTPs, recovery codes, session ids or PII marked sensitive in
[03](03-data-model.md). Secret-bearing types wrap `secrecy::SecretBox` (or an equivalent with
`zeroize` on drop) and their `Debug` output is redacted.

## 7. Configuration

- One configuration file (TOML) plus environment variable overrides with the prefix
  `LOAMS_IDENTITY_`. Secrets are read from files (`*_FILE` variables), never from the command line.
- Configuration that affects tenants (policies, lifetimes) lives in the event log as instance or
  org settings, not in the file. The file holds only process concerns: listeners, store
  connection, key store root key location, log level, external URLs.

## 8. Observability

- `tracing` spans per request with the request id, instance id and org id (ids only).
- OpenTelemetry export is optional and off by default.
- Metrics: request counts and latency per endpoint, token issuance counts, login failures,
  lockouts, projection lag, outbox lag, event-sink delivery failures.

## Sources

- The owner's mission prompt (2026-10-21): the Loams method rules (trait-first, deterministic,
  event-sourced, multi-tenant, authorization outside) and the crate list.
- Loams design 76 section 9.1 (the `Authorizer` trait shape, a Loams-side contract).
- [RFC 9562]: Universally Unique IDentifiers (UUIDv7).
- Crate documentation: `secrecy`, `zeroize`, `thiserror`, clippy's `disallowed_methods` lint.

[RFC 9562]: https://www.rfc-editor.org/rfc/rfc9562
