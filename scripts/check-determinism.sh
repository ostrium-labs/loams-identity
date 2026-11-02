#!/usr/bin/env bash
# Grep backstop for the determinism lint (ARCH-8, ADR 0016): catches names that clippy's
# disallowed-methods and disallowed-types can miss (macros, re-exports, glob imports), and any
# attempt to switch the lint off in a linted crate.
#
# Test code is exempt (ADR 0016), so these are skipped: files named `tests.rs` or under a `tests/`
# directory, and any item that follows a line that is exactly `#[cfg(test)]` (a `mod tests { .. }`,
# a function, or a `mod tests;` declaration), up to its closing brace or semicolon.
#
# Usage: scripts/check-determinism.sh [workspace root]   (default: this repository)
set -euo pipefail
root_dir=${1:-"$(dirname "$0")/.."}
cd "$root_dir"
# shellcheck source=scripts/determinism-crates.sh
source scripts/determinism-crates.sh

# ARCH-7 names, plus direct file, network and process I/O (ARCH-1, ARCH-2).
pattern='SystemTime|Instant::now|Utc::now|Local::now|now_utc|now_local|thread_rng|rand::rng\b|rand::random|OsRng|ThreadRng|SysRng|getrandom|new_v4|now_v[0-9]|thread::spawn|thread::scope|tokio::spawn|spawn_blocking|spawn_local|new_random|std::fs\b|std::net\b|std::process\b|std::env::var|tokio::(fs|net|process)\b'
# The only allowed opt-out in a linted crate is the test-code exemption in its crate root.
test_exemption='#![cfg_attr(test, allow(clippy::disallowed_methods, clippy::disallowed_types))]'

# Prints "file:line:text" for every non-test line of the Rust files under $1.
non_test_lines() {
  find "$1" -name '*.rs' ! -name tests.rs ! -path '*/tests/*' -print0 | sort -z |
    xargs -0 -r awk '
      FNR == 1 { skipping = 0 }
      skipping {
        line = $0; sub(/\/\/.*/, "", line)
        opens = gsub(/\{/, "{", line); closes = gsub(/\}/, "}", line)
        depth += opens - closes
        if (opens > 0) opened = 1
        if ((opened && depth <= 0) || (!opened && line ~ /;[[:space:]]*$/)) skipping = 0
        next
      }
      /^[[:space:]]*#\[cfg\(test\)\][[:space:]]*$/ { skipping = 1; depth = 0; opened = 0; next }
      { print FILENAME ":" FNR ":" $0 }
    '
}

status=0
while read -r crate root; do
  if ! grep -Eq '^\[lints\]' "crates/$crate/Cargo.toml" || ! grep -Eq '^workspace = true' "crates/$crate/Cargo.toml"; then
    echo "::error file=crates/$crate/Cargo.toml::missing [lints] workspace = true"
    status=1
  fi
  is_exempt "$crate" && continue
  if non_test_lines "crates/$crate/src" | grep -E "$pattern"; then
    echo "::error file=crates/$crate::name forbidden by ARCH-7 outside test code (see lines above)"
    status=1
  fi
  if non_test_lines "crates/$crate/src" | grep -E 'disallowed[_-](methods|types)' |
      grep -vF "$root:"; then
    echo "::error file=crates/$crate::determinism lint mentioned outside the crate root"
    status=1
  fi
  if grep -n 'disallowed_' "$root" | grep -vF "$test_exemption"; then
    echo "::error file=$root::the determinism lint may only be relaxed for test code"
    status=1
  fi
  if [[ -e crates/$crate/clippy.toml || -e crates/$crate/.clippy.toml ]]; then
    echo "::error file=crates/$crate::a crate-level clippy.toml would replace the shared lint list"
    status=1
  fi
done < <(all_crates)

if [[ $status -eq 0 ]]; then echo "determinism grep: ok"; fi
exit "$status"
