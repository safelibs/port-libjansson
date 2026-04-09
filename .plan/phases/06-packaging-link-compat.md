## Phase Name
Debian Packaging, Installed Dev Surface, Link Compatibility, And Mirrored Test Consumption

## Implement Phase ID
`impl_packaging_link_compat`

## Preexisting Inputs
Consume these artifacts in place. Do not rediscover or regenerate them.
- `safe/Cargo.toml`
- `safe/build.rs`
- `safe/csrc/pack_unpack_shim.c`
- `safe/csrc/sprintf_shim.c`
- `safe/include/jansson.h`
- `safe/include/jansson_config.h`
- `safe/jansson.map`
- `safe/jansson.pc.in`
- `safe/scripts/check-exports.sh`
- `safe/scripts/sync-upstream-tests.sh`
- `safe/scripts/build-upstream-api-tests.sh`
- `safe/scripts/check-allocator-hooks.sh`
- `safe/scripts/run-upstream-api-tests.sh`
- `safe/scripts/check-container-primitives.sh`
- `safe/scripts/run-data-suites.sh`
- `safe/src/lib.rs`
- `safe/src/abi.rs`
- `safe/src/error.rs`
- `safe/src/version.rs`
- `safe/src/raw/alloc.rs`
- `safe/src/raw/buf.rs`
- `safe/src/raw/list.rs`
- `safe/src/raw/table.rs`
- `safe/src/utf.rs`
- `safe/src/scalar.rs`
- `safe/src/array.rs`
- `safe/src/object.rs`
- `safe/src/strconv.rs`
- `safe/src/load.rs`
- `safe/src/dump.rs`
- `safe/src/pack.rs`
- `safe/src/unpack.rs`
- `safe/tests/upstream-api/test_array.c`
- `safe/tests/upstream-api/test_chaos.c`
- `safe/tests/upstream-api/test_copy.c`
- `safe/tests/upstream-api/test_dump.c`
- `safe/tests/upstream-api/test_dump_callback.c`
- `safe/tests/upstream-api/test_equal.c`
- `safe/tests/upstream-api/test_fixed_size.c`
- `safe/tests/upstream-api/test_load.c`
- `safe/tests/upstream-api/test_load_callback.c`
- `safe/tests/upstream-api/test_loadb.c`
- `safe/tests/upstream-api/test_memory_funcs.c`
- `safe/tests/upstream-api/test_number.c`
- `safe/tests/upstream-api/test_object.c`
- `safe/tests/upstream-api/test_pack.c`
- `safe/tests/upstream-api/test_simple.c`
- `safe/tests/upstream-api/test_sprintf.c`
- `safe/tests/upstream-api/test_unpack.c`
- `safe/tests/upstream-api/test_version.c`
- `safe/tests/upstream-api/util.h`
- `safe/tests/upstream-bin/json_process.c`
- `safe/tests/upstream-scripts/run-tests.sh`
- `safe/tests/upstream-scripts/valgrind.sh`
- `safe/tests/upstream-suites/valid/`
- `safe/tests/upstream-suites/invalid/`
- `safe/tests/upstream-suites/invalid-unicode/`
- `safe/tests/upstream-suites/encoding-flags/`
- `safe/README.md`
- `original/jansson-2.14/debian/control`
- `original/jansson-2.14/debian/rules`
- `original/jansson-2.14/debian/libjansson4.install`
- `original/jansson-2.14/debian/libjansson-dev.install`
- `original/jansson-2.14/debian/libjansson4.symbols`
- `original/jansson-2.14/jansson.pc.in`
- `original/jansson-2.14/src/jansson.h`
- `original/jansson-2.14/examples/simple_parse.c`
- `original/jansson-2.14/test/bin/json_process.c`
- `original/jansson-2.14/test/run-suites`
- `original/jansson-2.14/test/scripts/run-tests.sh`
- `original/jansson-2.14/test/scripts/valgrind.sh`
- `original/jansson-2.14/test/suites/api/`
- `original/jansson-2.14/test/suites/valid/`
- `original/jansson-2.14/test/suites/invalid/`
- `original/jansson-2.14/test/suites/invalid-unicode/`
- `original/jansson-2.14/test/suites/encoding-flags/`

