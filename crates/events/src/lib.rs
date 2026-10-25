//! Event envelope, event payload types, schema versions and upcasters.
//!
//! Specification: spec/04-event-catalog.md; spec/01-architecture.md §2.
//!
//! No I/O (ARCH-1).
//! The determinism lint (ARCH-7, ADR 0016) applies to this crate.

// Test code is exempt from the determinism lint (ADR 0016).
#![cfg_attr(test, allow(clippy::disallowed_methods, clippy::disallowed_types))]
