# ADR 0016: A `runtime` crate for real I/O, and a determinism lint

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

The owner's crate list has no home for the real implementations of `Clock`, `Rng`, `Mailer`,
`SmsSender`, `KeyStore`, `Cache`, `EventSink` and `HttpClient`. Putting them in `cli` mixes wiring
with logic; putting them in `core` breaks the no-I/O rule.

## Decision

- Add `loams-identity-runtime` for those implementations ([01](../../spec/01-architecture.md)
  section 2).
- Enforce ARCH-7 with a shared `clippy.toml` (`disallowed-methods`, `disallowed-types`) applied to
  every crate except `runtime`, `cli`, the store crates and test code, plus a CI grep as a backstop
  for macros and re-exports that clippy misses.

## Alternatives considered

- **Implementations in `cli`.** Workable but makes the binary crate large and hard to test.
- **One crate per adapter.** More crates for little gain at this size.

## Consequences

One extra crate, approved by the owner on 2026-10-21.

## Derived from

First principles; clippy documentation for `disallowed_methods` and `disallowed_types`.
