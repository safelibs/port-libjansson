# Compatibility Guarantees

This document is the authoritative compatibility contract for the safe port as an Ubuntu-compatible `libjansson` replacement. The phase checker is mirrored by [`safe/scripts/full-verify.sh`](/home/yans/safelibs/port-libjansson/safe/scripts/full-verify.sh), which runs the same matrix end to end.

## Source Compatibility

- The installed public headers remain `/usr/include/jansson.h` and `/usr/include/jansson_config.h`.
- The mirrored upstream API corpus lives under `safe/tests/upstream-api/**` and is kept synchronized with `original/jansson-2.14/test/suites/api/**` by [`safe/scripts/sync-upstream-tests.sh`](/home/yans/safelibs/port-libjansson/safe/scripts/sync-upstream-tests.sh).
- Build-tree source compatibility is verified by:
  - `safe/scripts/sync-upstream-tests.sh --check`
  - `safe/scripts/build-upstream-api-tests.sh --all`
  - `safe/scripts/run-upstream-api-tests.sh --all`
- Installed-dev source compatibility is verified after `dpkg -i` by rebuilding and rerunning that same mirrored corpus against `/usr` only:
  - `env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/build-upstream-api-tests.sh --installed-dev --all`
  - `env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/run-upstream-api-tests.sh --installed-dev --all`
- Mixed-header link-source compatibility is verified by staging the original upstream `jansson.h` text with the installed `jansson_config.h`, then compiling upstream consumer objects against that mixed surface in [`safe/scripts/check-link-compat.sh`](/home/yans/safelibs/port-libjansson/safe/scripts/check-link-compat.sh).

## Link Compatibility

- The shared-library SONAME remains `libjansson.so.4`.
- Export names are checked against `original/jansson-2.14/src/jansson.def`.
- Exported symbol versions are checked against `original/jansson-2.14/debian/libjansson4.symbols`.
- Installed-package link compatibility is verified under `/usr` only by:
  - `env -u ... safe/scripts/check-exports.sh --installed-root / --check-versions`
  - `env -u ... safe/scripts/check-link-compat.sh --installed-root /`
  - compiling `original/jansson-2.14/examples/simple_parse.c` through installed `pkg-config`
  - compiling that same example against `/usr/lib/$multiarch/libjansson.a`
- [`safe/scripts/check-link-compat.sh`](/home/yans/safelibs/port-libjansson/safe/scripts/check-link-compat.sh) links the original upstream API-test objects, `json_process`, and `simple_parse` example against the installed safe shared library and rejects any `ldd` resolution outside the selected installed root.

## Runtime Compatibility

- Build-tree runtime compatibility is verified against the mirrored `safe/tests/upstream-*` corpus by:
  - `safe/scripts/run-upstream-api-tests.sh --all`
  - `safe/scripts/run-data-suites.sh valid invalid invalid-unicode encoding-flags`
- Installed-package runtime compatibility is verified after `dpkg -i` by rerunning the same mirrored corpus against the installed package under `/usr`, again with build-tree search paths scrubbed:
  - `env -u ... safe/scripts/run-upstream-api-tests.sh --installed-dev --all`
  - `env -u ... safe/scripts/run-data-suites.sh --installed-dev valid invalid invalid-unicode encoding-flags`
- [`test-original.sh`](/home/yans/safelibs/port-libjansson/test-original.sh) is the authoritative downstream runtime/build entrypoint and accepts:
  - `JANSSON_IMPLEMENTATION=original|safe`
  - `JANSSON_TEST_MODE=build|runtime|all`
- `JANSSON_IMPLEMENTATION=safe JANSSON_TEST_MODE=runtime ./test-original.sh` verifies real installed binaries and plugins against the safe package, asserts that each exercised binary resolves `libjansson.so.4` from the selected installation, and then runs smoke tests for:
  - `emacs`
  - `janus`
  - `jose`
  - `jshon`
  - `mtr`
  - `suricata`
  - `tang`
  - `teamd` / `teamdctl`
  - `ulogd2-json`
  - `wayvnc` / `wayvncctl`
  - `webdis`
