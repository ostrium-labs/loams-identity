# ADR 0023: Own simulation harness on a single-threaded runtime with paused time

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

The owner requires deterministic simulation testing with fault injection: crashes between append
and projection, duplicated and reordered deliveries, clock skew and jumps, store timeouts, partial
writes. Same seed, same run.

## Candidates (crates.io, 2026-10-21)

| Option | Version, last release | Licence | Fit |
|---|---|---|---|
| Own harness on `tokio` current-thread runtime with `start_paused` | tokio 1.54.0 | MIT | Our I/O is already behind traits, so we only need a deterministic executor and seeded fakes |
| `madsim` | 0.2.34, 2025-10-11 | Apache-2.0 | Replaces tokio and std APIs with simulated ones; strong, but it patches the ecosystem and its last release is a year old |
| `turmoil` | 0.7.2, 2026-04-24 | MIT | Simulates networks between hosts; useful for multi-node tests, not needed while the core has no network I/O |

## Decision

- The `sim` crate drives the real handlers on a tokio current-thread runtime with paused time,
  with every trait replaced by a seeded fake that can inject faults (VER-12).
- Background work (projections, outbox, fan-out) is registered with a scheduler trait; in the
  simulator the scheduler runs tasks in an order chosen by the seed, one step at a time, so
  interleavings are explored deliberately rather than left to the runtime.
- No `tokio::spawn` in core code (ARCH-7), so the runtime's own scheduling order never reaches
  the outcome.
- `turmoil` may be added later for multi-node PostgreSQL scenarios.

## Alternatives considered

- **madsim.** Considered seriously; rejected for now because it swaps out tokio crate-wide and
  its release cadence has slowed. Revisit if our own harness proves too weak.
- **No DST; only property tests.** Does not cover crash and delivery faults.

## Consequences

The discipline in ARCH-7 to ARCH-11 is what makes the harness work; the lint is the guard.

## Derived from

FoundationDB-style simulation testing as a general method; tokio, madsim and turmoil
documentation; crates.io metadata checked 2026-10-21.