## New Outputs
- installable Ubuntu 24.04 `.deb` artifacts for `libjansson4` and `libjansson-dev`
- symbol-version checking against the Debian ABI file
- checked-in Debian control metadata that preserves the upstream binary-package relationships needed for a drop-in Ubuntu replacement
- a full link-compatibility checker using upstream-compiled object files, including the suite driver and sample program objects, in explicit installed-package mode
- mirrored-test-aware API and data-suite runner scripts that now treat the existing `safe/tests/` corpus as authoritative and support both build-tree and installed-package execution modes
- initial compatibility documentation covering source, link, runtime, and packaging guarantees

## File Changes
- Create `safe/scripts/build-deb.sh`
- Create `safe/scripts/check-link-compat.sh`
- Update `safe/scripts/check-exports.sh`
- Update `safe/scripts/sync-upstream-tests.sh`
- Update `safe/scripts/build-upstream-api-tests.sh`
- Update `safe/scripts/run-upstream-api-tests.sh`
- Update `safe/scripts/run-data-suites.sh`
- Create `safe/pkg/DEBIAN/control.libjansson4`
- Create `safe/pkg/DEBIAN/control.libjansson-dev`
- Create `safe/pkg/install-manifest.libjansson4`
- Create `safe/pkg/install-manifest.libjansson-dev`
- Update `safe/jansson.pc.in`
- Update `safe/README.md`
- Create `safe/COMPATIBILITY.md`

## Implementation Details
- Build `.deb` packages whose names match the distro packages exactly: `libjansson4` and `libjansson-dev`.
- Keep the human-authored package metadata in `safe/pkg/DEBIAN/control.libjansson4` and `safe/pkg/DEBIAN/control.libjansson-dev` as checked-in templates derived from `original/jansson-2.14/debian/control`, and preserve the upstream binary-package relationships explicitly:
  - `safe/pkg/DEBIAN/control.libjansson4` must retain `Package: libjansson4`, `Section: libs`, `Architecture: any`, `Multi-Arch: same`, `Depends: ${shlibs:Depends}, ${misc:Depends}`, `Pre-Depends: ${misc:Pre-Depends}`, and the upstream runtime description text
  - `safe/pkg/DEBIAN/control.libjansson-dev` must retain `Package: libjansson-dev`, `Section: libdevel`, `Architecture: any`, `Multi-Arch: same`, `Depends: libjansson4 (= ${binary:Version}), ${misc:Depends}`, and the upstream development-package description text
- Use a Debian package version that sorts higher than Ubuntu's package version while leaving the API version untouched, for example `2.14-2build2+safe1`. The package version may change, but:
  - `JANSSON_VERSION`
  - `jansson_version_str()`
  - `pkg-config --modversion jansson`
  - the SONAME
  - the symbol-version node `libjansson.so.4`
  must remain upstream-compatible.
- `safe/scripts/build-deb.sh` must clean `safe/dist/` before each build so later verifier globs match exactly one runtime package and one development package.
- `safe/scripts/build-deb.sh` must materialize final staging `DEBIAN/control` files for the host architecture reported by `dpkg --print-architecture`, preserve `Multi-Arch: same`, resolve the exact built version into the `libjansson-dev` dependency on `libjansson4`, and use standard Debian dependency generation (`dpkg-shlibdeps`, `dpkg-gencontrol`, or equivalently precise generated fields) so the runtime package keeps the upstream dependency/predependency shape instead of hand-written ad-hoc dependencies.
- Keep `safe/pkg/install-manifest.libjansson4` and `safe/pkg/install-manifest.libjansson-dev` multiarch-aware. They may use the upstream `usr/lib/*/` glob style or an explicit placeholder such as `@DEB_HOST_MULTIARCH@`, but `safe/scripts/build-deb.sh` must resolve them from `dpkg-architecture -qDEB_HOST_MULTIARCH` when staging and no checked-in file or verifier may hard-code a literal multiarch triplet.
- Install to the canonical Ubuntu multiarch paths by resolving `multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"` once and using that value consistently in the package manifests, pkg-config file, helper scripts, and verifiers:
  - `/usr/lib/$multiarch/libjansson.so.4.14.0`
  - `/usr/lib/$multiarch/libjansson.so.4`
  - `/usr/lib/$multiarch/libjansson.so`
  - `/usr/lib/$multiarch/libjansson.a`
  - `/usr/include/jansson.h`
  - `/usr/include/jansson_config.h`
  - `/usr/lib/$multiarch/pkgconfig/jansson.pc`
