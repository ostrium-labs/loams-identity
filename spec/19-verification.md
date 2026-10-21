# 19. Verification

A phase is not done until every check in this file that applies to it passes. Results are
committed, so a reviewer can see them without rerunning anything.

## 1. Unit and property tests

**VER-1.** Every state machine in [03](03-data-model.md) section 4 has `proptest` tests that
generate random sequences of commands (including concurrent interleavings expressed as
sequences of loads and appends) and check the invariants DM-I1 to DM-I7 after every step:

| State machine | Generated commands | Invariants |
|---|---|---|
| AuthRequest and code | create, authenticate, consent, issue code, redeem (correct and wrong verifier, wrong client, wrong redirect URI), redeem again, time passes | DM-I1, DM-15, DM-16 |
| TokenFamily | create, rotate with the current token, present a retired token, revoke for every reason, time passes | DM-I2, DM-I4, DM-17 to DM-19 |
| Session | create, add factor, touch, end, time passes, user deactivated | DM-I3 for session-bound families |
| Grant | create, widen, revoke, issue under it | DM-I3, DM-12 |
| User | the transitions in 4.1, credential changes, lockout | DM-13, DM-14 |
| SigningKey | create, activate, retire, sign, verify across rotation | KEY rules in [06](06-keys-and-jose.md) |

**VER-2.** Command handlers are pure (ARCH section 5), so the property tests call them directly
with an in-memory store and a `SimClock`.

**VER-3.** Snapshot tests (`insta`) pin the JSON of discovery, JWKS, token responses, ID tokens,
access tokens, introspection responses, error responses and exported events, with a fixed seed
so they are byte-stable.

**VER-4.** Every requirement id in `spec/` that names behaviour is referenced by at least one
test. A CI job extracts the ids from `spec/*.md` and from test names or `#[doc]` attributes, and
fails if a requirement of a completed phase has no test.

## 2. OpenID Foundation conformance suite

**VER-5.** The suite runs in Docker from `docker-compose.yml` (loams-identity, PostgreSQL, the
conformance suite and its MongoDB), pinned to a tagged release of the suite. A script drives it
with the suite's test-plan runner and writes `conformance/RESULTS.md` with the suite version, the
plan names and variants, and pass, warning, failure and skip counts per test.

| Plan | Phase | Notes |
|---|---|---|
| OIDC Basic OP (`oidcc-basic-certification-test-plan`, static client, discovery) | 1 | Code flow, `client_secret_basic` and `client_secret_post` |
| OIDC Config OP (`oidcc-config-certification-test-plan`) | 1 | Discovery and JWKS |
| OIDC Form Post OP (`oidcc-formpost-basic-certification-test-plan`) | 1 | `response_mode=form_post` |
| RP-Initiated Logout OP | 2 | |
| Dynamic OP (`oidcc-dynamic-certification-test-plan`) | 6 | Needs dynamic registration ([20](20-later-standards.md)) |
| Back-Channel Logout OP | 6 | |

Implicit, hybrid, Session Management and Front-Channel Logout plans do not apply (SEC-1,
ADR 0025).

**VER-6. PKCE and the Basic OP plan.** Other implementers report that the Basic OP plan sends
authorization requests without a PKCE `code_challenge`. The owner decided (2026-10-21) to allow an
opt-in exception for confidential OIDC clients that always send `nonce` ([07](07-oauth-core.md)
OAUTH-14a). The conformance clients use it, and `RESULTS.md` says so next to SEC-3. No other
client gets it by default. Verify the suite's behaviour against the pinned version at milestone
M1.18; if the suite sends PKCE by then, register the conformance clients without the exception.

**VER-7.** Expected warnings are recorded with their reason. A warning or failure that follows
from a conservative choice in `spec/` is listed in `RESULTS.md` next to the requirement id that
causes it. Nothing is skipped silently.

## 3. TLA+ model

