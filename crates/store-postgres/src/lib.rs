//! `EventStore` and `MetaStore` on PostgreSQL.
//!
//! Specification: spec/05-event-store-and-projections.md §1, §2, §4, §6.2; ADR 0003.
//!
//! May do I/O; exempt from the determinism lint (ADR 0016).

// Exempt from the determinism lint (ADR 0016): this crate may use real time, randomness and I/O.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
