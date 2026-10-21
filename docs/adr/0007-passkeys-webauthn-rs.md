# ADR 0007: Passkeys with `webauthn-rs` 0.5 behind a trait (MPL-2.0, OpenSSL flagged)

- Status: Proposed (needs an owner decision on the C dependency)
- Date: 2026-10-21

## Context

Phase 2 needs a WebAuthn relying party: registration and authentication ceremonies, attestation
handling, counters, backup flags, conditional mediation. The owner's prompt names `webauthn-rs`.
The rules allow MPL-2.0 and require flagging C dependencies.

## Candidates (checked on crates.io, 2026-10-21)

| Crate | Version, last release | Licence | Notes |
|---|---|---|---|
| `webauthn-rs` / `webauthn-rs-core` | 0.5.5, 2026-04-30 | MPL-2.0 | Mature, used by Kanidm, opinionated safe API, features for conditional UI and resident keys. **`webauthn-rs-core` has a hard dependency on `openssl` and `openssl-sys`** (C). About 3 million recent downloads |
| `webauthn-rp` | 0.3.0, 2025-04-03 | MIT OR Apache-2.0 | Pure Rust on RustCrypto (`p256`, `p384`, `ed25519-dalek`, `rsa`). Small user base (about 28 thousand recent downloads), one maintainer, self-hosted repository |
| `passkey` (1Password) | 0.6.0, 2026-10-01 | MIT OR Apache-2.0 | Client and authenticator side; useful for **tests** (a software authenticator), not an RP verifier |
| Hand-written RP over `coset` and RustCrypto | | | Most control; highest risk of subtle verification bugs |

## Decision

- Use **`webauthn-rs` 0.5** in the `authn` crate behind the `PasskeyVerifier` trait.
- Use it unmodified as a dependency. MPL-2.0 is file-level copyleft: we do not copy or modify its
  files, so our code stays Apache-2.0.
- Accept OpenSSL as a flagged C dependency, link the system OpenSSL 3 by default, offer the
  `vendored` feature for static builds, and record it in the C-dependency register.
- Enable `webauthn-rs`'s `danger-allow-state-serialisation` feature, so ceremony state (the
  challenge and options between start and finish) can be stored in the `MetaStore` with a short
  expiry and finished on any node. The feature is named "danger" because serialized state must not
  be handed to the client; we store it server-side only, keyed by a random id bound to the login
  state cookie, and delete it on first use.
- Use `passkey` (1Password) as a software authenticator in tests and in the simulator.
- Keep the trait narrow (start registration, finish registration, start authentication, finish
  authentication, with our own types), so `webauthn-rp` or a later pure-Rust option can replace
  it without touching callers.

## Alternatives considered

- **`webauthn-rp`**: avoids C, but the bus factor and user base are small for a security-critical
  verifier. Second choice; revisit if the owner rejects OpenSSL.
- **Hand-written**: rejected for Phase 2; the attestation and extension surface is large.

## Consequences

- `webauthn-rs` generates challenges internally from its own randomness, so its ceremonies are not
  deterministic. The simulator uses a scripted `PasskeyVerifier`; the real adapter has its own
  integration tests with the `passkey` software authenticator.
- `webauthn-rs-core` does not depend on the `rsa` crate (it uses OpenSSL), and a relying party
  only verifies signatures with public keys, so RUSTSEC-2023-0071 does not concern it.

## Owner question

Accept OpenSSL (through `webauthn-rs`) as a C dependency in the default build, or require a
pure-Rust passkey stack (`webauthn-rp`, or our own over RustCrypto) and accept the maturity risk?

## Derived from

crates.io metadata and dependency lists checked 2026-10-21; [WebAuthn Level 3] (W3C
Recommendation, 2026-08-25); Loams design 19 section 6 (Loams already planned `webauthn-rs` 0.5,
MPL-2.0, used unmodified).

[WebAuthn Level 3]: https://www.w3.org/TR/webauthn-3/
