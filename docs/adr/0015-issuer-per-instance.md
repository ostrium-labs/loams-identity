# ADR 0015: One issuer per instance, host-based routing, org as a claim

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

Tenancy has two levels. The issuer could be per instance (orgs share it) or per org (each org its
own issuer, discovery document and keys).

## Decision

- One issuer per instance (TEN-1), routed by host, with a path form for development.
- Orgs share the issuer and keys; the org is carried in the `org` claim and enforced by tenant
  scoping (TEN-13 to TEN-18).
- A customer who needs hard key isolation gets their own instance.

## Alternatives considered

- **Issuer per org.** Strong isolation, but N discovery documents and key sets, and every Loams
  install would need per-org issuer configuration. The instance level already gives that option.
- **Issuer per org under one host with paths.** Same cost as per-org issuers.

## Consequences

An RP that trusts the issuer trusts every org of the instance unless it checks `org`. The Loams
integration spec makes Loams check `org` against the install's org binding
([15](../../spec/15-loams-integration.md)).

## Derived from

[OIDC Discovery] section 3; [RFC 8414]. Zitadel's public docs describe an instance as one issuer
on one domain with many orgs ([Instances]); we reached the same level independently for the
reasons above, and the routing design is ours.

[OIDC Discovery]: https://openid.net/specs/openid-connect-discovery-1_0.html
[RFC 8414]: https://www.rfc-editor.org/rfc/rfc8414
[Instances]: https://zitadel.com/docs/concepts/structure/instance
