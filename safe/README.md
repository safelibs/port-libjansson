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

The bulk API and data-suite runners always consume the checked-in mirror under `safe/tests/`; refresh that mirror only through `safe/scripts/sync-upstream-tests.sh --sync`.

Compatibility scope and verification details live in [COMPATIBILITY.md](/home/yans/safelibs/port-libjansson/safe/COMPATIBILITY.md).
