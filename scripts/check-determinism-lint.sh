#!/usr/bin/env bash
# Proves the determinism checks work (milestone M1.1, ARCH-8, ADR 0016). In a temporary copy of
# the workspace, for each crate in turn:
#   1. plants `std::time::SystemTime::now()` in production code: clippy and the grep backstop
#      must reject it in every linted crate, and clippy must accept it in every exempt crate;
#   2. for linted crates, plants the same call in a `#[cfg(test)]` module only: clippy
#      (--all-targets) and the grep backstop must accept it, because test code is exempt;
#   3. for linted crates, plants it in production code after a `#[cfg(test)]` module: both must
#      reject it, so the test exemption does not hide the rest of the file.
# Run from anywhere; the working tree is never modified.
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tar -C "$repo" --exclude=./target --exclude=./.git -cf - . | tar -C "$work" -xf -
cd "$work"
# shellcheck source=scripts/determinism-crates.sh
source scripts/determinism-crates.sh
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$work/target}

prod='
/// Planted by scripts/check-determinism-lint.sh.
pub fn planted_wall_clock() -> u64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => 0,
    }
}'
test_only='
#[cfg(test)]
mod planted_tests {
    #[test]
    fn planted_wall_clock_in_test() {
        assert!(std::time::SystemTime::now() >= std::time::UNIX_EPOCH);
    }
}'

status=0
linted=0
fail() { echo "$1"; echo "::error::$2"; status=1; }

# run_case <crate root> <code> <clippy args...>: plants the code, runs clippy and the grep, and
# sets clippy_rc, clippy_out and grep_rc. The crate root is restored afterwards.
run_case() {
  local file=$1 code=$2; shift 2
  cp "$file" "$file.orig"
  printf '%s\n' "$code" >> "$file"
  set +e
  clippy_out=$(cargo clippy -q "$@" --message-format=short -- -D warnings 2>&1)
  clippy_rc=$?
  scripts/check-determinism.sh . >/dev/null 2>&1
  grep_rc=$?
  set -e
  mv "$file.orig" "$file"
}

while read -r crate root; do
  pkg="loams-identity-$crate"
  if is_exempt "$crate"; then
    run_case "$root" "$prod" -p "$pkg"
    if [[ $clippy_rc -ne 0 ]]; then
      fail "$clippy_out" "$pkg is exempt (ADR 0016) but clippy rejected the planted call"
    else
      echo "exempt: $pkg accepts the planted call"
    fi
    continue
  fi
  linted=$((linted + 1))

  run_case "$root" "$prod" -p "$pkg"
  if [[ $clippy_rc -eq 0 ]] || ! grep -q 'disallowed method `std::time::SystemTime::now`' <<<"$clippy_out"; then
    fail "$clippy_out" "$pkg: clippy accepted a planted SystemTime::now in production code"
  elif [[ $grep_rc -eq 0 ]]; then
    fail "" "$pkg: the grep backstop accepted a planted SystemTime::now in production code"
  else
    echo "linted: $pkg rejects the planted call (clippy and grep)"
  fi

  run_case "$root" "$test_only" -p "$pkg" --all-targets
  if [[ $clippy_rc -ne 0 ]]; then
    fail "$clippy_out" "$pkg: clippy rejected SystemTime::now in test code, which ADR 0016 exempts"
  elif [[ $grep_rc -ne 0 ]]; then
    fail "" "$pkg: the grep backstop rejected SystemTime::now in test code, which ADR 0016 exempts"
  else
    echo "linted: $pkg accepts the call in test code (clippy and grep)"
  fi

  run_case "$root" "$test_only$prod" -p "$pkg" --all-targets
  if [[ $clippy_rc -eq 0 ]]; then
    fail "$clippy_out" "$pkg: clippy accepted production code that follows a test module"
  elif [[ $grep_rc -eq 0 ]]; then
    fail "" "$pkg: the grep backstop accepted production code that follows a test module"
  else
    echo "linted: $pkg rejects production code after a test module (clippy and grep)"
  fi
done < <(all_crates)

if [[ $linted -eq 0 ]]; then echo "::error::no linted crates found"; status=1; fi
exit "$status"
