#!/usr/bin/env bash
# Field Soft packaging gate: Soft crates + e24 without gpu must build/test
# without linking next-gui-render-gpu or wgpu.
#
# Local:  ./tools/ci/field-soft.sh
# CI:     .github/workflows/ci.yml job field-soft
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

RED=$'\033[31m'
GRN=$'\033[32m'
YLW=$'\033[33m'
RST=$'\033[0m'

pass=0
fail=0

ok() { echo "${GRN}OK${RST}  $*"; pass=$((pass + 1)); }
bad() { echo "${RED}FAIL${RST} $*"; fail=$((fail + 1)); }
info() { echo "${YLW}--${RST} $*"; }

# Soft-only packages that Field packaging may ship (no render-gpu dep today).
FIELD_PKGS=(
  next-gui-profile
  next-gui-scene
  next-gui-layout
  next-gui-ui
  next-gui-text
  next-gui-render
  next-gui-render-soft
  next-gui-platform
  next-gui-immediate
  e01_soft_window
  e02_counter
  e03_ngui_counter
  e04_cjk_text
)

FORBIDDEN_RE='wgpu|next-gui-render-gpu'

assert_no_gpu_in_tree() {
  local label="$1"
  shift
  local tree
  # shellcheck disable=SC2068
  if ! tree="$(cargo tree $@ 2>&1)"; then
    bad "$label: cargo tree failed"
    echo "$tree" | sed 's/^/    /'
    return
  fi
  if echo "$tree" | grep -E "$FORBIDDEN_RE" >/dev/null; then
    bad "$label: forbidden GPU dep in tree"
    echo "$tree" | grep -E "$FORBIDDEN_RE" | sed 's/^/    /' || true
  else
    ok "$label: no wgpu / next-gui-render-gpu"
  fi
}

echo "=== Field Soft CI ==="
echo "root: $ROOT"
echo

info "1/4 cargo tree — Field Soft package set"
for pkg in "${FIELD_PKGS[@]}"; do
  assert_no_gpu_in_tree "$pkg" -p "$pkg"
done

info "2/4 cargo tree — e24 --no-default-features (Soft-only dual profile)"
assert_no_gpu_in_tree "e24_profile_dual --no-default-features" \
  -p e24_profile_dual --no-default-features

info "3/4 build + test Field Soft crates"
PKG_ARGS=()
for pkg in "${FIELD_PKGS[@]}"; do
  PKG_ARGS+=(-p "$pkg")
done
if cargo test "${PKG_ARGS[@]}" -- --quiet; then
  ok "cargo test (Field Soft package set)"
else
  bad "cargo test (Field Soft package set)"
fi

info "4/4 build e24 Soft-only + default (gpu) check"
if cargo build -p e24_profile_dual --no-default-features; then
  ok "e24 build --no-default-features (Soft)"
else
  bad "e24 build --no-default-features"
fi
if cargo check -p e24_profile_dual; then
  ok "e24 check (default features / gpu)"
else
  bad "e24 check (default features / gpu)"
fi

echo
echo "=== summary: ${pass} passed, ${fail} failed ==="
if [[ "$fail" -ne 0 ]]; then
  exit 1
fi
echo "Field Soft packaging gate: PASS"
