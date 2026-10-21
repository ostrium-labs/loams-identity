# ADR 0003: redb first, PostgreSQL second, TiKV later

- Status: Proposed
- Date: 2026-10-21

## Context

Loams' light default runs embedded storage (redb) with optional heavier services behind
profiles. Hosted `auth.loams.dev` and Kubernetes installs want PostgreSQL. Loams also runs TiKV
for some roles.

## Decision

- `store-redb` (redb 4.x, MIT OR Apache-2.0) is built first. It is the development, test and
  single-node backend, and the one the light compose profile uses.
- `store-postgres` (sqlx 0.9, MIT OR Apache-2.0, PostgreSQL 14 or later) follows within Phase 1.
- Both pass the same `EventStore` and `MetaStore` contract suite (ARCH-6), and a differential
  test runs one workload against both and compares results.
- TiKV is specified only when scheduled.

## Alternatives considered

- **SQLite.** Good embedded option, but redb matches the Loams light stack and avoids a C
  library.
- **PostgreSQL only.** Heavier for the light default and for tests.
- **sled.** Not maintained at the needed level.

## Consequences

- redb has a single writer per database; fine for one node. Multi-node requires PostgreSQL.
- Layouts in [05](../../spec/05-event-store-and-projections.md) section 6.

## Derived from

Crate documentation for redb and sqlx (versions checked on crates.io 2026-10-21: redb 4.4.0,
sqlx 0.9.0). Loams light-compose direction (Loams-side).
