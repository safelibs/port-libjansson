# Compatibility Guarantees

This document is the authoritative compatibility contract for the safe port as an Ubuntu-compatible `libjansson` replacement. `safe/scripts/full-verify.sh` is the single-entry release gate and is expected to cover the same matrix described below end to end.

## Source Compatibility

- The installed public headers remain `/usr/include/jansson.h` and `/usr/include/jansson_config.h`.
- The mirrored upstream API corpus lives under `safe/tests/upstream-api/**` and is kept synchronized with `original/jansson-2.14/test/suites/api/**` by `safe/scripts/sync-upstream-tests.sh`.
- Build-tree source compatibility is verified by:
  - `safe/scripts/sync-upstream-tests.sh --check`
  - `safe/scripts/build-upstream-api-tests.sh --all`
  - `safe/scripts/run-upstream-api-tests.sh --all`
- Installed-dev source compatibility is verified against an extracted installed root under `safe/.build/installed-root-final`:
  - `dpkg-deb -x safe/dist/libjansson4_*.deb "$install_root"`
  - `dpkg-deb -x safe/dist/libjansson-dev_*.deb "$install_root"`
  - `env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/build-upstream-api-tests.sh --installed-dev --installed-root "$install_root" --all`
  - `env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/run-upstream-api-tests.sh --installed-dev --installed-root "$install_root" --all`
- Mixed-header link-source compatibility is verified by staging the original upstream `jansson.h` text with the installed `jansson_config.h`, then compiling upstream consumer objects against that mixed surface in `safe/scripts/check-link-compat.sh`.

## Link Compatibility

- The shared-library SONAME remains `libjansson.so.4`.
- Export names are checked against `original/jansson-2.14/src/jansson.def`.
- Exported symbol versions are checked against `original/jansson-2.14/debian/libjansson4.symbols`.
- Installed-package link compatibility is verified against the extracted installed root by:
  - `env -u ... safe/scripts/check-exports.sh --installed-root "$install_root" --check-versions`
  - `env -u ... safe/scripts/check-link-compat.sh --installed-root "$install_root"`
  - compiling `original/jansson-2.14/examples/simple_parse.c` through `pkg-config` configured with `PKG_CONFIG_LIBDIR="$install_root/usr/lib/$multiarch/pkgconfig:$install_root/usr/lib/pkgconfig:$install_root/usr/share/pkgconfig"` and `PKG_CONFIG_SYSROOT_DIR="$install_root"`
  - compiling that same example against `"$install_root/usr/lib/$multiarch/libjansson.a"`
- `safe/scripts/check-link-compat.sh` links the original upstream API-test objects, `json_process`, and the `simple_parse` example against the installed safe shared library and rejects any `ldd` resolution outside the selected installed root.

## Runtime Compatibility

- Build-tree runtime compatibility is verified against the mirrored `safe/tests/upstream-*` corpus by:
  - `safe/scripts/run-upstream-api-tests.sh --all`
  - `safe/scripts/run-data-suites.sh valid invalid invalid-unicode encoding-flags`
- Installed-package runtime compatibility is verified against the extracted installed root by rerunning the same mirrored corpus with build-tree search paths scrubbed:
  - `env -u ... safe/scripts/run-upstream-api-tests.sh --installed-dev --installed-root "$install_root" --all`
  - `env -u ... safe/scripts/run-data-suites.sh --installed-dev --installed-root "$install_root" valid invalid invalid-unicode encoding-flags`
- `safe/scripts/in-container-dependent-tests.sh` is the authoritative in-container downstream runtime/build harness.
- `safe/scripts/run-dependent-image-tests.sh` is the authoritative host-side prepared-image runner. `test-original.sh` and `test-safe.sh` are convenience wrappers around the same prepared-image workflow and accept:
  - `JANSSON_IMPLEMENTATION=original|safe`
  - `JANSSON_TEST_MODE=build|runtime|all`
- In safe runtime mode the downstream smoke harness verifies that exercised binaries resolve `libjansson.so.4` from the selected safe installation and then runs smoke checks for the same 12 applications listed in `dependents.json`:
  - `emacs` via `emacs-nox`
  - `janus` via `janus`
  - `jose` via `jose`
  - `jshon` via `jshon`
  - `libteam` via `teamd` and `teamdctl`
  - `mtr` via `mtr-tiny`
  - `nghttp2` via `nghttp2-client`, using `nghttp2-server` only as the local fixture helper
  - `suricata` via `suricata`
  - `tang` via `tang-common`
  - `ulogd2` via `ulogd2-json`
  - `wayvnc` via `wayvnc` and `wayvncctl`
  - `webdis` via `webdis`
- The `nghttp2-client` runtime check starts `nghttpd`, requests a local resource with `nghttp --har=...`, verifies that the HAR output parses as JSON with a top-level `log` object, and also asserts that the exercised `nghttp` binary resolves the selected `libjansson.so.4`.
- The ulogd JSON plugin check resolves the installed plugin path from package contents instead of hard-coding an architecture path.
- Every downstream runtime/build run writes raw logs under `safe/.build/dependent-matrix/<implementation>/<mode>/<application>/...` and updates the checked-in issue inventory at `safe/tests/regressions/discovered-issues.md`.

## Prepared Image Compatibility

