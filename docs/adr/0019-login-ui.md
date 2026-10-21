# ADR 0019: Server-rendered login UI with askama; CSRF and cookie design

- Status: Proposed
- Date: 2026-10-21

## Context

The owner asks for a minimal server-rendered login UI. It handles passwords, passkeys, MFA,
consent, logout confirmation, registration, password reset, the device-flow page and first-run
setup. The Loams console has its own front end, which may later drive login through the
management API, but the provider must work on its own.

## Decision

- Templates: **askama** 0.16 (MIT OR Apache-2.0), compiled into the binary, auto-escaping on.
- No JavaScript is required except the small WebAuthn script for passkey ceremonies, served as a
  static file with a hash-pinned CSP.
- Plain HTML forms posting to `/login/...` routes, with redirects between steps (POST, redirect,
  GET).
- **Cookies:** a session cookie `__Host-` prefixed, `Secure`, `HttpOnly`, `SameSite=Lax`; a login
  state cookie (binding the browser to the authorization request) with `SameSite=Lax` and a short
  lifetime; a CSRF cookie with `SameSite=Strict`.
- **CSRF:** synchronizer token in every form, bound to the session or the login state, compared in
  constant time; plus a check that `Origin` (or `Sec-Fetch-Site`) is same-origin on every POST.
- **Branding:** per-org logo and three colours, from settings; no custom HTML or CSS injection.
- **Localization:** message catalogues per locale, chosen from `ui_locales` then
  `Accept-Language`, English fallback.

## Alternatives considered

- **maud** (MIT OR Apache-2.0, last release 2025-02): fine, but templates in Rust macros are
  harder for designers to edit.
- **minijinja** (Apache-2.0): runtime templates allow operator customization, which is also an
  injection surface. Not in Phase 1.
- **A single-page app.** More moving parts and a larger attack surface for the most sensitive
  pages.

## Consequences

The UI is plain. A custom login experience is possible later through an API, which is a separate
decision.

## Derived from

[OWASP CSRF Prevention Cheat Sheet], [OWASP Session Management Cheat Sheet], MDN documentation on
cookie prefixes, askama documentation, crates.io metadata checked 2026-10-21.

[OWASP CSRF Prevention Cheat Sheet]: https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html
[OWASP Session Management Cheat Sheet]: https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html
