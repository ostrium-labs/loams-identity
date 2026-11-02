//! Management API over gRPC.
//!
//! Specification: spec/17-management-api.md; ADR 0020.
//!
//! Reaches the outside world only through the traits in `loams-identity-core` (ARCH-2).
//! The determinism lint (ARCH-7, ADR 0016) applies to this crate.

// Test code is exempt from the determinism lint (ADR 0016).
#![cfg_attr(test, allow(clippy::disallowed_methods, clippy::disallowed_types))]
