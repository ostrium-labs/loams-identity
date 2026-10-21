# ADR 0008: TOTP with `totp-rs` 6, explicit-time calls only

- Status: Proposed
- Date: 2026-10-21

## Context

TOTP ([RFC 6238]) is a small construction over HMAC. It needs the current time, which must come
from `Clock`.

## Decision

- Use **`totp-rs` 6.0.0** (MIT, released 2026-08-06) with the `otpauth`, `qr` and `zeroize`
  features. Its dependencies (`hmac`, `sha1`, `sha2`, `constant_time_eq`) are pure Rust.
- Call only the functions that take an explicit timestamp. Every function that reads the system
  clock (the `*_current` family) is in `clippy.toml`'s disallowed methods.
- Generate the shared secret with our `Rng`, not the crate's `gen_secret` feature.
- Parameters: SHA-1, 6 digits, 30-second period, plus or minus one step, and replay protection
  of the last used step (DM-9). SHA-1 here is HMAC-SHA-1, which RFC 6238 allows and authenticator
  apps support most widely.

## Alternatives considered

- **Hand-written HOTP/TOTP over `hmac`.** About 40 lines and fully deterministic, but the crate
  also gives `otpauth` URIs and QR codes. Acceptable fallback if `totp-rs` changes direction.

## Consequences

Compatibility with common authenticator apps. The QR feature pulls in an image encoder; keep it
in the `authn` crate only.

## Derived from

[RFC 6238], [RFC 4226], crates.io metadata and dependency list checked 2026-10-21, Loams design 19
section 6 (Loams already uses `totp-rs` 6).

[RFC 6238]: https://www.rfc-editor.org/rfc/rfc6238
[RFC 4226]: https://www.rfc-editor.org/rfc/rfc4226
