#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

INSTALL_ROOT="${ROOT_DIR}/safe/.build/installed-root-final"
IMAGE_TAG="${IMAGE_TAG:-libjansson-safe-matrix:final}"
SAFETY_DOC="${ROOT_DIR}/safe/SAFETY.md"

note() {
  printf '\n==> %s\n' "$1"
}

fail() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

clean_env() {
  env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH "$@"
}

pkg_config_env() {
  local install_root="$1"
  local multiarch="$2"
  shift 2

  clean_env \
    PKG_CONFIG_DIR= \
    PKG_CONFIG_LIBDIR="${install_root}/usr/lib/${multiarch}/pkgconfig:${install_root}/usr/lib/pkgconfig:${install_root}/usr/share/pkgconfig" \
    PKG_CONFIG_SYSROOT_DIR="${install_root}" \
    "$@"
}

verify_safety_doc_coverage() {
  local audit_pattern='\bunsafe\b|extern "C"|no_mangle'
  local file
  local audit_files=()
  local c_surface_files=(
    "safe/csrc/pack_unpack_shim.c"
    "safe/csrc/sprintf_shim.c"
  )

  note "Auditing residual unsafe and exported C ABI surface"
  rg -n "${audit_pattern}" safe/src safe/csrc

  mapfile -t audit_files < <(rg --files-with-matches "${audit_pattern}" safe/src safe/csrc | sort)
  [ "${#audit_files[@]}" -gt 0 ] || fail "Residual safety audit unexpectedly returned no files"

  for file in "${audit_files[@]}"; do
    if ! grep -Fq "\`${file}\`" "${SAFETY_DOC}"; then
      fail "Residual audit file ${file} is not documented in ${SAFETY_DOC}"
    fi
  done

  for file in "${c_surface_files[@]}"; do
    if ! grep -Fq "\`${file}\`" "${SAFETY_DOC}"; then
      fail "Residual C shim ${file} is not documented in ${SAFETY_DOC}"
    fi
  done

  note "Confirmed SAFETY coverage for ${#audit_files[@]} residual Rust audit files and ${#c_surface_files[@]} C shim files"
}

note "Building release artifacts"
cargo build --manifest-path safe/Cargo.toml --release
test -f safe/target/release/libjansson.a
cargo test --manifest-path safe/Cargo.toml --release
cargo test --manifest-path safe/Cargo.toml --release parser_depth_limit_ --lib
cargo test --manifest-path safe/Cargo.toml --release container_seed_contract --lib

note "Running build-tree compatibility and contract checks"
safe/scripts/sync-upstream-tests.sh --check
safe/scripts/check-allocator-hooks.sh
safe/scripts/check-container-primitives.sh
safe/scripts/check-exports.sh --check-versions
safe/scripts/build-upstream-api-tests.sh --all
safe/scripts/run-upstream-api-tests.sh --all
safe/scripts/run-data-suites.sh valid invalid invalid-unicode encoding-flags

note "Building Debian packages"
safe/scripts/build-deb.sh
test "$(dpkg-deb -f safe/dist/libjansson4_*.deb Package)" = "libjansson4"
test "$(dpkg-deb -f safe/dist/libjansson-dev_*.deb Package)" = "libjansson-dev"
test "$(dpkg-deb -f safe/dist/libjansson4_*.deb Architecture)" = "$(dpkg --print-architecture)"
test "$(dpkg-deb -f safe/dist/libjansson-dev_*.deb Architecture)" = "$(dpkg --print-architecture)"
test "$(dpkg-deb -f safe/dist/libjansson4_*.deb Multi-Arch)" = "same"
test "$(dpkg-deb -f safe/dist/libjansson-dev_*.deb Multi-Arch)" = "same"
dpkg-deb -f safe/dist/libjansson4_*.deb Depends | grep -q .
grep -F 'Architecture: any' safe/pkg/DEBIAN/control.libjansson4
grep -F 'Multi-Arch: same' safe/pkg/DEBIAN/control.libjansson4
grep -F 'Depends: ${shlibs:Depends}, ${misc:Depends}' safe/pkg/DEBIAN/control.libjansson4
grep -F 'Pre-Depends: ${misc:Pre-Depends}' safe/pkg/DEBIAN/control.libjansson4
grep -F 'Architecture: any' safe/pkg/DEBIAN/control.libjansson-dev
grep -F 'Multi-Arch: same' safe/pkg/DEBIAN/control.libjansson-dev
grep -F 'Depends: libjansson4 (= ${binary:Version}), ${misc:Depends}' safe/pkg/DEBIAN/control.libjansson-dev
sh -c 'v="$(dpkg-deb -f safe/dist/libjansson-dev_*.deb Version)"; dpkg-deb -f safe/dist/libjansson-dev_*.deb Depends | grep -F "libjansson4 (= $v)"'

note "Running checked-in regression cases"
safe/scripts/run-regressions.sh

note "Preparing extracted installed root"
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
rm -rf "${INSTALL_ROOT}"
mkdir -p "${INSTALL_ROOT}"
dpkg-deb -x safe/dist/libjansson4_*.deb "${INSTALL_ROOT}"
dpkg-deb -x safe/dist/libjansson-dev_*.deb "${INSTALL_ROOT}"

note "Running installed-root compatibility checks"
clean_env safe/scripts/check-exports.sh --installed-root "${INSTALL_ROOT}" --check-versions
clean_env safe/scripts/build-upstream-api-tests.sh --installed-dev --installed-root "${INSTALL_ROOT}" --all
clean_env safe/scripts/run-upstream-api-tests.sh --installed-dev --installed-root "${INSTALL_ROOT}" --all
clean_env safe/scripts/run-data-suites.sh --installed-dev --installed-root "${INSTALL_ROOT}" \
  valid invalid invalid-unicode encoding-flags
pkg_config_env "${INSTALL_ROOT}" "${multiarch}" pkg-config --modversion jansson | grep -Fx '2.14'
test -f "${INSTALL_ROOT}/usr/lib/${multiarch}/libjansson.a"
pkg_config_env "${INSTALL_ROOT}" "${multiarch}" \
  sh -c 'install_root="$1"; multiarch="$2"; cc $(pkg-config --cflags jansson) original/jansson-2.14/examples/simple_parse.c $(pkg-config --libs jansson) -Wl,-rpath,"$install_root/usr/lib/$multiarch" -o /tmp/jansson-simple-parse-dynamic-final' \
  sh "${INSTALL_ROOT}" "${multiarch}"
clean_env ldd /tmp/jansson-simple-parse-dynamic-final | grep -F "${INSTALL_ROOT}/usr/lib/${multiarch}/libjansson.so.4"
clean_env cc -I"${INSTALL_ROOT}/usr/include" original/jansson-2.14/examples/simple_parse.c \
  "${INSTALL_ROOT}/usr/lib/${multiarch}/libjansson.a" \
  -o /tmp/jansson-simple-parse-static-final
clean_env safe/scripts/check-link-compat.sh --installed-root "${INSTALL_ROOT}"

note "Running prepared-image downstream matrix"
safe/scripts/build-dependent-image.sh --implementation safe --tag "${IMAGE_TAG}"
safe/scripts/run-dependent-image-tests.sh --image "${IMAGE_TAG}" --implementation safe --mode build
safe/scripts/run-dependent-image-tests.sh --image "${IMAGE_TAG}" --implementation safe --mode runtime

verify_safety_doc_coverage
