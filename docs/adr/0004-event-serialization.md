# ADR 0004: JSON event envelope with versioned payloads and upcasters

- Status: Proposed
- Date: 2026-10-21

## Context

Events live forever and are exported to audit sinks and to Loams (which lands audit data in
Iceberg tables). The format must be readable by other tools and must evolve.

## Decision

- Events are JSON (`serde_json`), envelope as in [04](../../spec/04-event-catalog.md) section 1.
- Each event type has a `schema` number. A shape change adds a new number and an upcaster from
  each older number; readers always see the newest shape.
- PostgreSQL stores the body as `jsonb`; redb stores the UTF-8 bytes.
- Map fields are ordered (ARCH-9) so stored bytes are deterministic.

## Alternatives considered

- **Protobuf or postcard.** Smaller and faster, but harder to inspect, to export and to evolve
  without a registry. Size is not a concern at identity-event volumes.
- **CloudEvents as the storage envelope.** We use CloudEvents only for export
  ([15](../../spec/15-loams-integration.md)); the stored envelope needs fields CloudEvents does not
  define (stream version, position).

## Consequences

Upcasters must be tested with stored fixtures of every old schema (fuzz target `event_decode`).

## Derived from

First principles; serde documentation.
