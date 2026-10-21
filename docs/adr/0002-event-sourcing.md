# ADR 0002: Event sourcing with per-aggregate streams, inline indexes and asynchronous projections

- Status: Proposed
- Date: 2026-10-21

## Context

The owner's method requires an event-sourced core with rebuildable projections, and audit as a
by-product. An identity provider also needs strong consistency for a few decisions: a code is
redeemed once, a refresh token rotates once, a revoked grant yields nothing, login names are
unique.

## Decision

- One stream per aggregate instance, with a version per stream and optimistic concurrency
  ([05](../../spec/05-event-store-and-projections.md) ES-1 to ES-7).
- **Inline indexes** written in the same transaction as the events, for uniqueness and for
  lookups by secret hash. Security decisions read only streams and inline indexes (ES-14).
- **Read checks** (expected version with no events) in token-issuing batches, so issuance and
  revocation of a parent cannot both succeed (ES-10).
- **Asynchronous projections** for lists, search and exported documents, rebuildable from
  position 0.
- **Fan-out workers** for revocations that affect many families; correctness does not depend on
  them (ES-11).
- An **outbox** reads the log and delivers exported events to sinks at least once.

## Alternatives considered

- **CRUD tables with an audit table.** Simpler, but audit is then a second write path that can
  drift, and the owner asked for event sourcing.
- **Only asynchronous projections.** Makes "code redeemable once" depend on projection lag;
  rejected.
- **One global stream with a single writer.** Simplest concurrency, but no per-aggregate
  isolation and harder to shard later.

## Consequences

- Ephemeral flows (authorization requests, device codes) become streams with retention, which
  adds writes per login. Measured in Phase 1 benchmarks.
- PostgreSQL appends serialize per instance (05 section 6.2). Acceptable for Phase 1.

## Derived from

General event-sourcing and CQRS practice; the owner's method rules. Not taken from any identity
product.
