# ADR 0011: SAML deferred; `samael` C dependencies flagged

- Status: Proposed
- Date: 2026-10-21

## Context

The owner lists SAML 2.0 as later, behind a feature flag, and only when a customer asks. SAML
needs XML parsing, XML canonicalization and XML signature verification, which are hard to get
right and historically a source of signature-wrapping vulnerabilities.

## Findings (crates.io, 2026-10-21)

`samael` 0.0.22 (MIT, released 2026-07-07):

- Still pre-1.0 (`0.0.x`).
- **Hard dependencies on `openssl` and `openssl-sys`** (C).
- The `xmlsec` feature adds **`libxml` (bindings to libxml2) and links xmlsec1** (C libraries),
  which signature verification needs.

## Decision

- No SAML in Phases 1 to 5.
- When a customer asks, write an ADR comparing: `samael` with its C stack behind a feature flag,
  a pure-Rust XML-DSig path (none mature as of this date), and brokering SAML through an external
  IdP into OIDC (which Loams already documents as its approach).
- The feature, if built, is off by default and excluded from the default container image.

## Alternatives considered

- **Build SAML now.** Not in scope; the C stack and the attack surface are not justified without
  a customer.

## Consequences

Enterprise customers who need SAML in Phases 1 to 5 put a SAML-to-OIDC broker in front.

## Derived from

crates.io metadata and dependency list for `samael` 0.0.22, checked 2026-10-21; [SAML 2.0 Core]
(OASIS); Loams design 19 section 6 (SAML through a broker, Loams-side).

[SAML 2.0 Core]: https://docs.oasis-open.org/security/saml/v2.0/saml-core-2.0-os.pdf
