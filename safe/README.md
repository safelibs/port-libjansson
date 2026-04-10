# libjansson-safe

This directory carries the Rust port of `libjansson` while preserving the upstream C ABI, SONAME, and installed development surface expected by Ubuntu 24.04 consumers.

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

Verify the mirrored upstream corpus, build-tree exports, packaging surface, and installed-root compatibility with:

```sh
safe/scripts/sync-upstream-tests.sh --check
safe/scripts/check-exports.sh
safe/scripts/build-upstream-api-tests.sh --all
safe/scripts/run-upstream-api-tests.sh --all
safe/scripts/run-data-suites.sh valid invalid invalid-unicode encoding-flags
```

To exercise the installed packages instead of the build tree, extract or install the `.deb` files into a root and point the runners at that root:

```sh
install_root="$PWD/safe/.build/installed-root"
rm -rf "$install_root"
mkdir -p "$install_root"
dpkg-deb -x safe/dist/libjansson4_*.deb "$install_root"
dpkg-deb -x safe/dist/libjansson-dev_*.deb "$install_root"

safe/scripts/check-exports.sh --installed-root "$install_root"
safe/scripts/run-upstream-api-tests.sh --installed-dev --installed-root "$install_root" --all
safe/scripts/run-data-suites.sh --installed-dev --installed-root "$install_root" \
    valid invalid invalid-unicode encoding-flags
safe/scripts/check-link-compat.sh --installed-root "$install_root"
```

Run the downstream-dependent harnesses from the repository root with the parameterized entrypoint:

```sh
safe/scripts/build-deb.sh
./test-original.sh
JANSSON_IMPLEMENTATION=safe JANSSON_TEST_MODE=runtime ./test-original.sh
JANSSON_IMPLEMENTATION=safe JANSSON_TEST_MODE=build ./test-original.sh
./test-safe.sh
```

Mode semantics:

- `JANSSON_IMPLEMENTATION=original` keeps the original runtime baseline as the default. Runtime mode builds upstream `original/jansson-2.14` into `/usr/local` and exercises the existing smoke tests against that overlay.
- In `JANSSON_TEST_MODE=build`, `JANSSON_IMPLEMENTATION=original` uses Ubuntu's archive `libjansson4` and `libjansson-dev` packages as the package-manager compile baseline.
- `JANSSON_IMPLEMENTATION=safe` consumes the prebuilt `safe/dist/libjansson4_*.deb` and `safe/dist/libjansson-dev_*.deb` artifacts, installs them with `dpkg -i`, and exercises the same downstream smoke tests as an actual system-package replacement.
- `JANSSON_TEST_MODE=build` skips the runtime smoke tests and instead invokes [`safe/scripts/check-dependent-builds.sh`](/home/yans/safelibs/port-libjansson/safe/scripts/check-dependent-builds.sh).
- `JANSSON_TEST_MODE=runtime` only runs the downstream binary smoke tests.
- `JANSSON_TEST_MODE=all` runs the package-based dependent rebuild harness first and then the runtime smoke tests.

The downstream matrix is driven directly from [`dependents.json`](/home/yans/safelibs/port-libjansson/dependents.json), which remains the one source of truth for the counted application inventory. The current Ubuntu 24.04 manifest contains these 12 unique `source_package` entries:

- `emacs`
- `janus`
- `jose`
- `jshon`
- `libteam`
- `mtr`
- `nghttp2`
- `suricata`
- `tang`
- `ulogd2`
- `wayvnc`
- `webdis`

[`safe/scripts/check-dependent-builds.sh`](/home/yans/safelibs/port-libjansson/safe/scripts/check-dependent-builds.sh) consumes that manifest directly. In safe mode it installs and pins the locally built `libjansson4` and `libjansson-dev` packages before rebuilding every unique source package in the manifest.

Build the reusable prepared-image scaffold with:

```sh
safe/scripts/build-dependent-image.sh --implementation safe --tag libjansson-safe-matrix:local
```

That image workflow installs the same Debian packages the rest of the verification path uses. It resolves the 12 primary application binaries from [`dependents.json`](/home/yans/safelibs/port-libjansson/dependents.json), installs the build/runtime prerequisite union already encoded in [`test-original.sh`](/home/yans/safelibs/port-libjansson/test-original.sh), and adds only the extra helper binaries required to exercise manifest entries, currently `nghttp2-server`.

When `--implementation safe` is selected, [`safe/scripts/build-dependent-image.sh`](/home/yans/safelibs/port-libjansson/safe/scripts/build-dependent-image.sh) reuses any preexisting `safe/dist/libjansson4_*.deb` and `safe/dist/libjansson-dev_*.deb` artifacts in place and only falls back to [`safe/scripts/build-deb.sh`](/home/yans/safelibs/port-libjansson/safe/scripts/build-deb.sh) when those Debian packages are missing.

The bulk API and data-suite runners always consume the checked-in mirror under `safe/tests/`; refresh that mirror only through `safe/scripts/sync-upstream-tests.sh --sync`.

Compatibility scope and verification details live in [COMPATIBILITY.md](/home/yans/safelibs/port-libjansson/safe/COMPATIBILITY.md).
