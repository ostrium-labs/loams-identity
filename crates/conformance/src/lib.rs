//! Glue for the OpenID Foundation conformance suite: test-plan configuration, client registrations and result parsing.
//!
//! Specification: spec/19-verification.md §2.
//!
//! Test-only crate that may do I/O; exempt from the determinism lint as test code (ADR 0016).

// Exempt from the determinism lint (ADR 0016): this crate may use real time, randomness and I/O.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
