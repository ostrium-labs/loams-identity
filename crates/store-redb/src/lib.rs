//! `EventStore` and `MetaStore` on redb (embedded, single node, development).
//!
//! Specification: spec/05-event-store-and-projections.md §1, §2, §4, §6.1.
//!
//! May do I/O; exempt from the determinism lint (ADR 0016).

// Exempt from the determinism lint (ADR 0016): this crate may use real time, randomness and I/O.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