**VER-8.** A TLA+ specification `tla/TokenLifecycle.tla` models: authorization requests and
codes, redemption, refresh-token families with rotation, reuse detection, revocation (by client,
user, grant, session end and agent-session end), browser sessions with logout, and expiry with an
abstract clock. Concurrency is modelled as interleaved load and compare-and-append steps on
versioned streams, with the read checks of ES-10.

Invariants to model-check with TLC (state space bounded: 2 clients, 2 users, 3 codes, 3
refresh generations, 2 sessions):

| Invariant | Statement |
|---|---|
| `CodeAtMostOnce` | No code is redeemed into tokens more than once. |
| `ReuseKillsFamily` | If a retired refresh token is presented while its family is active, the family is revoked in the same step, and no later token is issued in that family. |
| `RevokedGrantYieldsNothing` | After a grant is revoked, no token is issued under it. |
| `EndedSessionYieldsNothing` | After a session ends, no token is issued in a family bound to it. |
| `SingleCurrentRefresh` | Each active family has exactly one current refresh token. |
| `TypeOK` | The usual well-formedness invariant. |

Liveness (with fairness on the fan-out worker): every family under a revoked grant eventually
records `revoked`.

**VER-9.** The model is checked in CI with TLC on every change to `tla/` or to the token,
session or grant handlers, with a time limit of 15 minutes on a GitHub-hosted runner.

## 4. Deterministic simulation

**VER-10.** The `sim` crate runs the real handlers in one process with the simulated
implementations of every trait in [01](01-architecture.md) section 3, under a single-threaded
runtime with paused time (ADR 0023). A run is defined by a seed. The same seed MUST give
byte-identical event logs, responses and trace output. CI runs the same seed twice and compares.

**VER-11.** Workload: simulated browsers, RPs, machine clients and agents perform logins, code
flows, refreshes, token exchanges, logouts, revocations and admin actions, chosen by the seeded
`Rng`.

**VER-12.** Faults the simulator injects, each with a probability set per run:

| Fault | Where |
|---|---|
| Crash after event append, before the projection or outbox step | `EventStore` |
| Crash after projection write, before checkpoint | `MetaStore` |
| Append timeout with the write committed, and with it not committed (ES-7) | `EventStore` |
| Store unavailable for a period | `EventStore`, `MetaStore` |
| Duplicated, dropped and reordered deliveries | `EventSink` |
| Clock skew between simulated nodes, forward jumps, backward jumps | `Clock` |
| Slow, failing and malformed upstream responses | `HttpClient` |
| Mail delivery failure | `Mailer` |
| Partial write of a batch | `EventStore` (must be impossible; the fake asserts ES-1 by refusing to model it, and a separate test checks the real stores) |

**VER-13.** After each run the simulator checks: DM-I1 to DM-I7; that every projection rebuilt
from position 0 equals the incremental one (ES-15); that every exported event was delivered at
least once and that the identity-sync consumer model reached the same tuple set as a direct
computation from the log; that no response contained a secret it should not.

**VER-14.** CI runs 200 seeds per pull request and 10,000 seeds nightly. A failing seed is
printed and becomes a regression test.

## 5. Security tests

**VER-15.** A dedicated suite covers each attack below with at least one positive and one
negative test.

