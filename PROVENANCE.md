# Provenance

This file records where every specification and decision came from, so the clean-room process
(ADR 0001) can be audited. It is updated with every change to `spec/` or `docs/adr/`.

## Process record

| Date | Role | What happened |
|---|---|---|
| 2026-10-21 | Spec-writer (Phase A) | Wrote `spec/00` to `spec/20`, the ADRs, the threat model and the milestone plan. Three sub-sessions of the same spec-writer wrote groups of spec files in parallel from a shared set of fixed decisions; all followed the same rules. |
| 2026-10-21 | Owner | Reviewed Phase A and approved it with the controller's recommendations; all ADRs Accepted. The eleven decisions are in `docs/milestones.md`. They came from the owner, not from any outside source. |
| 2026-10-25 | Implementer (Phase B) | Started in a fresh agent session working from `spec/`, `docs/adr/`, the standards and crate documentation only. M1.1 (workspace skeleton, determinism lint, `deny.toml`, CI) used spec 01, spec 19, ADR 0016, ADR 0022 and ADR 0023, the clippy, cargo-deny and cargo-audit documentation, and crates.io licence metadata for the ADR 0022 exception crates. No Zitadel material and no other identity-provider implementation was read. |

## Rules that applied

- No Zitadel source code, forks, ports, binaries, code-search results, or texts quoting its
  internals were opened by the spec-writer. No GitHub repository of Zitadel was browsed.
- Zitadel's public product documentation at `zitadel.com/docs` was read for observable behaviour
  only. Every use is cited in the Sources section of the spec file that relies on it, and our
  names (paths, claims, scopes, roles, events, packages) were chosen to differ.
- Zitadel's Apache-2.0 protobuf definitions were not used (they are reserved for Phase 6).

## Loams-side material read

The spec-writer read these parts of the Loams monorepo (`ostrium-labs/loams`, branch `dev`) as
Loams-side contracts:

| Document | Sections read | Used for |
|---|---|---|
| `docs/design/76-identity-and-authorization.md` | 6 (OpenFGA model), 7 (tenancy in OpenFGA), 8 (sync bridge and resource tuples), 9 (the `Authorizer` trait, middleware, consistency), 10 (agents), 11 (audit), 12.1 and 12.3 (single-org mode, what Ostrium hosts), 15 (phases) | The `Authorizer` shape, the OpenFGA object types, the event-to-tuple rules, agent rules, audit record fields |
| `docs/design/19-console-identity-and-agents.md` | 4 to 6 (tenancy, principals and agents, human sign-in in OSS) | Principal kinds, agent owner and TTL cap, token claims of Loams' own tokens, setup-token rules, session lifetimes |
| `crates/loams-cache/Cargo.toml` | Package description | ADR 0021 |

**Note on design 76.** Sections 8.1 and 9.2 of design 76 contain a few facts about Zitadel's
behaviour (its Actions v2 executions, webhook signature header and a claim name) that design 76
says were checked against Zitadel's public docs. The spec-writer read those sentences while
reading the Loams-side contract. Nothing in `spec/` depends on them: the identity-sync transport
(CloudEvents with Standard Webhooks signatures, [15](spec/15-loams-integration.md)) and all claim
names (ADR 0017) were designed independently and differ.

**Not read.** In line with the controller's instruction of 2026-10-21, the spec-writer did not
open `docs/plans/ia0-spike-report.md`, anything under `conformance/identity/` or
`spikes/ia0-authz/`, the IA0 sections of plan IA1, or any worktree of the IA0 spike
(`loams-wt/ia0*`). The controller reported that the author of those files had read Zitadel
source, so they are treated as forbidden material. Nothing in this repository was derived from
them.

## Zitadel documentation pages consulted (observable behaviour only)