- Preserve the appendix-defined ownership of the phase-6 packaging and harness files:
  - `safe/scripts/check-exports.sh` becomes the export-name and symbol-version checker against `original/jansson-2.14/src/jansson.def` and `original/jansson-2.14/debian/libjansson4.symbols`, with explicit build-tree and installed-root modes.
  - `safe/scripts/build-upstream-api-tests.sh`, `safe/scripts/run-upstream-api-tests.sh`, and `safe/scripts/run-data-suites.sh` must keep the mirrored `safe/tests/upstream-*` corpus authoritative in both default build-tree and `--installed-dev` modes.
  - `safe/scripts/check-link-compat.sh` owns original-header object compilation, linking against the installed safe library, and execution of the suite driver and sample program objects.
  - `safe/scripts/build-deb.sh` owns multiarch-aware staging, cleaned `safe/dist/`, resolved control metadata, and `.deb` emission.
  - `safe/pkg/DEBIAN/control.libjansson4` and `safe/pkg/DEBIAN/control.libjansson-dev` are checked-in template metadata preserving the upstream runtime/dev package relationships.
  - `safe/pkg/install-manifest.libjansson4` and `safe/pkg/install-manifest.libjansson-dev` are the multiarch-aware runtime/dev install manifests or manifest templates.
  - `safe/tests/upstream-api/*.c`, `safe/tests/upstream-api/util.h`, `safe/tests/upstream-bin/json_process.c`, `safe/tests/upstream-scripts/**`, and `safe/tests/upstream-suites/**` remain the shipped local mirror consumed by the runners rather than being regenerated ad hoc.
  - `safe/README.md` and `safe/COMPATIBILITY.md` must document build, install, and source/link/runtime compatibility verification and then continue to be updated in later phases.
- Extend `safe/scripts/check-exports.sh` so it checks both:
  - symbol names against `original/jansson-2.14/src/jansson.def`
  - symbol versions against `original/jansson-2.14/debian/libjansson4.symbols`
  and supports an explicit `--installed-root <path>` mode that inspects the installed shared object under `$installed_root/usr/lib/$(dpkg-architecture -qDEB_HOST_MULTIARCH)/` instead of defaulting to `safe/target/release`.
- Extend `safe/scripts/build-upstream-api-tests.sh`, `safe/scripts/run-upstream-api-tests.sh`, and `safe/scripts/run-data-suites.sh` with two explicit execution modes:
  - default build-tree mode keeps using `safe/include`, `safe/target/release`, and the mirrored `safe/tests/**` sources
  - `--installed-dev` mode must still consume the mirrored `safe/tests/**` sources, but it must obtain headers and libraries exclusively from the installed package through `pkg-config`, `/usr/include`, and `/usr/lib/$(dpkg-architecture -qDEB_HOST_MULTIARCH)` under the selected installed root, while clearing or ignoring `PKG_CONFIG_PATH`, `LD_LIBRARY_PATH`, `LIBRARY_PATH`, `CPATH`, and `C_INCLUDE_PATH`
- Implement `safe/scripts/check-link-compat.sh` so it:
  - stages the original `jansson.h` text plus the installed `jansson_config.h` from the selected installed root
  - compiles all 18 upstream API tests, `original/jansson-2.14/test/bin/json_process.c`, and `original/jansson-2.14/examples/simple_parse.c` to `.o` files against that original public header surface
  - links those `.o` files against the installed safe shared library under `/usr/lib/$(dpkg-architecture -qDEB_HOST_MULTIARCH)`
  - runs the resulting executables, fails if `ldd` resolves `libjansson.so.4` anywhere outside the installed package path, and uses the linked `json_process` object build to execute at least one representative valid case and one representative invalid case from the mirrored suite corpus
