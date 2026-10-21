# ADR 0009: argon2id parameters and bounded hashing

- Status: Proposed
- Date: 2026-10-21

## Context

Passwords are hashed with argon2id (owner's rule). Parameters trade attacker cost against
login latency and memory; memory per hash also bounds how many logins can run at once.

## Decision

- Crates: `argon2` 0.6.0 and `password-hash` 0.6.1 (RustCrypto, MIT OR Apache-2.0).
- Two named profiles, selected per instance:

| Profile | Memory | Iterations | Lanes | Basis |
|---|---|---|---|---|
| `standard` (default) | 64 MiB | 3 | 4 | [RFC 9106] section 4, second recommended option |
| `small` | 19 MiB | 2 | 1 | OWASP Password Storage Cheat Sheet minimum |

- 16-byte salt from `Rng`, 32-byte output, stored as a PHC string.
- On a successful login with a hash made under other parameters, rehash and append
  `user.password_set` (same password, new PHC).
- A semaphore sized from configured memory (default: 25 percent of the process limit divided by
  the profile's memory) bounds concurrent hashes; waiters time out after 2 seconds with `503`
  (SEC-33).
- Unknown-user logins run a dummy verification against a fixed hash of the same profile
  (SEC-31).
- No pepper in Phase 1. A pepper (HMAC with a `KeyStore` key before hashing) can be added later
  without a password reset by wrapping on next login.

## Alternatives considered

- **RFC 9106 first option (2 GiB).** Too heavy for a login server.
- **scrypt or bcrypt.** The owner chose argon2id.

## Consequences

Benchmarks report login throughput with the profile stated (VER-18). The `small` profile is
weaker and is meant for small single-user installs; the setup command says so.

## Derived from

[RFC 9106], [OWASP Password Storage Cheat Sheet], `argon2` crate documentation, crates.io
metadata checked 2026-10-21.

[RFC 9106]: https://www.rfc-editor.org/rfc/rfc9106
[OWASP Password Storage Cheat Sheet]: https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
