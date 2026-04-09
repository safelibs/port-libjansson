## Phase Name
Downstream Dependent Compile Harness And Runtime Replacement Harness

## Implement Phase ID
`impl_downstream_compat`

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
- `safe/README.md`
- `safe/COMPATIBILITY.md`
- `dependents.json`
- `test-original.sh`

## New Outputs
- downstream source-package build harness driven by `dependents.json`
- parameterized runtime/compile replacement harness
- optional wrapper script for safe-mode execution

## File Changes
- Create `safe/scripts/check-dependent-builds.sh`
- Update `test-original.sh`
- Create `test-safe.sh` if useful as a thin wrapper
- Update `safe/README.md`
- Update `safe/COMPATIBILITY.md`

## Implementation Details
- Extend `test-original.sh` so it has explicit modes instead of a single baseline runtime flow:
  - `JANSSON_IMPLEMENTATION=original|safe`
  - `JANSSON_TEST_MODE=build|runtime|all`
- Preserve the existing original-baseline behavior for `JANSSON_IMPLEMENTATION=original`; the harness should still be able to validate the original library in the same container workflow.
- In safe mode, install the locally built `.deb` packages with `dpkg -i` instead of shadowing with `/usr/local` + `LD_LIBRARY_PATH`. The safe package needs to be tested as an actual system replacement.
- In `JANSSON_TEST_MODE=build`, skip the runtime smoke-test package exercise and instead install the source-build prerequisites plus the safe `libjansson4` / `libjansson-dev` packages before invoking `safe/scripts/check-dependent-builds.sh`.
- In `JANSSON_TEST_MODE=runtime`, preserve the current smoke tests and only change the library-install step selected by `JANSSON_IMPLEMENTATION`.
- Implement `safe/scripts/check-dependent-builds.sh` as the authoritative compile-compatibility harness for the `dependents.json` manifest. It should:
  - parse the unique `source_package` values from `jq -r '.dependents[].source_package' dependents.json | sort -u`
  - enable source repositories inside the Ubuntu 24.04 container if needed
  - install the safe `libjansson4` and `libjansson-dev` packages first
  - hold or pin those local packages so `apt-get build-dep` does not replace them with the distro version
  - fetch each dependent source package with `apt-get source "$srcpkg"`
  - install build-dependencies
  - run `DEB_BUILD_OPTIONS=nocheck dpkg-buildpackage -B -uc -us` for each source package
  - fail immediately if any build reinstalls the distro Jansson package or if any package build fails
- The compile harness must cover all unique source packages in the manifest, not a subset and not a synthetic probe:
  - `emacs`
  - `janus`
  - `jshon`
  - `jose`
  - `mtr`
  - `suricata`
  - `tang`
  - `libteam`
  - `ulogd2`
  - `wayvnc`
  - `webdis`
- Preserve the appendix-defined ownership of the phase-7 downstream harness files:
  - `safe/scripts/check-dependent-builds.sh` is the authoritative rebuild harness for every unique `source_package` named in `dependents.json`.
  - `test-original.sh` remains the authoritative original-vs-safe and build-vs-runtime harness entrypoint and must be extended in place rather than replaced.
  - `test-safe.sh`, if added, is only a convenience wrapper around the same parameterized harness.
  - `safe/README.md` and `safe/COMPATIBILITY.md` should record how downstream compile and runtime compatibility are exercised.
- Keep the existing runtime smoke tests in `test-original.sh`, but move the install step behind the new `JANSSON_IMPLEMENTATION` switch so the same script can validate both the original build and the safe replacement.
- While updating `test-original.sh`, remove architecture-specific library/plugin paths from the existing smoke tests. In particular, the current hard-coded `ulogd_output_JSON.so` probe should resolve the active multiarch triplet with `dpkg-architecture -qDEB_HOST_MULTIARCH` or query the package file list before asserting linkage.
- Before yielding, commit all phase work to git with a message that begins with `impl_downstream_compat:`.

## Verification Phases
### `check_dependent_compile_compat`
Phase ID: `check_dependent_compile_compat`
Type: `check`
Fixed `bounce_target`: `impl_downstream_compat`
Purpose: Verify that every dependent source package named in `dependents.json` continues to build against the installed safe dev package.

Commands:
```sh
safe/scripts/build-deb.sh
JANSSON_IMPLEMENTATION=safe JANSSON_TEST_MODE=build ./test-original.sh
```

### `check_dependent_runtime_compat`
Phase ID: `check_dependent_runtime_compat`
Type: `check`
Fixed `bounce_target`: `impl_downstream_compat`
Purpose: Verify that the runtime-dependent binaries in `dependents.json` continue to function when the safe package replaces upstream Jansson.

Commands:
```sh
safe/scripts/build-deb.sh
JANSSON_IMPLEMENTATION=safe JANSSON_TEST_MODE=runtime ./test-original.sh
```

## Success Criteria
- The build-mode harness confirms that all 11 unique dependent source packages compile against the safe dev package.
- The runtime-mode harness confirms that the existing dependent binary smoke tests still pass under the safe package.
- `test-original.sh` supports `JANSSON_IMPLEMENTATION=original|safe` and `JANSSON_TEST_MODE=build|runtime|all` without regressing the original-baseline flow.

## Git Commit Requirement
The implementer must commit work to git before yielding, with a message that begins with `impl_downstream_compat:`.
