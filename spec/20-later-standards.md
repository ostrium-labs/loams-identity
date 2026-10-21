# 20. Later standards

Standards the owner listed that are not part of Phases 1 to 3, or that are optional in them. Each
section says the recommended phase, the behaviour in enough detail to plan the data model now,
and what stays out. Nothing in this file is built before its phase is scheduled, and the
discovery document does not advertise any of it until it ships ([09](09-oidc.md) OIDC-16).

| Section | Standard | Recommended phase |
|---|---|---|
| 1 | Pushed authorization requests ([RFC 9126]) | 2 (optional per client) |
| 2 | JWT-secured authorization requests ([RFC 9101]), only through PAR | 2, with PAR |
| 3 | DPoP ([RFC 9449]) | 6 |
| 4 | Client ID Metadata Documents ([CIMD]) | 3 (recommended, for MCP clients) |
| 5 | Dynamic client registration ([RFC 7591], [RFC 7592], [OIDC Registration]) | 6 |
| 6 | Back-channel logout ([OIDC Back-Channel Logout]) | 6 |
| 7 | Step-up authentication challenges ([RFC 9470]) | Already covered by OIDC parameters |
| 8 | SCIM 2.0 ([RFC 7643], [RFC 7644]) | When a customer asks |
| 9 | SAML 2.0 | When a customer asks, behind a feature flag |
| 10 | A Zitadel-compatible management API surface | 6 |

## 1. Pushed authorization requests (PAR) `[P2, optional]`

PAR moves the authorization request parameters from the browser's URL into a direct,
authenticated POST, so they cannot be read or altered in the browser and long requests fit.
Recommended for Phase 2 because it is small, closes a class of parameter-tampering issues, and is
what high-security clients expect.

**LATER-1.** `POST /oauth/par`, form-encoded, with the same parameters as the authorization
endpoint ([07](07-oauth-core.md) section 3) plus client authentication. Confidential clients
must authenticate (OAUTH-26); public clients send `client_id`. Every parameter is validated
exactly as at the authorization endpoint, at push time, and an invalid request gets a JSON error
response (`400`, [RFC 9126] section 2.3), not a redirect.

**LATER-2.** On success: `201` with `{"request_uri": "urn:ietf:params:oauth:request_uri:<value>",
"expires_in": 60}`. The value is 32 bytes from `Rng`, base64url. The server appends
`auth_request.created` with `pushed: true` and stores SHA-256 of the full `request_uri` in a
`request_uris` inline index ([05](05-event-store-and-projections.md) section 4).

**LATER-3.** At the authorization endpoint, a request with `request_uri` must also carry
`client_id`, equal to the client that pushed it. *Conservative choice:* any other parameter in
that request fails with `invalid_request`, rather than being ignored, so no parameter can be
injected in the browser. A `request_uri` is usable once: the first use binds the auth request to
the browser (OAUTH-7) and later uses fail with `invalid_request_uri`. An expired or unknown value
fails the same way.

**LATER-4.** Per-client setting `require_pushed_authorization_requests` (default false) and the
instance setting of the same name ([RFC 9126] sections 5 and 6). When set, an authorization
request without `request_uri` fails with `invalid_request`. Metadata gains
`pushed_authorization_request_endpoint` and `require_pushed_authorization_requests`, and
`request_uri_parameter_supported` becomes `true`.

## 2. JWT-secured authorization requests (JAR) `[P2, with PAR]`

**LATER-5.** A signed request object is accepted only as the `request` parameter of a PAR
request, never at the authorization endpoint directly. It is validated as a JWT per KEY-17 with
its own use profile: `typ` `oauth-authz-req+jwt` or absent, an asymmetric `alg` from the client's
registered keys (never `none`, never `HS*`), `iss` equal to the client id, `aud` equal to the
issuer as its only value, `exp` at most 5 minutes after `iat`, and a `jti` in the replay cache.
Its claims replace the form parameters; form parameters other than `client_id` and client
authentication alongside a `request` fail with `invalid_request` ([RFC 9101] section 6.3).

