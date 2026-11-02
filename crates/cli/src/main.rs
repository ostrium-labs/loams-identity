//! The `loams-identity` binary: `serve`, `setup`, `migrate`, `keys` and `admin` subcommands, and configuration loading.
//!
//! Specification: spec/01-architecture.md §2, §7; spec/17-management-api.md.
//!
//! Binds listeners and wires the real implementations (ARCH-2); exempt from the determinism lint (ADR 0016).

// Exempt from the determinism lint (ADR 0016): this crate may use real time, randomness and I/O.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

fn main() {}
