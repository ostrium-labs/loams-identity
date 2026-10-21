# ADR 0021: foyer directly for the cache; not Loams' `loams-cache`

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

The owner's method names "our foyer-based cache crate" as the `Cache` implementation. In the Loams
monorepo, `loams-cache` is a read-through RAM and NVMe **byte-range** cache over immutable objects,
and it is not published to crates.io. loams-identity needs a small key-value cache for derived
data: JWKS documents, client metadata, upstream IdP metadata and JWKS, unwrapped data keys.

## Decision

- The `Cache` trait's real implementation uses **foyer** 0.22 (Apache-2.0) directly, in memory
  only by default (the hybrid disk tier is optional and off).
- Cache only derived, non-authoritative data. Never cache a positive security decision for longer
  than its source allows (for example revocation state is not cached at all; client metadata is
  cached at most 30 seconds and invalidated on `application.*` events).
- Unwrapped data keys are cached at most 5 minutes and zeroized on eviction.
- The in-memory implementation for the simulator uses `Clock` for expiry.

## Alternatives considered

- **Depend on `loams-cache` from the monorepo** (git dependency). Wrong shape (byte ranges), and a
  git dependency ties this repo's releases to the monorepo's.
- **moka.** Also good; foyer keeps the Loams stack consistent.

## Consequences

If Loams later publishes a general key-value cache crate, switching is a one-crate change.

## Derived from

foyer documentation; Loams monorepo `crates/loams-cache/Cargo.toml` description (Loams-side,
read 2026-10-21); crates.io metadata.
