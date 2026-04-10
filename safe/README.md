# libjansson-safe

This directory carries the Rust port of `libjansson` while preserving the upstream C ABI, SONAME, and installed development surface expected by Ubuntu 24.04 consumers.

`safe/scripts/full-verify.sh` is the authoritative release gate. It runs the build-tree suites, allocator/container checks, checked-in regressions, Debian packaging checks, extracted installed-root verification under `safe/.build/installed-root-final`, the prepared-image downstream build/runtime matrix, and the final SAFETY audit.

Build the release artifacts and emit Ubuntu-compatible `.deb` packages with:

```sh
safe/scripts/build-deb.sh
```

That script rebuilds `safe/dist/` from scratch and emits exactly:

- `libjansson4_<version>_<arch>.deb`
- `libjansson-dev_<version>_<arch>.deb`

The package version is allowed to sort higher than Ubuntu's `2.14-2build2`, but the installed API/ABI version stays upstream-compatible:

- `JANSSON_VERSION` remains `2.14`
- `jansson_version_str()` returns `2.14`
- `pkg-config --modversion jansson` returns `2.14`
- the SONAME remains `libjansson.so.4`
- the exported symbol-version node remains `libjansson.so.4`

The build-tree compatibility and contract checks that feed the final gate are:

```sh
safe/scripts/sync-upstream-tests.sh --check
safe/scripts/check-allocator-hooks.sh
safe/scripts/check-container-primitives.sh
safe/scripts/check-exports.sh --check-versions
safe/scripts/build-upstream-api-tests.sh --all
safe/scripts/run-upstream-api-tests.sh --all
safe/scripts/run-data-suites.sh valid invalid invalid-unicode encoding-flags
safe/scripts/run-regressions.sh
```

The authoritative installed-package workflow is an extracted root, not a mutable host `/` install:

```sh
install_root="$PWD/safe/.build/installed-root-final"
rm -rf "$install_root"
mkdir -p "$install_root"
dpkg-deb -x safe/dist/libjansson4_*.deb "$install_root"
dpkg-deb -x safe/dist/libjansson-dev_*.deb "$install_root"

clean_env() {
  env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH "$@"
}

clean_env safe/scripts/check-exports.sh --installed-root "$install_root" --check-versions
clean_env safe/scripts/build-upstream-api-tests.sh --installed-dev --installed-root "$install_root" --all
clean_env safe/scripts/run-upstream-api-tests.sh --installed-dev --installed-root "$install_root" --all
clean_env safe/scripts/run-data-suites.sh --installed-dev --installed-root "$install_root" \
    valid invalid invalid-unicode encoding-flags
clean_env safe/scripts/check-link-compat.sh --installed-root "$install_root"
```

For direct `pkg-config` and link validation against that extracted root, keep all paths expressed via `dpkg-architecture -qDEB_HOST_MULTIARCH`:

```sh
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
pkgcfg="$install_root/usr/lib/$multiarch/pkgconfig:$install_root/usr/lib/pkgconfig:$install_root/usr/share/pkgconfig"

env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH \
  PKG_CONFIG_DIR= \
  PKG_CONFIG_LIBDIR="$pkgcfg" \
  PKG_CONFIG_SYSROOT_DIR="$install_root" \
  pkg-config --modversion jansson
```

The downstream matrix is driven directly from `dependents.json`. The authoritative 12-application set is:

- `emacs` via `emacs-nox`
- `janus` via `janus`
- `jose` via `jose`
- `jshon` via `jshon`
- `libteam` via `libteam-utils` (`teamd`, `teamdctl`)
- `mtr` via `mtr-tiny`
- `nghttp2` via `nghttp2-client`, with `nghttp2-server` only as the local fixture helper
- `suricata` via `suricata`
- `tang` via `tang-common`
- `ulogd2` via `ulogd2-json`
- `wayvnc` via `wayvnc` and `wayvncctl`
- `webdis` via `webdis`

The authoritative prepared-image workflow is:

```sh
image_tag="libjansson-safe-matrix:local"
safe/scripts/build-dependent-image.sh --implementation safe --tag "$image_tag"
safe/scripts/run-dependent-image-tests.sh --image "$image_tag" --implementation safe --mode build
safe/scripts/run-dependent-image-tests.sh --image "$image_tag" --implementation safe --mode runtime
```

`safe/scripts/run-dependent-image-tests.sh` expects a prepared image tag built by `safe/scripts/build-dependent-image.sh`. `test-safe.sh` is only a convenience wrapper around that same workflow; it builds `libjansson-dependent-matrix:safe` by default and then delegates to `safe/scripts/run-dependent-image-tests.sh`.

When `--implementation safe` is selected, `safe/scripts/build-dependent-image.sh` reuses any preexisting `safe/dist/libjansson4_*.deb` and `safe/dist/libjansson-dev_*.deb` artifacts in place and only falls back to `safe/scripts/build-deb.sh` when those packages are missing.

Every build/runtime matrix run writes per-application logs under `safe/.build/dependent-matrix/` and updates `safe/tests/regressions/discovered-issues.md`. Clean runs are recorded explicitly; failing runs preserve stable `APP-*` issue IDs in place.

The checked-in regression runner is `safe/scripts/run-regressions.sh`. It consumes `safe/tests/regressions/manifest.json`, requires coverage for every `APP-*` entry still present in `safe/tests/regressions/discovered-issues.md`, and still requires the nghttp2 image regression cases even when the issue inventory is clean.

Legacy `dpkg -i` into host `/` can still be useful for local debugging, but it is not the release gate. Compatibility scope and verification details live in `safe/COMPATIBILITY.md`, and the residual unsafe surface is documented in `safe/SAFETY.md`.
