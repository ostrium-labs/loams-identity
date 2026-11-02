# Sourced by the determinism checks. ADR 0016 exempts these crates from the determinism lint
# (ARCH-7): runtime, cli and the store crates; conformance is test-only glue (01 section 2).
# Every other crate under crates/ is linted, so a new crate is covered by default.
# shellcheck shell=bash
DETERMINISM_EXEMPT=(runtime cli store-redb store-postgres conformance)

is_exempt() {
  local c
  for c in "${DETERMINISM_EXEMPT[@]}"; do [[ $c == "$1" ]] && return 0; done
  return 1
}

# Prints "<dir> <crate root file>" for every crate under crates/.
all_crates() {
  local d
  for d in crates/*/; do
    d=${d%/}
    if [[ -f $d/src/lib.rs ]]; then echo "${d#crates/} $d/src/lib.rs"
    elif [[ -f $d/src/main.rs ]]; then echo "${d#crates/} $d/src/main.rs"
    else echo "::error file=$d::no src/lib.rs or src/main.rs" >&2; return 1
    fi
  done
}
