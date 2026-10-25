#!/usr/bin/env bash
# Proves the determinism lint works (milestone M1.1, ARCH-8, ADR 0016). In a temporary copy of the
# workspace, plants `std::time::SystemTime::now()` in each crate in turn and runs clippy on it:
# every linted crate must be rejected with the disallowed-method error, and every exempt crate
# must pass. Run from anywhere; the working tree is never modified.
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tar -C "$repo" --exclude=./target --exclude=./.git -cf - . | tar -C "$work" -xf -
cd "$work"
# shellcheck source=scripts/determinism-crates.sh
source scripts/determinism-crates.sh
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$work/target}

plant='
/// Planted by scripts/check-determinism-lint.sh.
pub fn planted_wall_clock() -> u64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => 0,
    }
}'

status=0
linted=0
while read -r crate root; do
  cp "$root" "$root.orig"
  printf '%s\n' "$plant" >> "$root"
  pkg="loams-identity-$crate"
  set +e
  out=$(cargo clippy -q -p "$pkg" --message-format=short -- -D warnings 2>&1)
  rc=$?
  set -e
  mv "$root.orig" "$root"
  if is_exempt "$crate"; then
    if [[ $rc -ne 0 ]]; then
      echo "$out"
      echo "::error::$pkg is exempt (ADR 0016) but clippy rejected the planted call"
      status=1
    else
      echo "exempt: $pkg accepts the planted call"
    fi
  else
    linted=$((linted + 1))
    if [[ $rc -ne 0 ]] && grep -q 'disallowed method `std::time::SystemTime::now`' <<<"$out"; then
      echo "linted: $pkg rejects the planted call"
    else
      echo "$out"
      echo "::error::$pkg accepted a planted SystemTime::now; the determinism lint is not applied"
      status=1
    fi
  fi
done < <(all_crates)

if [[ $linted -eq 0 ]]; then echo "::error::no linted crates found"; status=1; fi
exit "$status"