- Do not repopulate `safe/tests/` in this phase. Phase 1 already created the mirror. Verify it with `safe/scripts/sync-upstream-tests.sh --check` and keep every runner defaulting to the existing mirror.
- Make the mirrored corpus authoritative for bulk test execution:
  - `safe/scripts/build-upstream-api-tests.sh --all` and `safe/scripts/run-upstream-api-tests.sh --all` must cover all 18 API tests mirrored under `safe/tests/upstream-api/`
  - `safe/scripts/build-upstream-api-tests.sh --installed-dev --all` and `safe/scripts/run-upstream-api-tests.sh --installed-dev --all` must cover those same 18 mirrored API tests against the installed package
  - `safe/scripts/run-data-suites.sh valid invalid invalid-unicode encoding-flags` and `safe/scripts/run-data-suites.sh --installed-dev valid invalid invalid-unicode encoding-flags` must both resolve those suite names under `safe/tests/upstream-suites/`
- Keep `original/jansson-2.14/test/**` as the source of truth and use the sync script to refresh the shipped mirror when upstream artifacts intentionally change; do not hand-edit the mirrored tests independently.
- Create the first version of `safe/COMPATIBILITY.md` in this phase so later phases can update it instead of introducing a new compatibility document at the very end.
- Before yielding, commit all phase work to git with a message that begins with `impl_packaging_link_compat:`.

## Verification Phases
### `check_packaging_surface`
Phase ID: `check_packaging_surface`
Type: `check`
Fixed `bounce_target`: `impl_packaging_link_compat`
Purpose: Verify that the safe build produces correctly described Ubuntu 24.04 `libjansson4` / `libjansson-dev` packages, installs the correct pkg-config/header/dev surface including `libjansson.a`, and compiles mirrored upstream tests against the installed package rather than the build tree.

Commands:
```sh
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
safe/scripts/sync-upstream-tests.sh --check
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/check-exports.sh --installed-root / --check-versions
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/build-upstream-api-tests.sh --installed-dev test_version
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/run-upstream-api-tests.sh --installed-dev test_version
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/run-data-suites.sh --installed-dev valid invalid
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH sh -c 'test "$(pkg-config --modversion jansson)" = "2.14"'
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"; test -f "/usr/lib/$multiarch/libjansson.a"
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH sh -c 'multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"; cc $(pkg-config --cflags jansson) original/jansson-2.14/examples/simple_parse.c $(pkg-config --libs jansson) -o /tmp/jansson-simple-parse && ldd /tmp/jansson-simple-parse | grep "/usr/lib/$multiarch/libjansson.so.4"'
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"; env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH cc -I/usr/include original/jansson-2.14/examples/simple_parse.c "/usr/lib/$multiarch/libjansson.a" -o /tmp/jansson-simple-parse-static
```

### `check_link_compat`
Phase ID: `check_link_compat`
Type: `check`
Fixed `bounce_target`: `impl_packaging_link_compat`
Purpose: Verify that objects compiled against the original public header surface link against the installed safe library under `/usr` and still run.

Commands:
```sh
safe/scripts/build-deb.sh
dpkg -i safe/dist/libjansson4_*.deb safe/dist/libjansson-dev_*.deb
ldconfig
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/check-link-compat.sh --installed-root /
```

## Success Criteria
- The `.deb` packages build, their control metadata passes direct inspection, and they install correctly.
- Installed-package mode exercises `pkg-config`, the shared-link compile, the static-archive surface, and the mirrored API and data-suite runners against `/usr` rather than the build tree.
- Exported symbol versions match the Debian ABI file, link compatibility holds for objects compiled against the original public header surface, and the `safe/tests/` mirror remains sync-checked and authoritative.

## Git Commit Requirement
The implementer must commit work to git before yielding, with a message that begins with `impl_packaging_link_compat:`.
