# loams-identity

A clean-room identity provider in Rust for [Loams](https://github.com/ostrium-labs/loams):
OpenID Connect and OAuth 2.0 provider, multi-tenant organizations, passkeys and MFA, machine
identities for services and AI agents, and an event-sourced audit trail. Apache-2.0.

**Status: Phase B has started; nothing works yet.** The owner approved the specifications and
every ADR on 2026-10-21. Milestone M1.1 is done: the Cargo workspace with its fourteen empty
crates, the determinism lint, `deny.toml` and CI ([milestones](docs/milestones.md)). There is no
working binary and no release yet.

## What it will be

- **An OpenID Provider and OAuth 2.0 authorization server**, OAuth 2.1 behaviour by default:
  authorization code with PKCE for every client (one opt-in exception for confidential OIDC
  clients that always send `nonce`), exact redirect matching, refresh-token rotation
  with reuse detection, no implicit flow, no password grant.
- **Multi-tenant from day one:** instances (one issuer each) containing organizations. A single
  org runs on the same code path.
- **Strong authentication:** argon2id passwords, passkeys (WebAuthn Level 3), TOTP, email OTP,
  step-up.
- **Machine identity:** service accounts and agents, `private_key_jwt`, client credentials,
  device flow, and token exchange for agent delegation with `act` claims.
- **Event-sourced core:** the event log is the source of truth, projections are rebuildable, and
  audit comes for free. Personal data in the log is crypto-shredded on erasure.
- **Authorization stays outside.** Fine-grained permissions live in OpenFGA behind Loams'
  `Authorizer` trait; this service emits the membership and lifecycle events that keep OpenFGA in
  sync.
- **Deterministic by construction:** time, randomness and I/O are injected, so the whole system
  runs under deterministic simulation with fault injection.

## Where it fits in Loams

Loams design 76 uses Zitadel and OpenFGA for identity and authorization, and lists a Rust-native
identity provider behind the same traits as a later option. This project builds that option. Zitadel stays at `auth.loams.dev` until loams-identity passes
the OpenID conformance plans and a security review after Phase 3. The
contract with Loams (token claims, identity-sync events, the OpenFGA mapping) is in
[spec/15-loams-integration.md](spec/15-loams-integration.md).

## Clean room

Zitadel v3 and later is AGPL-3.0. This project implements public standards and observable
behaviour from scratch and never reads Zitadel's source. Specifications are written in one role
and implemented in another, and every source is recorded. See
[ADR 0001](docs/adr/0001-clean-room-process.md), [PROVENANCE.md](PROVENANCE.md) and
[CONTRIBUTING.md](CONTRIBUTING.md).

## Read the specification

| Document | What it covers |
|---|---|
| [spec/](spec/00-index.md) | The behaviour, file by file, with requirement ids and sources |
| [spec/03-data-model.md](spec/03-data-model.md) and [spec/04-event-catalog.md](spec/04-event-catalog.md) | Aggregates, states and every event |
| [docs/threat-model.md](docs/threat-model.md) | STRIDE threat model and the top threats |
| [docs/adr/](docs/adr/README.md) | Architecture decisions, including every crate choice |
| [docs/milestones.md](docs/milestones.md) | Ordered milestones for Phases 1 to 3, and the owner's decisions |

## Planned architecture

```text
 crates/
   loams-identity-core          domain, state machines, traits (no I/O)
   loams-identity-events        event envelope, payloads, upcasters
   loams-identity-store-redb    embedded store (first)
   loams-identity-store-postgres
   loams-identity-oidc          provider endpoints
   loams-identity-authn         passwords, passkeys, TOTP, OTP
   loams-identity-federation    upstream identity providers
   loams-identity-api-rest      management API (JSON, OpenAPI)
   loams-identity-api-grpc      management API (gRPC)
   loams-identity-login-ui      server-rendered login pages
   loams-identity-runtime       real I/O implementations of the traits
   loams-identity-cli           the loams-identity binary
   loams-identity-sim           deterministic simulation harness
   loams-identity-conformance   OpenID conformance suite glue
```

Trait boundaries (`EventStore`, `MetaStore`, `Cache`, `Clock`, `Rng`, `KeyStore`, `Mailer`,
`SmsSender`, `EventSink`, `Authorizer`, `HttpClient`) are listed in
[spec/01-architecture.md](spec/01-architecture.md). How to run the conformance suite will be
documented when Phase 1 adds `docker-compose.yml`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Every commit needs a DCO sign-off, and contributors must
never read Zitadel's source code. Security reports: [SECURITY.md](SECURITY.md).

## Licence

[Apache-2.0](LICENSE). Copyright 2026 Ostrium Labs and loams-identity contributors.
