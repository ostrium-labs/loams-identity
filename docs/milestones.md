# Milestone plan

Status: Phase A (specifications) was reviewed and approved by the owner on 2026-10-21. Phase B
has **not** started: the owner gave the ClickHouse-compatible engine priority. When it starts, it
runs in a fresh agent session working from `spec/` and the standards alone (ADR 0001). No
milestone below has started.

Each milestone is small enough for one pull request or a short series. After each one:
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, tests, `cargo deny check`, then a
commit whose message names the spec sections it implements (for example
`oidc: authorization endpoint (07 §3, OAUTH-1..OAUTH-12)`). The phase exit checklist is in
[19](../spec/19-verification.md) section 9.

The implementer works from `spec/` and the cited standards only (ADR 0001).

## Phase 1: Core OP

| # | Milestone | Specs | Done when |
|---|---|---|---|
| M1.1 | Workspace skeleton: the crates of 01 §2 (empty), `clippy.toml` with the determinism lint, `deny.toml`, CI (fmt, clippy, test, deny, audit, DCO), `docs/c-dependencies.md` | 01, ADR 0016, ADR 0022 | CI green on an empty workspace; the lint rejects a planted `SystemTime::now` |
| M1.2 | Core traits and their simulated implementations: `Clock`, `Rng`, `KeyStore`, `Cache`, `HttpClient`, `Mailer`, `EventSink`, `Authorizer`, `PasswordHasher`; contract-test suites | 01 §3 to §4 | Contract tests pass for the fakes |
| M1.3 | Event envelope, event types for Phase 1, upcaster framework, the in-memory `EventStore` and `MetaStore` | 04, 05 §1 to §2 | Envelope snapshot tests; ES-1 to ES-7 contract tests |
| M1.4 | `store-redb`: streams, log, inline indexes, ephemeral KV, checkpoints | 05 §4, §6.1 | Same contract suite passes on redb |
| M1.5 | Instance, Org and User aggregates (human, password only), tenant scoping types, crypto-shredding data keys | 02, 03 §2.1 to §2.3, ADR 0018 | Property tests for user states; first cross-tenant tests (VER-16 skeleton) |
| M1.6 | `sim` crate v1: seeded driver, fault-injecting store, clock skew; same-seed check in CI | 19 §4, ADR 0023 | Two runs of a seed are byte-identical |
| M1.7 | Signing keys: lifecycle, software `KeyStore` (Ed25519, P-256, RSA through aws-lc-rs), JWS signer, JWT verifier with the header rules, JWKS endpoint | 06, ADR 0005, ADR 0006 | Key-rotation tests; `jwt_parse` fuzz target; alg-confusion and `kid` tests |
| M1.8 | Discovery and authorization-server metadata | 09 discovery section | Snapshot of both documents |
| M1.9 | Projects (default project per org, and the instance-wide `loams` project for Loams deployments, LOAMS-19a) and Applications; CLI to create clients; `cli setup` with the setup token | 03 §2.4 to §2.5, 11 setup section, 17 CLI | A client can be registered from the CLI on a fresh install |
| M1.10 | Login UI v1: identify, password (argon2id profiles), browser session, CSRF, cookies, security headers, lockout and rate limits, anti-enumeration | 10 (sessions), 11, ADR 0009, ADR 0019, ADR 0024 | Timing and enumeration tests; lockout tests |
| M1.11 | Authorization endpoint: validation order, PKCE (with the opt-in `nonce` exception, OAUTH-14a), redirect matching, consent and grants, code issuance, `iss`, `form_post` | 07 §1 to §3 | Redirect-confusion and PKCE-downgrade suites; `redirect_uri_match` and `authorize_params` fuzz targets |
| M1.12 | Token endpoint: `authorization_code` and `refresh_token` grants, client authentication (secret basic and post), ID tokens, JWT and opaque access tokens, rotation with reuse detection | 07 §4 to §8, 08, 09 | Code-replay and refresh-reuse suites; property tests DM-I1 to DM-I4 |
| M1.13 | Userinfo, introspection, revocation; fan-out workers; read checks | 08, 09 userinfo, 05 §3 | Revocation tests for every cause; ES-11 tests |
| M1.14 | Outbox and the identity-sync export (CloudEvents over a webhook sink and the pull feed) for the Phase 1 events marked **sync** | 15, 04 §5 | Simulation shows the consumer model converges under drops, duplicates and reordering |
| M1.15 | `store-postgres` with the same contract suite; differential test redb vs PostgreSQL | 05 §6.2, ADR 0003 | Both stores pass; differential test green |
| M1.16 | TLA+ model of codes, families, grants and sessions; TLC in CI | 19 §3 | All invariants hold in the bounded model |
| M1.17 | Management API v1 (REST, then gRPC) for instances, orgs, users, projects, applications, keys, sessions, settings; `Authorizer` deny-by-default | 17, ADR 0020 | Route-permission CI test; cross-tenant suite covers every route |
| M1.18 | `docker-compose.yml` with PostgreSQL and the conformance suite; Basic, Config and Form Post OP plans; `conformance/RESULTS.md` | 19 §2 | Results recorded; the conformance clients use the opt-in PKCE exception (OAUTH-14a, VER-6) and `RESULTS.md` says so |
| M1.19 | Benchmarks baseline and the Phase 1 security review against 18 | 18, 19 §8 | `bench/RESULTS.md`; every SEC id of Phase 1 has a test |

## Phase 2: Strong authentication