- The ulogd JSON plugin check resolves the installed plugin path from package contents instead of hard-coding an architecture path.

## Downstream Build Compatibility

- [`safe/scripts/check-dependent-builds.sh`](/home/yans/safelibs/port-libjansson/safe/scripts/check-dependent-builds.sh) is the authoritative compile-compatibility harness for every unique `source_package` in [`dependents.json`](/home/yans/safelibs/port-libjansson/dependents.json).
- `JANSSON_IMPLEMENTATION=safe JANSSON_TEST_MODE=build ./test-original.sh` runs that harness inside a clean Ubuntu 24.04 container after installing the locally built replacement packages.
- The harness rebuilds exactly these Ubuntu 24.04 source packages:
  - `emacs`
  - `janus`
  - `jose`
  - `jshon`
  - `libteam`
  - `mtr`
  - `suricata`
  - `tang`
  - `ulogd2`
  - `wayvnc`
  - `webdis`
- In safe mode it installs the locally built `libjansson4` and `libjansson-dev` packages first, pins their exact versions, marks them held, and aborts on any package-version drift so `apt-get build-dep` cannot silently replace the candidate under test.
- The per-package rebuild sequence is:
  - enable `deb-src` entries if the image lacks them
  - `apt-get source "$srcpkg"`
  - `apt-get build-dep -y "$srcpkg"`
  - `DEB_BUILD_OPTIONS=nocheck dpkg-buildpackage -B -uc -us`
- The Emacs special case keeps `EMACS_INHIBIT_NATIVE_COMPILATION=1` during `dpkg-buildpackage` so native-compilation artifacts do not obscure the libjansson dependency edge being verified.

## Packaging Compatibility

- The emitted binary package names match Ubuntu exactly: `libjansson4` and `libjansson-dev`.
- `libjansson4` preserves:
  - `Architecture: any`
  - `Multi-Arch: same`
  - `Depends: ${shlibs:Depends}, ${misc:Depends}`
  - `Pre-Depends: ${misc:Pre-Depends}`
- `libjansson-dev` preserves:
  - `Architecture: any`
  - `Multi-Arch: same`
  - `Depends: libjansson4 (= ${binary:Version}), ${misc:Depends}`
- The installed development surface is verified after `dpkg -i` at Ubuntu-standard paths:
  - `/usr/lib/$multiarch/libjansson.so.4.14.0`
  - `/usr/lib/$multiarch/libjansson.so.4`
  - `/usr/lib/$multiarch/libjansson.so`
  - `/usr/lib/$multiarch/libjansson.a`
  - `/usr/include/jansson.h`
  - `/usr/include/jansson_config.h`
  - `/usr/lib/$multiarch/pkgconfig/jansson.pc`
- Commands 19-27 of the final verifier intentionally unset `PKG_CONFIG_PATH`, `LD_LIBRARY_PATH`, `LIBRARY_PATH`, `CPATH`, and `C_INCLUDE_PATH` so installed-package validation cannot silently fall back to `safe/target/release`, `safe/include`, or any other build-tree artifact.

## Security-Relevant Compatibility Guarantees

- Object hashing is randomized by default. `json_object()` seeds the process-global object hasher on first use; `json_object_seed()` only establishes a deterministic seed if it runs before that first use.
- Parsing is iterative and depth-bounded. `safe/src/load.rs` maintains an explicit heap `Vec<Frame>` and enforces `JSON_PARSER_MAX_DEPTH = 2048`, so attacker-supplied nesting does not recurse on the process stack.
- The mirrored upstream corpus under `safe/tests/` is part of the compatibility contract. The final verifier uses the mirrored `safe/tests/upstream-api/**`, `safe/tests/upstream-bin/**`, `safe/tests/upstream-scripts/**`, and `safe/tests/upstream-suites/**` corpus by default and touches `original/jansson-2.14/test/**` only through the explicit sync/check mechanism.

## Versioning Rules

- The Debian package version may advance independently in order to sort higher than Ubuntu's archive version.
- The upstream API version remains fixed at `2.14`.
- `jansson_version_str()` and `pkg-config --modversion jansson` therefore continue reporting `2.14`.