| Attack | Expected outcome | Requirements |
|---|---|---|
| Redirect URI confusion: prefix, suffix, path traversal, case change, userinfo (`https://good@evil`), encoded characters, extra query, fragment, IDN look-alikes, loopback port games for non-native clients | Rejected, error page, no redirect | SEC-4, SEC-5 |
| PKCE downgrade: no challenge, `plain`, verifier for a code with another challenge, verifier on a code that had none | Rejected | SEC-3 |
| Code replay: second redemption, parallel redemptions | Second fails; first family revoked | SEC-7 |
| Code injection: code issued to client A redeemed by client B; code redeemed with another redirect URI | `invalid_grant` | [07](07-oauth-core.md) |
| Refresh reuse: retired token, parallel use of the current token | Family revoked; at most one rotation succeeds | SEC-8 |
| JWT `alg` confusion: `none`, HS256 with the public key as secret, RS/ES swap, `alg` not matching the JWK | Rejected | SEC-12 to SEC-14 |
| `kid` injection: path, SQL, URL, very long, unknown, `kid` from another instance | Rejected, no lookup outside the instance key set | SEC-15 |
| Token substitution across tenants: token of instance X at instance Y; access token of org A used for a management call in org B; ID token used as access token | Rejected | SEC-16, SEC-18, TEN-15 |
| Open redirect on logout: unregistered, prefix-matching, no client identified | Not redirected | SEC-9 |
| Login timing and enumeration: unknown user vs wrong password vs locked, measured over 1,000 attempts | No statistically significant difference (Welch's t-test, p > 0.01) | SEC-31 |
| CSRF on login, consent, logout, credential changes | Rejected | SEC-25 |
| SSRF via `jwks_uri`, IdP discovery, webhooks | Rejected | SEC-34 |

## 6. Cross-tenant leakage

**VER-16.** A generated test enumerates every repository method, projection query, inline index
lookup, protocol endpoint and management API route. For each, it creates two instances with two
orgs each, populates them with the same shapes of data, and calls the path with the scope or
credentials of one tenant and the ids or values of another. Every such call must behave as
"not found" (TEN-14, TEN-17). A new route without an entry fails the test.

## 7. Fuzzing

**VER-17.** `cargo-fuzz` targets, run for 10 minutes each nightly and 30 seconds each per pull
request:

| Target | Input |
|---|---|
| `jwt_parse` | Compact JWS strings into our verification pipeline (header parsing, base64url, JSON, claim checks) |
| `redirect_uri_match` | Pairs of registered and requested URIs |
| `authorize_params` | Query strings to the authorization endpoint parser |
| `token_params` | Form bodies to the token endpoint parser, including client authentication headers |
| `event_decode` | Stored event bytes through the upcasters |
| `scim_filter` `[later]` | SCIM filter expressions |

## 8. Benchmarks

**VER-18.** `criterion` benchmarks, reported in `bench/RESULTS.md` with the machine description:

| Benchmark | Reported |
|---|---|
| JWT access token issuance (refresh grant, end to end in process, redb and PostgreSQL) | p50, p99, throughput |
| Introspection of a JWT access token and of an opaque token | p50, p99 |
| JWT verification of our own access token (what Loams gateways do) | p50, p99 |
| Login (password check), with the argon2id parameters stated (ADR 0009) | p50, p99, logins per second per core |
| Event append (single stream; token-issuing batch) | p50, p99 |

No target numbers are set in Phase A; Phase 1 records the first baseline and later phases may
not regress it by more than 20 percent without an ADR.

## 9. Phase exit checklist

Each phase ends with: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, all tests,
`cargo deny check`, `cargo audit`, the simulation seeds, the fuzz smoke run, TLC (from Phase 1),
the conformance plans that apply, and an updated `conformance/RESULTS.md` and `bench/RESULTS.md`.

## Sources

- OpenID Foundation, [About the conformance suite] (Docker deployment, test plans and the
  runner script).
- Reports from other implementers that the Basic OP plan sends no PKCE challenge, for example
  [versola PR 517] (an independent project, not a provider we copy; used only for the fact about
  the test suite).
- Leslie Lamport, [TLA+ tools] documentation.
- Crate documentation: [proptest], [insta], [criterion], [cargo-fuzz].
- The owner's mission prompt (2026-10-21), section "Verification".

[About the conformance suite]: https://openid.net/certification/about-conformance-suite/
[versola PR 517]: https://github.com/versolauth/versola/pull/517
[TLA+ tools]: https://lamport.azurewebsites.net/tla/tools.html
[proptest]: https://docs.rs/proptest
[insta]: https://docs.rs/insta
[criterion]: https://docs.rs/criterion
[cargo-fuzz]: https://rust-fuzz.github.io/book/cargo-fuzz.html
