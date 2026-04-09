## Phase Name
Final Hardening, Unsafe Audit, And Full Regression Sweep

## Implement Phase ID
`impl_final_hardening`

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
- `safe/pkg/DEBIAN/control.libjansson4`
- `safe/pkg/DEBIAN/control.libjansson-dev`
- `safe/pkg/install-manifest.libjansson4`
- `safe/pkg/install-manifest.libjansson-dev`
- `safe/scripts/check-exports.sh`
- `safe/scripts/sync-upstream-tests.sh`
- `safe/scripts/build-upstream-api-tests.sh`
- `safe/scripts/check-allocator-hooks.sh`
- `safe/scripts/run-upstream-api-tests.sh`
- `safe/scripts/check-container-primitives.sh`
- `safe/scripts/run-data-suites.sh`
- `safe/scripts/build-deb.sh`
- `safe/scripts/check-link-compat.sh`
- `safe/scripts/check-dependent-builds.sh`
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
- `safe/COMPATIBILITY.md`
- `relevant_cves.json`
- `dependents.json`
- `test-original.sh`
- `original/jansson-2.14/examples/simple_parse.c`
- `original/jansson-2.14/src/jansson.h`
- `original/jansson-2.14/src/jansson.def`
- `original/jansson-2.14/src/strbuffer.c`
- `original/jansson-2.14/debian/libjansson4.symbols`
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
- final compatibility and safety fixes
- explicit documentation of residual `unsafe` and compatibility guarantees
- optional one-shot full verification wrapper

## File Changes
- Update any previously created Rust, C shim, script, package, or mirrored test files as needed
- Create `safe/SAFETY.md`
- Update `safe/COMPATIBILITY.md`
- Optionally create `safe/scripts/full-verify.sh`

## Implementation Details
- Fix all residual mismatches surfaced by the combined API, data-suite, link-compatibility, package, dependent-build, and runtime harnesses.
- Audit every remaining `unsafe` block and every residual C file. The final state should leave non-safe code only where it is genuinely required for:
  - the public C ABI
  - public-layout pointer casting
  - allocator-hook integration
  - raw `FILE *` / fd interaction
  - the stable-Rust variadic boundary
- Preserve the appendix-defined ownership of the final-state documentation and wrapper files:
  - `safe/SAFETY.md` must inventory and justify every remaining `unsafe` block and residual C boundary.
  - `safe/COMPATIBILITY.md` must end as the authoritative explanation of source, link, runtime, and packaging guarantees plus how each guarantee is verified.
  - `safe/scripts/full-verify.sh`, if created, is only a convenience wrapper around the same full matrix already spelled out in this phase and must not introduce a divergent verification contract.
- Reconfirm the two explicit CVE requirements:
  - randomized seeded object hashing by default with deterministic override only before first use
  - bounded/iterative parsing that cannot exhaust the process stack on attacker-supplied nesting
- Confirm the mirrored `safe/tests/` corpus remains synchronized with `original/jansson-2.14/test/**`.
- Treat this phase's checker as the authoritative final matrix. It must rerun the built-package metadata checks plus the installed-package export, mirrored-API, mirrored-data-suite, header/pkg-config/static-archive, link-compatibility, dependent-build, and dependent-runtime checks after `dpkg -i`; do not rely on earlier phase results without rerunning them.
- In `check_final_hardening`, commands 4-6 and 20-22 must consume the mirrored `safe/tests/upstream-api/**`, `safe/tests/upstream-bin/**`, `safe/tests/upstream-scripts/**`, and `safe/tests/upstream-suites/**` corpus by default. They may only touch `original/jansson-2.14/test/**` indirectly through the already-created sync mechanism, not as an execution-time fallback.
- In `check_final_hardening`, commands 19-27 are installed-package validation only. After `dpkg -i`, they must resolve headers, pkg-config metadata, shared objects, and static archives from `/usr` (or another explicitly passed installed root) and must not silently fall back to `safe/target/release`, `safe/include`, or other build-tree artifacts.
- Do a final performance sanity pass on the custom buffers, tables, and parser stack so compatibility fixes have not introduced obviously pathological behavior.
- Before yielding, commit all phase work to git with a message that begins with `impl_final_hardening:`.

## Verification Phases
### `check_final_hardening`
Phase ID: `check_final_hardening`
Type: `check`
Fixed `bounce_target`: `impl_final_hardening`
Purpose: Re-run the complete compatibility matrix, close any remaining gaps, and confirm that the remaining `unsafe` / residual C surface is justified and minimal.

Commands:
```sh
cargo build --manifest-path safe/Cargo.toml --release
test -f safe/target/release/libjansson.a
safe/scripts/sync-upstream-tests.sh --check
# Commands 4-6 must use the mirrored safe/tests/upstream-* corpus by default.
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
# Commands 19-27 must validate the installed package under /usr and must not fall back to build-tree headers or libraries.
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/check-exports.sh --installed-root / --check-versions
# Commands 20-22 must still use the mirrored safe/tests/upstream-* corpus, now against the installed package under /usr.
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
```

## Success Criteria
- The complete verification matrix runs without skipping any compatibility class.
- The remaining `unsafe` and residual C inventory is recorded and justified in `safe/SAFETY.md`.
- Seeded hashing, bounded parsing, and mirrored test synchronization remain compliant with `relevant_cves.json` and `original/jansson-2.14/test/**`.

## Git Commit Requirement
The implementer must commit work to git before yielding, with a message that begins with `impl_final_hardening:`.