- `safe/docker/dependent-matrix.Dockerfile` is the reusable Ubuntu 24.04 scaffold for the downstream dependent matrix.
- `safe/scripts/build-dependent-image.sh` resolves the primary application binaries directly from `dependents.json`, so the checked-in manifest remains the authoritative source of truth for the counted matrix.
- `safe/scripts/run-dependent-image-tests.sh` is the prepared-image execution entrypoint used by the release gate. It mounts the repository into the prepared image and invokes the shared in-container harness instead of maintaining a second copy of the smoke/build logic.
- The prepared image installs the union of build/runtime prerequisites, the 12 primary manifest binaries, and only the extra helper binaries required to exercise a manifest entry. Today that extra helper is `nghttp2-server` for the `nghttp2` runtime case.
- In safe mode the image builder reuses any preexisting `safe/dist/libjansson4_*.deb` and `safe/dist/libjansson-dev_*.deb` artifacts, and only invokes `safe/scripts/build-deb.sh` when those Debian packages are missing.
- The prepared-image path always installs `libjansson4` and `libjansson-dev` through Debian packages, so the image consumes the same package artifacts that the rest of the verification workflow exercises.
- `test-safe.sh` first prepares `libjansson-dependent-matrix:safe` and then delegates to `safe/scripts/run-dependent-image-tests.sh`. `test-original.sh` does the same for `libjansson-dependent-matrix:original` unless `JANSSON_IMPLEMENTATION=safe` is selected explicitly.
- Prepared-image runs persist deterministic logs under `safe/.build/dependent-matrix/<implementation>/<mode>/...` and write or update stable `APP-*` entries in `safe/tests/regressions/discovered-issues.md`.

## Downstream Build Compatibility

- `safe/scripts/check-dependent-builds.sh` is the authoritative compile-compatibility harness for every unique `source_package` in `dependents.json`.
- `safe/scripts/run-dependent-image-tests.sh --image "$image_tag" --implementation safe --mode build` runs that harness inside the prepared image after installing the locally built replacement packages.
- The 12-source-package matrix is defined by `dependents.json`. The current Ubuntu 24.04 manifest rebuilds exactly:
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
- In safe mode it installs the locally built `libjansson4` and `libjansson-dev` packages first, pins their exact versions, marks them held, and aborts on any package-version drift so `apt-get build-dep` cannot silently replace the candidate under test.
- The per-package rebuild sequence is:
  - enable `deb-src` entries if the image lacks them
  - `apt-get source "$srcpkg"`
  - `apt-get build-dep -y "$srcpkg"`
  - `DEB_BUILD_OPTIONS='nocheck nostrip noautodbgsym' dpkg-buildpackage -B -uc -us`
- The harness disables stripping and auto-dbgsym generation because the downstream matrix is validating compile compatibility against the selected `libjansson`, not debug-symbol packaging behavior.
- The Emacs special case also keeps `EMACS_INHIBIT_NATIVE_COMPILATION=1` during `dpkg-buildpackage` so native-compilation artifacts do not obscure the libjansson dependency edge being verified.
- Build-stage failures are collected per package under `safe/.build/dependent-matrix/<implementation>/build/<source-package>/...` so the issue inventory can point to deterministic logs without stopping at the first package failure.

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
- The installed development surface is verified inside the extracted installed root at Ubuntu-standard paths:
  - `$install_root/usr/lib/$multiarch/libjansson.so.4.14.0`
  - `$install_root/usr/lib/$multiarch/libjansson.so.4`
  - `$install_root/usr/lib/$multiarch/libjansson.so`
  - `$install_root/usr/lib/$multiarch/libjansson.a`
  - `$install_root/usr/include/jansson.h`
  - `$install_root/usr/include/jansson_config.h`
  - `$install_root/usr/lib/$multiarch/pkgconfig/jansson.pc`
- The final verifier intentionally unsets `PKG_CONFIG_PATH`, `LD_LIBRARY_PATH`, `LIBRARY_PATH`, `CPATH`, and `C_INCLUDE_PATH` during installed-root validation so compatibility checks cannot silently fall back to `safe/target/release`, `safe/include`, or any other build-tree artifact.
- Host `dpkg -i` into `/` remains a convenience workflow for local debugging only; it is not the release gate.

## Regression Coverage

- `safe/scripts/run-regressions.sh` is the authoritative checked-in regression runner.
- It consumes `safe/tests/regressions/manifest.json`, executes the checked-in shell/image cases, and requires every `APP-*` issue ID still present in `safe/tests/regressions/discovered-issues.md` to have manifest coverage.
- A clean issue inventory does not disable the regression gate; the nghttp2 image cases remain mandatory coverage even when `safe/tests/regressions/discovered-issues.md` reports no open application-level regressions.

## Security-Relevant Compatibility Guarantees

- Object hashing is randomized by default. `json_object()` seeds the process-global object hasher on first use; `json_object_seed()` only establishes a deterministic seed if it runs before that first use.
- Parsing is iterative and depth-bounded. `safe/src/load.rs` maintains an explicit heap `Vec<Frame>` and enforces `JSON_PARSER_MAX_DEPTH = 2048`, so attacker-supplied nesting does not recurse on the process stack.
- The mirrored upstream corpus under `safe/tests/` is part of the compatibility contract. The final verifier uses the mirrored `safe/tests/upstream-api/**`, `safe/tests/upstream-bin/**`, `safe/tests/upstream-scripts/**`, and `safe/tests/upstream-suites/**` corpus by default and touches `original/jansson-2.14/test/**` only through the explicit sync/check mechanism.

## Versioning Rules

- The Debian package version may advance independently in order to sort higher than Ubuntu's archive version.
- The upstream API version remains fixed at `2.14`.
- `jansson_version_str()` and `pkg-config --modversion jansson` therefore continue reporting `2.14`.
