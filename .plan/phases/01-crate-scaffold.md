## Phase Name
Crate Bootstrap, ABI Scaffold, And Mirrored Test Corpus

## Implement Phase ID
`impl_crate_scaffold`

## Preexisting Inputs
Consume these artifacts in place. Do not rediscover, refetch, or regenerate them.
- `.plan/goal.md`
- The fact that `safe/` does not yet exist and must be created from scratch
- `original/jansson-2.14/CMakeLists.txt`
- `original/jansson-2.14/debian/libjansson-dev.install`
- `original/jansson-2.14/jansson.pc.in`
- `original/jansson-2.14/src/Makefile.am`
- `original/jansson-2.14/src/jansson.def`
- `original/jansson-2.14/src/jansson.h`
- `original/jansson-2.14/src/jansson_config.h.in`
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
- `safe/` Cargo package skeleton
- installed-header and pkg-config surface under `safe/include/` and `safe/`
- shared-library SONAME control and static-library output matching the Ubuntu dev package contract
- linker version/export control derived from upstream artifacts
- residual C shim scaffolding for the stable-Rust variadic exports
- initial helper scripts for exports and test syncing
- full mirrored `safe/tests/` corpus for the upstream API tests, `json_process`, suite data, and upstream test helper scripts

## File Changes
- Create `safe/Cargo.toml`
- Create `safe/build.rs`
- Create `safe/csrc/pack_unpack_shim.c`
- Create `safe/csrc/sprintf_shim.c`
- Create `safe/src/lib.rs`
- Create `safe/include/jansson.h`
- Create `safe/include/jansson_config.h`
- Create `safe/jansson.map`
- Create `safe/jansson.pc.in`
- Create `safe/scripts/check-exports.sh`
- Create `safe/scripts/sync-upstream-tests.sh`
- Create `safe/tests/upstream-api/`
- Create `safe/tests/upstream-bin/`
- Create `safe/tests/upstream-scripts/`
- Create `safe/tests/upstream-suites/`
- Create `safe/README.md`

## Implementation Details
- Treat the absence of `safe/` as part of the input contract for this phase: phase 1 owns creating the directory tree, package scaffold, mirrored tests, and export controls from scratch rather than assuming a precreated Rust package already exists.
- Create `safe/` as a normal Cargo package with `[lib] name = "jansson"` and `crate-type = ["cdylib", "staticlib", "rlib"]` so the project emits the required artifact names `libjansson.so` and `libjansson.a`.
- Derive the Linux SONAME from the upstream ABI contract, not from Cargo defaults. `original/jansson-2.14/src/Makefile.am:25-30` and `original/jansson-2.14/CMakeLists.txt:34-40` imply `libjansson.so.4` with runtime version `4.14.0`.
- Generate a GNU ld version script from the existing `original/jansson-2.14/src/jansson.def` file instead of hand-maintaining a second symbol list. Phase 1 may emit placeholder bodies, but all 81 public symbol names must already exist.
- Create and compile the residual C shim files in this phase, not later. They must export the stable-Rust-inexpressible ABI entry points from the start:
  - `json_pack`
  - `json_pack_ex`
  - `json_vpack_ex`
  - `json_unpack`
  - `json_unpack_ex`
  - `json_vunpack_ex`
  - `json_sprintf`
  - `json_vsprintf`
  Until phase 5, those shim bodies may return placeholder failure results compatible with their signatures (`NULL` for pointer-returning APIs and `-1` for integer-returning APIs), but the exported symbols, calling convention, and linkage must already be correct in phase 1.
- Copy the upstream public header into `safe/include/jansson.h` without semantic changes. In particular preserve:
  - `json_t`, `json_error_t`, and the public enums
  - the refcounting macros and inline `json_incref()` / `json_decref()`
  - the object-iterator macros
  - all public function declarations
