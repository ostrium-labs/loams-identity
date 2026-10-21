# ADR 0022: Licence allowlist, exceptions, and the C-dependency register

- Status: Proposed (two licence exceptions need the owner's approval)
- Date: 2026-10-21

## Context

The owner's rule: dependencies must be Apache-2.0, MIT, BSD, ISC or MPL-2.0; no GPL, AGPL or LGPL;
enforce with `cargo-deny`; flag crates with C dependencies.

Some crates that are hard to avoid use other permissive licences (checked on crates.io
2026-10-21):

| Licence | Crates | How they arrive |
|---|---|---|
| Unicode-3.0 | `icu_normalizer`, `icu_properties` and related ICU4X crates | `url` → `idna` (every HTTP stack and URL parser) |
| CDLA-Permissive-2.0 | `webpki-roots`, `webpki-root-certs` | rustls root certificates for outbound HTTPS |
| Zlib | `foldhash` | A hasher used by some collection crates |

## Decision

- `deny.toml` allows: `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `MIT`, `MIT-0`,
  `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `MPL-2.0`.
- **Proposed exceptions, scoped to named crates only:** `Unicode-3.0` (ICU4X crates),
  `CDLA-Permissive-2.0` (`webpki-roots`, `webpki-root-certs`), `Zlib` (`foldhash`). These are
  permissive data or code licences without copyleft. They need the owner's approval because they
  are outside the stated list.
- Denied always: GPL, AGPL, LGPL, SSPL, BUSL, any licence with field-of-use limits, and
  unlicensed crates.
- `cargo-deny` also checks advisories (RustSec), bans duplicate major versions with an exception
  list, and restricts sources to crates.io (no git dependencies without an ADR).
- `cargo-audit` runs in CI; `cargo-vet` audits are recorded for crypto, JOSE, WebAuthn and
  protocol-parsing crates.
- **C-dependency register** (`docs/c-dependencies.md`, created in Phase 1 with the first
  `Cargo.lock`): every crate that compiles C or assembly, why we accept it, and how to replace it.
  Expected entries: `aws-lc-sys` (RSA signing, rustls provider), `openssl-sys` (through
  `webauthn-rs`, Phase 2, pending the owner's decision in ADR 0007), `ring` if any dependency
  pulls it. SAML's libxml2 and xmlsec1 are not accepted (ADR 0011).

## Alternatives considered

- **No exceptions.** Rules out `url` and therefore every HTTP client and server crate; not
  workable.
- **Allow every OSI-approved permissive licence globally.** Too loose; scoped exceptions keep new
  licences visible.

## Consequences

Each new licence or C dependency needs a line in this ADR or the register, reviewed in the pull
request that adds it.

## Owner question

Approve the three scoped exceptions (Unicode-3.0, CDLA-Permissive-2.0, Zlib)?

## Derived from

The owner's mission prompt rule 5; cargo-deny documentation; crates.io licence metadata checked
2026-10-21.