**LATER-6.** Encrypted request objects are not supported. `request_parameter_supported` stays
`false` because the parameter is not accepted at the authorization endpoint itself.

## 3. DPoP `[P6]`

Sender-constrained tokens for agents and clients that run outside a sandbox (Loams design 19
section 5.3).

**LATER-7.** A client sends `DPoP: <proof JWT>` on token requests. The proof is validated per
[RFC 9449] section 4.3 with its own KEY-17 use profile: `typ` `dpop+jwt`; `alg` in `EdDSA`,
`ES256`, `ES384`, `RS256`, `PS256`; the `jwk` header holds the public key (the only use where a
key is taken from the token, and the key must contain no private members); `htm` equals the
request method; `htu` equals the endpoint URL without query and fragment; `iat` within 60
seconds of `Clock` time; `jti` recorded in the replay cache for 5 minutes per (instance, key
thumbprint, `jti`).

**LATER-8.** *Conservative choice:* the token endpoint always requires a server nonce. The first
request without one gets `400` with `error: "use_dpop_nonce"` and a `DPoP-Nonce` header; nonces
are derived from `Clock` time windows and an instance key, valid for 5 minutes, so no per-client
state is stored ([RFC 9449] section 8).

**LATER-9.** A token issued with a valid proof is bound to the key: the JWT access token carries
`cnf: { "jkt": <SHA-256 JWK thumbprint, RFC 7638> }`, `token_type` is `DPoP`, and introspection
returns `cnf`. Refresh tokens issued to public clients with a proof are bound to the same key and
are refused without a matching proof ([RFC 9449] section 5). Userinfo and the management API
require `Authorization: DPoP` plus a proof with `ath` for bound tokens.

**LATER-10.** Per-client `dpop_bound_access_tokens` (default false; [RFC 9449] section 5.2)
makes proofs mandatory for that client. Metadata gains `dpop_signing_alg_values_supported`.
Token families record `jkt` so rotation keeps the binding.

## 4. Client ID Metadata Documents `[P3, recommended]`

MCP clients are expected to identify themselves with a URL whose document describes them,
instead of registering in advance ([MCP authorization], which says authorization servers SHOULD
support this and marks dynamic registration as deprecated). Recommended for Phase 3 because agent
and MCP clients are central to Loams.

**LATER-11.** Off by default. The instance setting `client_id_metadata_documents` is `off`,
`allowlist` (with host patterns) or `open`, and names the org and project in which such clients
are recorded. Metadata gains `client_id_metadata_document_supported: true` when on.

**LATER-12.** A `client_id` that is an `https` URL with a path and no fragment is treated as a
metadata document URL. The server fetches it through `HttpClient` under the outbound URL rules
([15](15-loams-integration.md) section 5.3, which already refuse special-use addresses), reads at
most 5 KiB, accepts only `200`, follows no redirects, and caches valid documents per their HTTP
cache headers clamped between 5 minutes and 24 hours. Errors and invalid documents are never
cached ([CIMD]).

**LATER-13.** The document is valid only if: its `client_id` equals the URL by simple string
comparison; `redirect_uris` is present and each entry passes the redirect rules for `native` or
`web` clients ([03](03-data-model.md) DM-11); `token_endpoint_auth_method` is `none` or
`private_key_jwt` (with `jwks` or `jwks_uri` holding public keys only), never a shared-secret
method; it holds no `client_secret`; grant types are at most `authorization_code` and
`refresh_token`.

**LATER-14.** Such a client is recorded as an application (kind `native` for `none`, `web` for
`private_key_jwt`) the first time it is used, refreshed from the document on later uses, never
`first_party`, and always shown with its URL's host on the consent screen. If the fetch fails,
the authorization request is aborted with an error page.

## 5. Dynamic client registration `[P6]`

Needed for the OpenID conformance suite's Dynamic OP profile ([19](19-verification.md)).

**LATER-15.** `POST /oauth/register` per [RFC 7591] and [OIDC Registration]. Off by default;
the instance setting `dynamic_registration` is `off`, `initial_access_token` (a bearer token
created by an admin, single-use or with a use count) or `open` (refused when the instance is
marked `production`). Registered clients go into a configured org and project.

