#!/usr/bin/env bash
# Grep backstop for the determinism lint (ARCH-8, ADR 0016): catches names that clippy's
# disallowed-methods and disallowed-types can miss (macros, re-exports, glob imports), and any
# attempt to switch the lint off in a linted crate. Run from the repository root.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/determinism-crates.sh
source scripts/determinism-crates.sh

# ARCH-7 names, plus direct file, network and process I/O (ARCH-1, ARCH-2).
pattern='SystemTime|Instant::now|Utc::now|Local::now|now_utc|now_local|thread_rng|rand::rng\b|rand::random|OsRng|ThreadRng|SysRng|getrandom|new_v4|now_v[0-9]|thread::spawn|thread::scope|tokio::spawn|spawn_blocking|spawn_local|new_random|std::fs\b|std::net\b|std::process\b|std::env::var|tokio::(fs|net|process)\b'
# The only allowed opt-out in a linted crate is the test-code exemption in its crate root.
test_exemption='#![cfg_attr(test, allow(clippy::disallowed_methods, clippy::disallowed_types))]'

status=0
while read -r crate root; do
  if ! grep -Eq '^\[lints\]' "crates/$crate/Cargo.toml" || ! grep -Eq '^workspace = true' "crates/$crate/Cargo.toml"; then
    echo "::error file=crates/$crate/Cargo.toml::missing [lints] workspace = true"
    status=1
  fi
  is_exempt "$crate" && continue
  if grep -rnE "$pattern" "crates/$crate/src"; then
    echo "::error file=crates/$crate::name forbidden by ARCH-7 (see lines above)"
    status=1
  fi
  if grep -rn 'disallowed_methods\|disallowed_types\|disallowed-methods\|disallowed-types' "crates/$crate/src" |
      grep -vF "$root:" ; then
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
