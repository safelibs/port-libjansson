#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

cargo build --manifest-path safe/Cargo.toml --release
test -f safe/target/release/libjansson.a
safe/scripts/sync-upstream-tests.sh --check
safe/scripts/build-upstream-api-tests.sh --all
safe/scripts/run-upstream-api-tests.sh --all
safe/scripts/run-data-suites.sh valid invalid invalid-unicode encoding-flags
safe/scripts/check-exports.sh --check-versions
safe/scripts/build-deb.sh
test "$(dpkg-deb -f safe/dist/libjansson4_*.deb Package)" = "libjansson4"
test "$(dpkg-deb -f safe/dist/libjansson4_*.deb Architecture)" = "$(dpkg --print-architecture)"
test "$(dpkg-deb -f safe/dist/libjansson4_*.deb Multi-Arch)" = "same"
dpkg-deb -f safe/dist/libjansson4_*.deb Depends | grep -q .
grep -F 'Architecture: any' safe/pkg/DEBIAN/control.libjansson4
grep -F 'Multi-Arch: same' safe/pkg/DEBIAN/control.libjansson4
grep -F 'Depends: ${shlibs:Depends}, ${misc:Depends}' safe/pkg/DEBIAN/control.libjansson4
grep -F 'Pre-Depends: ${misc:Pre-Depends}' safe/pkg/DEBIAN/control.libjansson4
test "$(dpkg-deb -f safe/dist/libjansson-dev_*.deb Package)" = "libjansson-dev"
test "$(dpkg-deb -f safe/dist/libjansson-dev_*.deb Architecture)" = "$(dpkg --print-architecture)"
test "$(dpkg-deb -f safe/dist/libjansson-dev_*.deb Multi-Arch)" = "same"
sh -c 'v="$(dpkg-deb -f safe/dist/libjansson-dev_*.deb Version)"; dpkg-deb -f safe/dist/libjansson-dev_*.deb Depends | grep -F "libjansson4 (= $v)"'
grep -F 'Architecture: any' safe/pkg/DEBIAN/control.libjansson-dev
grep -F 'Multi-Arch: same' safe/pkg/DEBIAN/control.libjansson-dev
grep -F 'Depends: libjansson4 (= ${binary:Version}), ${misc:Depends}' safe/pkg/DEBIAN/control.libjansson-dev
dpkg -i safe/dist/libjansson4_*.deb safe/dist/libjansson-dev_*.deb
ldconfig
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/check-exports.sh --installed-root / --check-versions
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/build-upstream-api-tests.sh --installed-dev --all
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/run-upstream-api-tests.sh --installed-dev --all
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/run-data-suites.sh --installed-dev valid invalid invalid-unicode encoding-flags
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH sh -c 'test "$(pkg-config --modversion jansson)" = "2.14"'
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"; test -f "/usr/lib/$multiarch/libjansson.a"
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH sh -c 'multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"; cc $(pkg-config --cflags jansson) original/jansson-2.14/examples/simple_parse.c $(pkg-config --libs jansson) -o /tmp/jansson-simple-parse && ldd /tmp/jansson-simple-parse | grep "/usr/lib/$multiarch/libjansson.so.4"'
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"; env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH cc -I/usr/include original/jansson-2.14/examples/simple_parse.c "/usr/lib/$multiarch/libjansson.a" -o /tmp/jansson-simple-parse-static
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/check-link-compat.sh --installed-root /
JANSSON_IMPLEMENTATION=safe JANSSON_TEST_MODE=build ./test-original.sh
JANSSON_IMPLEMENTATION=safe JANSSON_TEST_MODE=runtime ./test-original.sh
rg -n '\bunsafe\b|extern "C"|no_mangle' safe/src safe/csrc
