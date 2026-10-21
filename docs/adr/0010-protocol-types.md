# ADR 0010: Own provider-side protocol types; `openidconnect` and `oauth2` only for federation

- Status: Proposed
- Date: 2026-10-21

## Context

The point of this project is to own the provider's protocol state machines. Existing Rust crates
are mostly client-side. The owner asked us to evaluate `oxide-auth` for ideas only if its licence
fits.

## Candidates (checked on crates.io, 2026-10-21)

| Crate | Version, last release | Licence | Side | Notes |
|---|---|---|---|---|
| `oauth2` | 5.0.0, 2025-01-21 | MIT OR Apache-2.0 | Client | Mature, about 17 million recent downloads; HTTP client pluggable |
| `openidconnect` | 4.0.1, 2025-07-06 | MIT | Relying party | Mature; discovery, ID-token verification, metadata types; depends on `oauth2` 5, `chrono`, `rand` 0.8, RustCrypto |
| `oxide-auth` | 0.6.1, 2024-06-02 | MIT OR Apache-2.0 | Server | Last release over two years ago; uses `rust-argon2`, `chrono`, `rand` 0.8 |

## Decision

- Provider-side request parsing, validation, state machines and responses are our own code in
  `core` and `oidc`, written from the RFCs.
- `openidconnect` 4 and `oauth2` 5 are used only in the `federation` crate (Phase 5), as the
  relying-party client toward upstream identity providers, with our `HttpClient` plugged in. Their
  internal clock and randomness use is confined there; the simulator replaces the whole
  federation client behind a trait.
- `oxide-auth` is not a dependency. Its licence is compatible, so reading its documentation for
  ideas is allowed, but its maintenance level does not justify depending on it.
- We may use `openidconnect`'s metadata types for parsing **upstream** discovery documents, never
  for producing our own (our discovery document is our own struct, so its fields are under our
  control).

## Alternatives considered

- **Build on `oxide-auth`.** Rejected: stale, and it would make the core state machines someone
  else's design.
- **Use `openidconnect` types for our discovery and ID tokens.** Tempting, but ties our output to
  a client library's choices and its `chrono` time model.

## Consequences

More code to write and test, which is the intended work. Fuzzing and conformance cover it.

## Derived from

crates.io metadata and dependency lists checked 2026-10-21; crate documentation on docs.rs.