| # | Milestone | Specs | Done when |
|---|---|---|---|
| M2.1 | TOTP enrolment and verification, recovery codes | 12, ADR 0008 | Replay of a used step refused; property tests |
| M2.2 | Email verification and email OTP through `Mailer`; password reset flow | 11, 12 | Enumeration tests for reset |
| M2.3 | Passkeys: registration, authentication, conditional mediation, counters and backup flags (`webauthn-rs` behind `PasskeyVerifier`, OpenSSL accepted in ADR 0007) | 12, ADR 0007 | Ceremonies pass with a software authenticator |
| M2.4 | MFA policy, `acr` and `amr`, step-up for `acr_values` and `max_age`, per-client `required_acr` | 09, 12 | Step-up tests; acr mapping snapshot |
| M2.5 | Session management: session list, end one or all, re-authentication for sensitive actions | 10 | Session-bound families die with the session (TLA+ invariant already covers it) |
| M2.6 | RP-Initiated Logout with confirmation page | 10 | Logout open-redirect suite; RP-Initiated Logout conformance plan |
| M2.7 | Optional: PAR, if the owner pulls it into Phase 2 | 20 | Per-client `require_pushed_authorization_requests` |
| M2.8 | Phase 2 re-run of conformance, benchmarks, security review | 18, 19 | Results updated |

## Phase 3: Machine identity

| # | Milestone | Specs | Done when |
|---|---|---|---|
| M3.1 | Machine users (service accounts, agents with owner and TTL cap), machine keys, client secrets | 03, 13 | Agent without an active owner gets no token (DM-8) |
| M3.2 | `private_key_jwt` client authentication and the JWT bearer grant, with the `jti` replay cache | 07, 13, 06 | Assertion replay and audience tests |
| M3.3 | Client credentials grant | 13 | Tokens carry `principal` and no user claims |
| M3.4 | Personal access tokens (feature flag, never for agents) | 13 | Expiry required; refused for agents |
| M3.5 | Device authorization grant with the `/device` page | 13, 11 | Polling, `slow_down`, user-code brute-force limits |
| M3.6 | Resource indicators and resource-server applications | 08 TOK-6, 07 | Audience narrowing tests |
| M3.7 | Token exchange: delegation with `act`, `may_act`, narrow-only rules; agent sessions | 13, 15 | Property tests: exchanged tokens never exceed either party; agent-session end revokes |
| M3.8 | Workload-identity federation (trust policies) and token vending for sandboxes | 13 | Trust-policy matching tests; SSRF tests for issuer JWKS |
| M3.8a | Client ID Metadata Documents for MCP clients (owner decision 9) | 20 §4 | Metadata fetch passes the SSRF rules (SEC-34) |
| M3.9 | identity-sync events for agents and service accounts; Loams `Authorizer` adapter tests | 15 | Agent deactivation reaches the sync consumer model within one simulated delivery cycle |
| M3.10 | Phase 3 TLA+ extension (exchanged families, agent sessions), conformance re-run, benchmarks, security review | 18, 19 | Results updated |

## Later phases (not planned in detail)

- **Phase 4:** projects and roles, role assignments, project grants, role claims, org membership
  sync to OpenFGA, webhook targets ([14](../spec/14-orgs-projects-roles.md)).
- **Phase 5:** upstream OIDC federation, account linking, JIT provisioning
  ([16](../spec/16-federation.md)).
- **Phase 6:** SCIM, SAML (only on customer demand), Back-Channel Logout, DPoP, dynamic
  registration, a compatible API surface ([20](../spec/20-later-standards.md)).

## Owner decisions (2026-10-21)

The owner approved Phase A with the controller's recommendations. Each decision is recorded in
the files named.

| # | Question | Decision | Recorded in |
|---|---|---|---|
| 1 | PKCE and the conformance Basic OP plan | An opt-in exception only for confidential OIDC clients that always send `nonce` ([RFC 9700] §2.1.1). PKCE is mandatory otherwise. | 07 OAUTH-14a and OAUTH-33, 03 (`pkce_exception`), 18 SEC-3 and SEC-40, 19 VER-6 |
| 2 | OpenSSL through `webauthn-rs` | Accepted, behind the `PasskeyVerifier` trait. Revisit a pure-Rust stack later. | ADR 0007, ADR 0022 |
| 3 | Licence exceptions | Unicode-3.0, CDLA-Permissive-2.0 and Zlib approved, each limited to the named crates. | ADR 0022 |
| 4 | Zitadel at `auth.loams.dev` | Zitadel stays until loams-identity passes the OpenID conformance plans and a security review after Phase 3; the switch is decided then. | 15 LOAMS-26 |
| 5 | Token issuer for Loams agents | loams-identity stays upstream. Loams' own token endpoint keeps environment narrowing and vending. | 15 LOAMS-6a |
| 6 | Teams and groups | Owned by the Loams control plane. No group or team aggregate here. | 14 ORG-28a |
| 7 | Phase 1 scope | Confirmed: the identity-sync outbox (M1.14) and crypto-shredding (M1.5) are in Phase 1. | This plan |
| 8 | New crate | `loams-identity-runtime` added. | ADR 0016, 01 |
| 9 | Client ID Metadata Documents | Pulled into Phase 3 (M3.8a). | 20 §4, this plan |
| 10 | Identity projects and Loams projects | One identity project, `loams`, per instance, for Loams' applications and coarse role claims. Loams projects stay in the control plane and OpenFGA; identity projects and role assignments are never mapped to tuples. | 15 LOAMS-19a and the tuple table, 14 §3, 03 |
| 11 | SAML | Deferred until a customer asks. | ADR 0011 |

All ADRs are Accepted. Any later change to these decisions needs a new ADR or an amendment that
names the date and the owner's approval.

[RFC 9700]: https://www.rfc-editor.org/rfc/rfc9700
