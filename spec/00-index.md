# Specification index and conventions

This directory is the only input the implementer (Phase B) works from, together with the
standards it cites. Each file describes behaviour in our own words and lists its sources at the
end. Nothing here is copied or paraphrased from another identity provider's source code.

Status: Phase A (specification). Nothing is implemented yet.

**Note for implementers.** Some Sources sections cite pages of Zitadel's public documentation.
Those links record where the spec-writer learned an observable behaviour; they are provenance,
not reading material. Implementers do not follow them (ADR 0001). Everything needed to build is
in these files and in the standards they cite.

## Reading order

| File | Subject | First phase |
|---|---|---|
| [01-architecture.md](01-architecture.md) | Crates, trait boundaries, determinism, errors | 1 |
| [02-tenancy.md](02-tenancy.md) | Instances, organizations, issuers, tenant scoping | 1 |
| [03-data-model.md](03-data-model.md) | Aggregates, entities, states, invariants, identifiers | 1 |
| [04-event-catalog.md](04-event-catalog.md) | Every domain event, its payload and its consumers | 1 |
| [05-event-store-and-projections.md](05-event-store-and-projections.md) | Append, concurrency, indexes, projections, outbox, storage layouts | 1 |
| [06-keys-and-jose.md](06-keys-and-jose.md) | Signing keys, rotation, JWKS, JWS rules, JWT validation | 1 |
| [07-oauth-core.md](07-oauth-core.md) | Clients, authorization endpoint, code + PKCE, token endpoint, client authentication | 1 |
| [08-tokens.md](08-tokens.md) | Access tokens, refresh-token families, introspection, revocation | 1 |
| [09-oidc.md](09-oidc.md) | ID tokens, claims, scopes, discovery, userinfo, prompt and max_age | 1 |
| [10-sessions-and-logout.md](10-sessions-and-logout.md) | Browser sessions, RP-initiated logout, back-channel logout | 1 (logout in 2) |
| [11-login-and-passwords.md](11-login-and-passwords.md) | Login UI, passwords, lockout, rate limits, CSRF, cookies, setup | 1 |
| [12-passkeys-and-mfa.md](12-passkeys-and-mfa.md) | WebAuthn passkeys, TOTP, email OTP, recovery codes, step-up | 2 |
| [13-machine-identity.md](13-machine-identity.md) | Machine users, private_key_jwt, client credentials, PATs, device flow, token exchange | 3 |
| [14-orgs-projects-roles.md](14-orgs-projects-roles.md) | Projects, applications, roles, grants, role claims | 4 |
| [15-loams-integration.md](15-loams-integration.md) | Token contract with Loams, identity-sync events, OpenFGA mapping, audit | 1 to 4 |
| [16-federation.md](16-federation.md) | Upstream OIDC identity providers, linking, JIT provisioning | 5 |
| [17-management-api.md](17-management-api.md) | REST and gRPC management API, CLI | 1 |
| [18-security-baseline.md](18-security-baseline.md) | Numbered security requirements that every phase must meet | 1 |
| [19-verification.md](19-verification.md) | Conformance, TLA+, simulation, fuzzing, benchmarks | 1 |
| [20-later-standards.md](20-later-standards.md) | PAR, DPoP, dynamic registration, SCIM, SAML | 2 to 6 |

The data model and the event catalog are [03](03-data-model.md) and [04](04-event-catalog.md).
The threat model is [../docs/threat-model.md](../docs/threat-model.md), the decisions are in
[../docs/adr/](../docs/adr/README.md), and the milestone plan is
[../docs/milestones.md](../docs/milestones.md).

## Conventions

- **Requirement keywords.** MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are used as in
  [RFC 2119] and [RFC 8174], and only when written in capitals.
- **Requirement ids.** Each normative statement that a test should cover carries an id such as
  `OAUTH-12`. The prefix names the file. Tests name the ids they cover, so coverage can be
  checked mechanically. Ids are never reused; a removed requirement keeps its id with the text
  "withdrawn".
- **Phase tags.** A requirement or section marked `[P3]` is first implemented in Phase 3. Untagged
  text applies from the phase given in the table above.
- **"Conservative choice".** Where a standard leaves a choice open, the spec says which option we
  take and why, marked *Conservative choice*. The implementer must not loosen these to pass a
  test; a conflict is reported instead (see [19-verification.md](19-verification.md)).
- **Durations** are written as `15m`, `1h`, `30d`. **Sizes** use KiB and MiB.
- **Identifiers in examples** are invented. Hosts use `example.com` ([RFC 2606]).

## Prefixes

| Prefix | File |
|---|---|
| `ARCH` | 01 architecture |
| `TEN` | 02 tenancy |
| `DM` | 03 data model |
| `EV` | 04 event catalog |
| `ES` | 05 event store and projections |
| `KEY` | 06 keys and JOSE |
| `OAUTH` | 07 OAuth core |
| `TOK` | 08 tokens |
| `OIDC` | 09 OIDC |
| `SES` | 10 sessions and logout |
| `LOGIN` | 11 login and passwords |
| `MFA` | 12 passkeys and MFA |
| `MACH` | 13 machine identity |
| `ORG` | 14 orgs, projects, roles |
| `LOAMS` | 15 Loams integration |
| `FED` | 16 federation |
| `API` | 17 management API |
| `SEC` | 18 security baseline |
| `VER` | 19 verification |
| `LATER` | 20 later standards |

## Glossary

| Term | Meaning here |
|---|---|
| Instance | The top tenant. One instance is one issuer. |
| Organization (org) | A tenant inside an instance. Owns users, projects and applications. |
| User | A principal that signs in. Either human or machine. A machine user is a service account or an agent. |
| Agent | A machine user that represents an AI agent. It has an accountable owner (a human user) and short-lived tokens only. |
| Application | An OAuth client registered in a project. Also called a client. |
| Grant | A user's standing authorization of one client in one org, with the scopes the user consented to. Token families hang off a grant. |
| Token family | All refresh tokens descended from one authorization by rotation. |
| Session | A browser sign-in session at the identity provider (single sign-on). Not the same as an agent session. |
| Agent session | A bounded unit of agent work (for example one coding task) that tokens can be scoped to. |
| Projection | A read model rebuilt from the event log. |
| OP | OpenID Provider, the role this service plays. |
| RP | Relying Party, a client that consumes ID tokens. |
| Loams | The data platform this service is the identity layer for. |

## Sources

- [RFC 2119]: Key words for use in RFCs to Indicate Requirement Levels.
- [RFC 8174]: Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words.
- [RFC 2606]: Reserved Top Level DNS Names.
- The owner's mission prompt (2026-10-21), for the phase list and the method.

[RFC 2119]: https://www.rfc-editor.org/rfc/rfc2119
[RFC 8174]: https://www.rfc-editor.org/rfc/rfc8174
[RFC 2606]: https://www.rfc-editor.org/rfc/rfc2606
