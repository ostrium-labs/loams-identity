# ADR 0025: No OIDC Session Management iframe and no front-channel logout

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

The owner's list includes "Session Management". OpenID Connect Session Management 1.0 defines a
`check_session_iframe` that an RP embeds to poll the OP's session state through a cookie read in
a third-party context. Front-Channel Logout 1.0 also relies on the OP's cookies inside iframes.
Browsers now block or partition third-party cookies by default (Safari, Firefox) or are moving to,
so both mechanisms fail silently for many users.

## Decision

- Do not implement the `check_session_iframe` or Front-Channel Logout.
- Implement what still works: RP-Initiated Logout 1.0 (Phase 2), Back-Channel Logout 1.0
  (Phase 6), the `sid` claim from Phase 1 so back-channel logout can be added without changing ID
  tokens, and `prompt=none` re-checks for RPs that want to know whether the user is still signed
  in.
- "Session management" in our scope means the OP's own browser sessions
  ([10](../../spec/10-sessions-and-logout.md)): lifetimes, listing, ending, and binding refresh
  tokens to them.

## Alternatives considered

- **Implement the iframe anyway** for RPs on the same site. Little value, ongoing maintenance, and
  a conformance profile we would pass only in permissive browsers.

## Consequences

Discovery does not advertise `check_session_iframe` or `frontchannel_logout_supported`. RPs that
need logout propagation use back-channel logout.

## Derived from

[OIDC Session Management 1.0], [OIDC Front-Channel Logout 1.0], [OIDC Back-Channel Logout 1.0],
[OIDC RP-Initiated Logout 1.0]; browser vendors' public documentation on third-party cookie
blocking (general web platform facts).

[OIDC Session Management 1.0]: https://openid.net/specs/openid-connect-session-1_0.html
[OIDC Front-Channel Logout 1.0]: https://openid.net/specs/openid-connect-frontchannel-1_0.html
[OIDC Back-Channel Logout 1.0]: https://openid.net/specs/openid-connect-backchannel-1_0.html
[OIDC RP-Initiated Logout 1.0]: https://openid.net/specs/openid-connect-rpinitiated-1_0.html
