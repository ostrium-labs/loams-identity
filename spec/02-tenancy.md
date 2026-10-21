# 02. Tenancy

Two levels of tenant: the **instance** and, inside it, the **organization**. Everything else
belongs to exactly one organization, or to the instance itself.

## 1. Instance

An instance is one issuer. It has its own signing keys, its own default settings, its own set
of organizations, and one or more host names.

**TEN-1.** Each instance has exactly one issuer identifier: an `https` URL with no query and no
fragment ([OIDC Discovery] section 3, [RFC 8414] section 2). The default form is
`https://<primary host>`. A path form `https://<host>/i/<instance slug>` is allowed for
deployments that cannot use one host per instance (for example local development on
`localhost`).

**TEN-2.** Requests are routed to an instance by the `Host` header (host form) or by the path
prefix (path form), looked up in the `instance_hosts` index. A request whose host and path match
no instance gets a plain `404` with no instance-specific content. Routing happens before any
other processing, and the resolved instance id is attached to the request context.

**TEN-3.** Every value that names the issuer in a response (discovery `issuer`, token `iss`,
the `iss` authorization-response parameter of [RFC 9207]) comes from the resolved instance, never
from a request header other than the one used for routing.

**TEN-4.** An instance has a state: `active`, `suspended` or `removed`. A suspended instance
answers every protocol endpoint with `503` and a `Retry-After`, except discovery and JWKS, so
resource servers can still validate tokens until they expire. A removed instance answers `404`.

**TEN-5.** Creating an instance is a privileged operation of the process operator (the `cli`
`setup` command, or the system API in [17](17-management-api.md)). It is not reachable from any
tenant's credentials.

Instance settings hold the defaults for every organization: login policy, password policy,
lockout policy, token lifetimes, branding, enabled features. An organization may override a
setting; an unset org setting inherits the instance value at read time.

## 2. Organization

An organization is the unit Loams calls an org. It owns users (human and machine), projects,
applications, org domains and org-level settings.

**TEN-6.** Every user, project, application, grant, token family and org setting belongs to
exactly one org, recorded as `org_id`, and never moves to another org.

**TEN-7.** An org has a state: `active`, `deactivated` or `removed`. Users of a deactivated org
cannot sign in, and token, refresh, introspection and userinfo requests for its users or clients
fail as if the token were inactive. Removing an org is a soft delete: its event streams stay for
audit, its users and clients are treated as removed, and its unique names (domains, login names)
are released after the retention period in [05](05-event-store-and-projections.md).

**TEN-8.** One org per instance is the **default org**. A login that does not name an org (no
org hint, and a login name that matches no org domain) starts in the default org. Self-service
registration, if enabled, creates users in the default org unless the request names another
org that allows registration.

**TEN-9.** An org has zero or more **org domains** (DNS names). One may be marked primary. A
domain can be claimed by at most one org per instance. If the instance setting
`require_verified_org_domains` is on, a domain becomes usable for login-name suffixes and org
discovery only after verification by a DNS TXT record or an HTTP file at a well-known path
holding a random token. Verification is re-checked on a schedule (default every 24h); a failed
re-check marks the domain unverified but does not change existing login names.

## 3. The single-org default (OSS and light installs)

**TEN-10.** A fresh install creates one instance and one org during first-run setup (see
[11](11-login-and-passwords.md) section 8). That org is the default org. All code paths are the
same as with many orgs: there is no "single tenant" branch in the code. A test runs the full
protocol suite on an install with exactly one org and asserts that every stored record carries
both ids.

## 4. Org context in a request

The org a request acts in is resolved in this order, and the first match wins:

1. The authenticated principal's own org (for tokens, sessions and management calls).
2. An explicit org hint on an authorization request: the scope `urn:loams:org:<org_id>` or the
   parameter `org_hint=<org_id or org domain>` (our extension; ignored by other servers).
3. The org domain suffix of the login name the user typed.
4. The default org.

**TEN-11.** If an org hint names an org that does not exist or is not active, the authorization
request fails with `invalid_request` and the error description "unknown organization". The
login page MUST NOT reveal whether the org exists by any other difference in behaviour.

**TEN-12.** A user authenticated in org A who presents an org hint for org B is not switched to
org B. The request fails with `access_denied` unless the user has an external membership in B
(Phase 4, [14](14-orgs-projects-roles.md)).

## 5. Tenant scoping rules

These rules are the core of the cross-tenant leakage tests in [19](19-verification.md).

**TEN-13.** Every repository and projection query takes a `TenantScope` value as its first
argument: either `Instance(instance_id)` or `Org(instance_id, org_id)`. There is no query API
that takes neither. A query in `Org` scope MUST filter on both ids.

**TEN-14.** Lookups by an externally supplied value (client id, login name, token hash, code
hash, key id, redirect URI, session id) MUST include the instance id in the index key. A value
that exists in another instance behaves exactly like a value that does not exist.

**TEN-15.** A token, code or session issued by instance X MUST be rejected by instance Y even if
the store is shared, the signature would verify (keys are per instance, so it will not) and the
token's identifiers happen to exist in Y. The check is on the `iss` claim and on the instance id
stored with the token's record.

**TEN-16.** A client registered in org A may authenticate users of org A only, unless its
project is granted to another org (Phase 4). An authorization request where the authenticated
user's org is not allowed for the client fails with `access_denied` after login, with the same
timing and page as a user who cancels.

**TEN-17.** Management API calls carry the caller's principal; every call checks that the
target object's `org_id` (and `instance_id`) matches a scope the caller is authorized for,
before reading the object's content. A wrong-org id answers `404`, not `403`, so ids from other
orgs cannot be probed.

**TEN-18.** Exported events (identity-sync, audit, webhooks) carry `instance_id` and `org_id`, and
a sink configured for one org MUST receive only that org's events.

## 6. Where tenancy is visible in tokens

- `iss` identifies the instance.
- The private claim `org` carries the user's org id in ID tokens, access tokens, introspection and
  userinfo (see [15](15-loams-integration.md) for the full Loams claim contract).
- Client ids are unique per instance, so `client_id` and `azp` are unambiguous within an issuer.

## Sources

- [OIDC Discovery]: OpenID Connect Discovery 1.0, section 3 (issuer form).
- [RFC 8414]: OAuth 2.0 Authorization Server Metadata, section 2.
- [RFC 9207]: OAuth 2.0 Authorization Server Issuer Identification.
- Zitadel public docs, observable behaviour only: an instance is the top of the hierarchy, holds
  default settings and normally runs on one domain as one issuer; organizations hold their own
  user pools and can override instance defaults; an org can hold several domains, one primary,
  with optional DNS or HTTP verification; one org can be the default for logins that name none
  ([Instances], [Organizations]). The design above (scope types, routing, error behaviour, the
  default-org rules, the hint scope name) is ours.
- Loams design 19 section 4 and design 76 section 7: OSS runs one org on the same code path, and
  every object is rooted in an org (Loams-side contracts).

[OIDC Discovery]: https://openid.net/specs/openid-connect-discovery-1_0.html
[RFC 8414]: https://www.rfc-editor.org/rfc/rfc8414
[RFC 9207]: https://www.rfc-editor.org/rfc/rfc9207
[Instances]: https://zitadel.com/docs/concepts/structure/instance
[Organizations]: https://zitadel.com/docs/concepts/structure/organizations
