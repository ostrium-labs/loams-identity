//! Real implementations of the I/O traits: clock, random source, mailer, SMS sender, key store, cache, event sinks and HTTP client.
//!
//! Specification: spec/01-architecture.md §2, §3; ADR 0016.
//!
//! The one library crate allowed to touch the system clock, the OS random source and the network (ADR 0016).

// Exempt from the determinism lint (ADR 0016): this crate may use real time, randomness and I/O.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