| Page | Facts taken |
|---|---|
| [Instances](https://zitadel.com/docs/concepts/structure/instance) | An instance is the top level, holds default settings, is normally one issuer on one domain |
| [Organizations](https://zitadel.com/docs/concepts/structure/organizations) | Orgs hold user pools, can override instance settings, have domains with optional verification, one can be the default |
| [Projects](https://zitadel.com/docs/concepts/structure/projects) | Projects hold roles (key, display name, group) and applications; settings can assert roles or require a role or grant for login |
| [Granted projects](https://zitadel.com/docs/concepts/structure/granted_projects) | A project can be granted to another org with a subset of roles |
| [Users](https://zitadel.com/docs/concepts/structure/users) | Human and machine users, one org for life, metadata |
| [OIDC endpoints](https://zitadel.com/docs/apis/openidoauth/endpoints) | Which standard endpoints and parameters exist; exact redirect matching; introspection needs client authentication; revoking a refresh token also invalidates its access tokens |
| [Scopes](https://zitadel.com/docs/apis/openidoauth/scopes), [Claims](https://zitadel.com/docs/apis/openidoauth/claims), [Grant types](https://zitadel.com/docs/apis/openidoauth/grant-types) | Standard scopes and claims are supported; the product adds org and role scopes (we use our own names); supported and unsupported grants |
| [Token exchange](https://zitadel.com/docs/guides/integrate/token-exchange) | Token exchange with an actor token adds an `act` claim; audience and scope can only narrow; an extra audit event records impersonation |
| [Default settings](https://zitadel.com/docs/guides/manage/console/default-settings) | Kinds of login, password, lockout, domain and token-lifetime settings exist |
| [Actions v2](https://zitadel.com/docs/concepts/features/actions_v2) | The product can call external endpoints on events (we designed our own outbox and sink) |
| [Service accounts](https://zitadel.com/docs/guides/integrate/service-accounts/authenticate-service-accounts) | Machine users authenticate with private-key JWT, client credentials or PATs |
| [Identity providers](https://zitadel.com/docs/guides/integrate/identity-providers/introduction), [Identity brokering](https://zitadel.com/docs/concepts/features/identity-brokering) | Upstream OIDC, OAuth and SAML providers, JIT creation and account linking exist |

## Sources per specification file

| File | Sources |
|---|---|
| `spec/00-index.md` | RFC 2119, RFC 8174, RFC 2606; owner's prompt |
| `spec/01-architecture.md` | Owner's prompt (method rules, crate list); Loams design 76 §9.1 (`Authorizer` shape); RFC 9562; crate docs (`secrecy`, `zeroize`, `thiserror`, clippy lints) |
| `spec/02-tenancy.md` | OIDC Discovery §3; RFC 8414; RFC 9207; Zitadel docs (Instances, Organizations); Loams designs 19 §4 and 76 §7 |
| `spec/03-data-model.md` | RFC 6749, 6238, 7519, 7636, 8252, 8693, 8707, 9700, 9562; Loams designs 19 §5 and 76 §10; Zitadel docs (Users, Projects, Service accounts) |
| `spec/04-event-catalog.md` | First principles; RFC 7386; Zitadel docs (Token exchange: that such events exist) |
| `spec/05-event-store-and-projections.md` | First principles (event sourcing); redb and sqlx docs; PostgreSQL docs |
| `spec/06-keys-and-jose.md` | RFC 7515, 7517, 7518, 7519, 7523, 7638, 7797, 8037, 8725, 9068, 9449, 9864, 6979; draft-ietf-oauth-rfc7523bis; OIDC Registration; OIDC Back-Channel Logout; RUSTSEC-2023-0071; docs.rs `jsonwebtoken` 11.1.0; Zitadel docs (OIDC endpoints: keys carry a `kid` and rotate) |
| `spec/07-oauth-core.md` | RFC 6749, 7523, 7636, 8252, 8414, 8707, 9207, 9700; draft-ietf-oauth-v2-1-15; OAuth 2.0 Form Post Response Mode; OIDC Core; Zitadel docs (OIDC endpoints, grant types: exact redirect matching, supported response modes) |
| `spec/08-tokens.md` | RFC 6749, 6750, 7009, 7662, 8693, 8707, 9068, 9700, 9701; OIDC Core §11; Loams design 19 §5.3; Zitadel docs (OIDC endpoints: introspection needs client authentication, refresh revocation also invalidates access tokens) |
| `spec/09-oidc.md` | OIDC Core (errata set 2), OIDC Discovery, OIDC Prompt Create, OIDC Front-Channel and Back-Channel Logout; RFC 6750, 8176, 8414, 9207; NIST SP 800-63B-4; OpenID Foundation certification pages; Zitadel docs (scopes, claims, endpoints: standard scopes, prompt values, RFC 8176 `amr`) |
| `spec/10-sessions-and-logout.md` | RFC 6265bis, RFC 8176, RFC 9700; OIDC Core; OIDC RP-Initiated Logout, Back-Channel Logout, Session Management and Front-Channel Logout; NIST SP 800-63B-4; OWASP Session Management Cheat Sheet; Loams design 19 §6; Zitadel docs (OIDC endpoints: end-session parameters) |
| `spec/11-login-and-passwords.md` | RFC 6749, 8265, 9106, 9700; OIDC Core; NIST SP 800-63B-4; OWASP Password Storage, Authentication and CSRF cheat sheets; Have I Been Pwned range API documentation; `argon2`, `askama`, `governor` docs; Loams design 19 §6; Zitadel docs (Default settings: kinds of login options exist) |
| `spec/12-passkeys-and-mfa.md` | WebAuthn Level 3; RFC 4226, 6238, 8176; OIDC Core; OpenID Connect Unmet Authentication Requirements; NIST SP 800-63B-4; the otpauth Key URI format; `webauthn-rs` and `totp-rs` docs; Zitadel docs (Default settings: passkeys, TOTP, OTP and force-MFA options exist) |
| `spec/13-machine-identity.md` | RFC 6749, 7521, 7523, 7638, 8628, 8693, 8707, 9068; draft-ietf-oauth-rfc7523bis (revision 11); OIDC Core; Loams designs 19 §5 and 76 §10; Zitadel docs (service accounts, grant types, token exchange: observable behaviour only) |
| `spec/14-orgs-projects-roles.md` | RFC 7519 §4.3, RFC 8693 §4.1; Loams design 76 §6; Zitadel docs (Projects, Granted projects: observable behaviour only) |
| `spec/15-loams-integration.md` | RFC 7519, 8414, 8693, 8707, 9068, 9207, 9728; MCP authorization specification (2026-07-28); CloudEvents 1.0; Standard Webhooks; OpenFGA docs; Loams design 76 §§6, 8.1 (event-to-tuple mapping and rules only), 9.1, 10, 11; Loams design 19 §5 |
| `spec/16-federation.md` | OIDC Core, OIDC Discovery; RFC 6749, 7636, 8176, 9207, 9700; Google, Microsoft Entra, GitHub and GitLab provider documentation; `openidconnect` and `oauth2` docs; Loams design 19 §6; Zitadel docs (Identity providers, Identity brokering: observable behaviour only) |
| `spec/17-management-api.md` | RFC 6585, 6750, 7386, 9110, 9457; the HTTP Idempotency-Key draft; `utoipa`, `tonic`, `axum` docs |
| `spec/18-security-baseline.md` | RFC 6749, 6750, 8252, 9207, 9700, 8725, 9068; OWASP cheat sheets (session, CSRF, SSRF); MDN; owner's prompt |
| `spec/19-verification.md` | OpenID Foundation conformance suite docs; a public report that the Basic OP plan sends no PKCE (versola PR 517, used only for that fact); TLA+ tools; proptest, insta, criterion, cargo-fuzz docs; owner's prompt |
| `spec/20-later-standards.md` | RFC 9126, 9101, 9449, 7638, 7591, 7592, 9470, 7643, 7644, 8705; OAuth Client ID Metadata Document draft (revision 02); MCP authorization specification; OIDC Registration; OIDC Back-Channel Logout; crates.io data for `samael` |
| `docs/threat-model.md` | RFC 6819, 9700, 8725; OIDC Core §16; WebAuthn L3 §13; NIST SP 800-63B; STRIDE; RUSTSEC-2023-0071 |
| `docs/milestones.md` | The spec files above; owner's prompt (phases) |

## Sources per ADR

| ADR | Derived from |
|---|---|
| 0001 | Owner's prompt (clean-room rules) |
| 0002 | General event-sourcing practice; owner's method |
| 0003 | redb and sqlx docs; crates.io metadata (2026-10-21); Loams light-compose direction |
| 0004 | First principles; serde docs |
| 0005 | docs.rs `jsonwebtoken` 11.1.0; crates.io metadata for `jsonwebtoken`, `josekit`, `jwt-simple`, `biscuit`; RFC 7515, 7517, 7519, 8725, 9864 |
| 0006 | OIDC Discovery §3; OIDC Registration §2; RFC 8037, 6979, 9864, 7518; RUSTSEC-2023-0071; crates.io metadata; Loams design 19 §5.3 |
| 0007 | crates.io metadata and dependency lists (`webauthn-rs`, `webauthn-rp`, `passkey`); WebAuthn Level 3; Loams design 19 §6 |
| 0008 | RFC 6238, RFC 4226; crates.io metadata for `totp-rs`; Loams design 19 §6 |
| 0009 | RFC 9106; OWASP Password Storage Cheat Sheet; `argon2` docs |
| 0010 | crates.io metadata and dependency lists (`oauth2`, `openidconnect`, `oxide-auth`) |
| 0011 | crates.io metadata and dependency list (`samael`); SAML 2.0 Core; Loams design 19 §6 |
| 0012 | RFC 9068, 7662, 6749; Loams design 19 §5.3 |
| 0013 | RFC 9562; `uuid` docs; general secret-scanning practice |
| 0014 | RFC 9700 §4.14.2; RFC 6749 §10.4; OAuth 2.1 draft 15 |
| 0015 | OIDC Discovery §3; RFC 8414; Zitadel docs (Instances), reached independently |
| 0016 | First principles; clippy docs |
| 0017 | RFC 7519 §4.3, 9068, 8693, 8176; OIDC Core §2; Loams designs 19 §5.3 and 76 §9.1 |
| 0018 | General crypto-shredding practice; NIST SP 800-38D; `aes-gcm` docs |
| 0019 | OWASP CSRF and Session cheat sheets; MDN; askama docs |
| 0020 | RFC 9457; axum, utoipa, tonic, prost docs |
| 0021 | foyer docs; Loams `crates/loams-cache/Cargo.toml` |
| 0022 | Owner's prompt rule 5; cargo-deny docs; crates.io licence metadata |
| 0023 | General DST practice; tokio, madsim, turmoil docs |
| 0024 | NIST SP 800-63B; OWASP Authentication Cheat Sheet; `governor` docs; Zitadel docs (Default settings: that separate password and OTP limits exist) |
| 0025 | OIDC Session Management, Front-Channel, Back-Channel and RP-Initiated Logout specifications; public browser documentation on third-party cookies |
| 0026 | Project Fluent docs; crates.io metadata |
| 0027 | NIST SP 800-63B; the bundled list's own source and licence, to be recorded when added |

## Uncertainty log

| Date | Item | Resolution |
|---|---|---|
| 2026-10-21 | Design 76 §8.1 and §9.2 mention Zitadel-specific facts (see the note above). | Not used. The sync transport and claim names in `spec/` are our own and differ. |
| 2026-10-21 | The idea that an org has a primary domain used for login-name suffixes appears both in Zitadel's public docs and in general multi-tenant practice. | Kept as observable behaviour, cited in `spec/02`. The verification, routing and default-org rules are our own. |