**LATER-16.** Validation is the same as management-API creation (API-21). Defaults when a field
is omitted: `token_endpoint_auth_method` `client_secret_basic` ([RFC 7591] section 2);
`id_token_signed_response_alg` `RS256` ([OIDC Registration] section 2, KEY-1);
`grant_types` `["authorization_code"]`; `response_types` `["code"]`. Requested values we do not
support (implicit, `client_secret_jwt`, encryption) fail with `invalid_client_metadata`. Software
statements are not supported.

**LATER-17.** The response includes a registration access token and
`registration_client_uri` for [RFC 7592] read, update and delete. The registration access token
is `lirg_` plus 32 random bytes, base64url, stored as SHA-256 (an addition to ADR 0013's prefix
list). Registration is rate-limited per source address.

## 6. Back-channel logout `[P6]`

The session side (who participated, the logout token's claims) is fixed in
[10](10-sessions-and-logout.md) section 8 (SES-31, SES-32). This section fixes delivery.

**LATER-18.** Clients register `backchannel_logout_uri` (an `https` URL passing the outbound URL
rules) and `backchannel_logout_session_required`. Metadata gains
`backchannel_logout_supported: true` and `backchannel_logout_session_supported: true`.

**LATER-19.** Delivery is `POST` with `Content-Type: application/x-www-form-urlencoded` and the
body `logout_token=<JWT>`, through the outbox ([OIDC Back-Channel Logout] section 2.5). The
logout token has header `typ: logout+jwt` and the claims in SES-32. A `200` or `204` response is
success. Anything else, or no answer within 5 seconds, is retried with backoff (5s, 30s, 2m,
10m, then hourly) for up to 24 hours. Each attempt mints a fresh logout token (new `jti`, `iat`
and `exp`), because a logout token is valid for only 2 minutes.

**LATER-20.** Delivery never blocks the logout itself and never retries forever. Persistent
failures for a client are reported as a metric and in the audit stream.

## 7. Step-up authentication challenges ([RFC 9470])

A resource server that needs a stronger or fresher authentication answers
`401` with `error="insufficient_user_authentication"` and `acr_values` or `max_age`. The client
then sends a new authorization request with those parameters. This OP already supports
`acr_values` and `max_age` ([09](09-oidc.md) OIDC-8 and OIDC-10) and puts `acr` and `auth_time`
into access tokens ([08](08-tokens.md)), so nothing more is needed on this side. Loams resource
servers may use it for sensitive operations.

## 8. SCIM 2.0 `[later, on customer request]`

**LATER-21.** When scheduled: an inbound SCIM service provider ([RFC 7644]) at
`/scim/v2/orgs/{org_id}/Users` (and `/Groups` only if the owner decides that groups live in this
service, [14](14-orgs-projects-roles.md) section 10), authenticated with a machine user's token
whose audience is `<issuer>/scim` and which holds `user.manage` on the org. Users map to the
`User` aggregate with the core schema ([RFC 7643]): `userName` to login name, `name`, `emails`,
`active` to deactivate and reactivate, `externalId` stored as an external id. `DELETE` maps to
remove. ETags map to aggregate versions (API-9). Filtering supports only `userName eq` and
`externalId eq`.

## 9. SAML 2.0 `[later, feature flag]`

Both directions are possible: this service as a SAML identity provider for applications, and
SAML upstream providers in federation ([16](16-federation.md) FED-2 excludes them for now).

**LATER-22.** Not built until a customer asks, and then only behind a Cargo feature `saml` that is
off in default builds and in the release images unless the owner decides otherwise.

**LATER-23.** Library evaluation (ADR 0011). `samael` 0.0.22 (MIT, last release 2026-07) is
pre-1.0, depends on `openssl` and `openssl-sys` unconditionally, and needs the C libraries
libxml2 and xmlsec1 through its optional `xmlsec` feature, which is the feature that verifies XML
signatures. That is three C dependencies in the security-critical path, and XML signature
handling is where SAML implementations historically fail (signature wrapping, comment and
canonicalization tricks). Options, to be decided when scheduled:

| Option | For | Against |
|---|---|---|
| `samael` with `xmlsec` | Exists; full SAML coverage | Pre-1.0; OpenSSL, libxml2 and xmlsec1 C dependencies; needs a security review of its signature verification |
| Our own minimal SP and IdP on `quick-xml` and RustCrypto | Pure Rust; we control the parsing rules | XML canonicalization and signature verification are hard to get right; large test burden |
| No SAML in this service: put a SAML-to-OIDC broker in front | No XML code here at all | One more component for customers who need SAML |

**LATER-24.** Whatever is chosen: only signed assertions are accepted, signatures are verified on
the exact element that is consumed (never on an ancestor or a sibling), XML external entities and
DTDs are refused, and the response's `Destination`, `InResponseTo`, `Audience`, `NotBefore` and
`NotOnOrAfter` are checked against `Clock` with a 60-second skew.

## 10. A Zitadel-compatible API surface `[P6]`

See [17](17-management-api.md) section 9. Specified only when scheduled, from Zitadel's published
Apache-2.0 protobuf definitions and its public documentation, under ADR 0001.

## 11. Out of scope

Not planned in any phase unless the owner asks: implicit and hybrid flows, the resource owner
password grant, `client_secret_jwt`, encrypted JWTs (JWE) of any kind, signed or encrypted
userinfo and introspection responses, pairwise subject identifiers, the OIDC Session Management
iframe (ADR 0025), front-channel logout, CIBA, FAPI 2.0 profiles, OpenID Federation, mutual-TLS
client authentication ([RFC 8705]) and LDAP.

## Sources

- [RFC 9126]: OAuth 2.0 Pushed Authorization Requests, sections 2, 4, 5 and 6.
- [RFC 9101]: JWT-Secured Authorization Request, section 6.3.
- [RFC 9449]: DPoP, sections 4.3, 5, 5.2, 6 and 8.
- [RFC 7638]: JWK Thumbprint.
- [CIMD]: OAuth Client ID Metadata Document (Internet-Draft, checked at revision 02, July 2026).
- [MCP authorization]: Model Context Protocol, Authorization, revision 2026-07-28.
- [RFC 7591] and [RFC 7592]: Dynamic Client Registration and its management protocol.
- [OIDC Registration]: OpenID Connect Dynamic Client Registration 1.0, section 2.
- [OIDC Back-Channel Logout]: OpenID Connect Back-Channel Logout 1.0, sections 2.4 and 2.5.
- [RFC 9470]: OAuth 2.0 Step Up Authentication Challenge Protocol.
- [RFC 7643] and [RFC 7644]: SCIM 2.0 core schema and protocol.
- [RFC 8705]: OAuth 2.0 Mutual-TLS Client Authentication (out of scope).
- Crate documentation: [samael] (version, licence, dependencies and features, checked on
  crates.io 2026-10-21).

[RFC 9126]: https://www.rfc-editor.org/rfc/rfc9126
[RFC 9101]: https://www.rfc-editor.org/rfc/rfc9101
[RFC 9449]: https://www.rfc-editor.org/rfc/rfc9449
[RFC 7638]: https://www.rfc-editor.org/rfc/rfc7638
[CIMD]: https://datatracker.ietf.org/doc/draft-ietf-oauth-client-id-metadata-document/
[MCP authorization]: https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization
[RFC 7591]: https://www.rfc-editor.org/rfc/rfc7591
[RFC 7592]: https://www.rfc-editor.org/rfc/rfc7592
[OIDC Registration]: https://openid.net/specs/openid-connect-registration-1_0.html
[OIDC Back-Channel Logout]: https://openid.net/specs/openid-connect-backchannel-1_0.html
[RFC 9470]: https://www.rfc-editor.org/rfc/rfc9470
[RFC 7643]: https://www.rfc-editor.org/rfc/rfc7643
[RFC 7644]: https://www.rfc-editor.org/rfc/rfc7644
[RFC 8705]: https://www.rfc-editor.org/rfc/rfc8705
[samael]: https://docs.rs/samael
