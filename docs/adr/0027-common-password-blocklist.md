# ADR 0027: A bundled common-password blocklist, licence checked before adding

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

NIST SP 800-63B-4 requires checking new passwords against a list of commonly used, expected or
compromised values. The login spec makes a local blocklist always on
([11](../../spec/11-login-and-passwords.md)), with an optional online breached-password check that
is off by default.

## Decision

- Bundle a list of the most common passwords (on the order of 100,000 entries), normalized the
  same way passwords are ([11](../../spec/11-login-and-passwords.md), RFC 8265 OpaqueString), and
  stored compressed in the binary.
- Before the list is added (milestone M1.10), its source and licence are recorded in
  `PROVENANCE.md` and this ADR. Only a list under a licence on the ADR 0022 allowlist, or in the
  public domain, may be bundled. A candidate is the common-credentials data in the SecLists
  project, to be confirmed at that time.
- Context-specific words are also refused: the instance and org names, the user's login name and
  email local part.

## Alternatives considered

- **Online breached-password check only.** Needs network access and a third party; kept optional.
- **No list.** Fails the NIST requirement.

## Consequences

The binary grows by roughly 0.5 to 1 MiB compressed.

## Derived from

[NIST SP 800-63B](https://pages.nist.gov/800-63-4/sp800-63b.html), password verifier
requirements.
