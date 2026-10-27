//! Domain types, aggregates, commands, protocol state machines and the I/O trait definitions.
//!
//! Specification: spec/01-architecture.md §2 to §6.
//!
//! No I/O (ARCH-1).
//! The determinism lint (ARCH-7, ADR 0016) applies to this crate.

// Test code is exempt from the determinism lint (ADR 0016).
#![cfg_attr(test, allow(clippy::disallowed_methods, clippy::disallowed_types))]

mod ports;
pub use ports::*;

#[cfg(feature = "contract-tests")]
pub mod contracts;