- Materialize `safe/include/jansson_config.h` from the existing template by running the upstream configure logic inside an Ubuntu 24.04 environment for the current Debian host architecture, using the same distro/toolchain assumptions that the packaging and harness phases will later use. Do not freeze a single-architecture result if the resolved public macros differ by host architecture.
- Add crate-wide safety constraints early, such as `#![deny(unsafe_op_in_unsafe_fn)]`, because later phases will need tightly audited `unsafe`.
- Preserve the appendix-defined ownership of the phase-1 scaffold files:
  - `safe/Cargo.toml` owns the Cargo package metadata, `[lib] name = "jansson"`, `cdylib` plus `staticlib` outputs, release profile, and build dependencies for the residual C shims.
  - `safe/build.rs` owns linker flags, SONAME/version-script emission, C shim compilation, and generated export/map handling.
  - `safe/include/jansson.h` and `safe/include/jansson_config.h` are the installed public header/config surface and must match the intended ABI without semantic drift.
  - `safe/jansson.map` is the GNU ld version script that exposes only the upstream symbol surface under the `libjansson.so.4` version node.
  - `safe/jansson.pc.in` owns installed pkg-config metadata matching upstream fields and canonical include/lib paths.
  - `safe/src/lib.rs` is the public `extern "C"` export wiring and module-registration point, even while most function bodies are still placeholders.
  - `safe/csrc/pack_unpack_shim.c` and `safe/csrc/sprintf_shim.c` are the residual C ABI shims for the stable-Rust-inexpressible variadic and `va_list` ingress.
  - `safe/scripts/check-exports.sh` is the owned export-name checker, and `safe/scripts/sync-upstream-tests.sh` is the owned mirror copy/check script for the local `safe/tests/upstream-*` corpus.
  - `safe/tests/upstream-api/*.c`, `safe/tests/upstream-api/util.h`, `safe/tests/upstream-bin/json_process.c`, `safe/tests/upstream-scripts/**`, and `safe/tests/upstream-suites/**` are shipped local mirrors of the upstream test corpus and must be created here rather than referenced only from `original/`.
  - `safe/README.md` should begin documenting the package build, test, and install entry points in this phase so later phases can extend it in place.
- Add `safe/scripts/sync-upstream-tests.sh` now so later phases can verify the required `safe/tests/` mirror without rediscovering the upstream corpus. The script must copy:
  - `original/jansson-2.14/test/suites/api/**` into `safe/tests/upstream-api/`
  - `original/jansson-2.14/test/bin/**` into `safe/tests/upstream-bin/`
  - `original/jansson-2.14/test/scripts/**` into `safe/tests/upstream-scripts/`
  - `original/jansson-2.14/test/suites/{valid,invalid,invalid-unicode,encoding-flags}/**` into `safe/tests/upstream-suites/`
  preserve executable bits on the mirrored `run` scripts, and support:
  - `--sync` to refresh the mirror
  - `--check` to fail if the mirror is missing or stale
- Run `safe/scripts/sync-upstream-tests.sh --sync` in this phase so the mirrored API tests, `json_process.c`, and suite data already exist before any later phase starts consuming them.

## Verification Phases
### `check_crate_scaffold`
Phase ID: `check_crate_scaffold`
Type: `check`
Fixed `bounce_target`: `impl_crate_scaffold`
Purpose: Verify that `safe/` exists as a standard Rust package, emits the required shared and static libraries, exposes the full symbol-name surface, and carries a complete mirror of the upstream tests.

Commands:
```sh
cargo build --manifest-path safe/Cargo.toml --release
test -f safe/target/release/libjansson.a
safe/scripts/check-exports.sh --names-only
readelf -d safe/target/release/libjansson.so | grep 'SONAME.*libjansson.so.4'
cc -I safe/include original/jansson-2.14/examples/simple_parse.c -L safe/target/release -Wl,-rpath,$PWD/safe/target/release -ljansson -o /tmp/jansson-simple-parse
safe/scripts/sync-upstream-tests.sh --check
```

## Success Criteria
- The shared and static libraries build successfully.
- The symbol-name surface matches `jansson.def`.
- The shared object reports SONAME `libjansson.so.4`.
- The public header is usable by compiling `original/jansson-2.14/examples/simple_parse.c`.
- The mirrored upstream test corpus exists and is script-checked.

## Git Commit Requirement
The implementer must commit work to git before yielding, with a message that begins with `impl_crate_scaffold:`.
