//! Deterministic simulation harness: fake implementations of every I/O trait with fault injection, a seeded workload driver and invariant checkers.
//!
//! Specification: spec/19-verification.md §4; ADR 0023.
//!
//! No real I/O: time and randomness come from seeded fakes.
//! The determinism lint (ARCH-7, ADR 0016) applies to this crate.

// Test code is exempt from the determinism lint (ADR 0016).
#![cfg_attr(test, allow(clippy::disallowed_methods, clippy::disallowed_types))]
