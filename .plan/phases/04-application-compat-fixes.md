## Phase Name
Compatibility Fixes With Tester And Senior-Tester Review

## Implement Phase ID
`impl_application_compat_fixes`

## Preexisting Inputs
Consume these artifacts in place. If they already exist, update them instead of rediscovering or regenerating them.
- `safe/tests/regressions/discovered-issues.md`
- `safe/tests/regressions/manifest.json`
- `safe/scripts/run-regressions.sh`
- `safe/tests/regressions/cases/**`
- `safe/tests/regressions/fixtures/** when present`
- `safe/src/**`
- `safe/csrc/**`
- `safe/include/**`
- `safe/pkg/**`
- `safe/tests/upstream-*`
- `safe/docker/dependent-matrix.Dockerfile`
- `safe/scripts/build-dependent-image.sh`
- `safe/scripts/run-dependent-image-tests.sh`
- `safe/scripts/build-deb.sh`
- `safe/scripts/check-exports.sh`
- `safe/scripts/check-link-compat.sh`
- `safe/scripts/build-upstream-api-tests.sh`
- `safe/scripts/run-upstream-api-tests.sh`
- `safe/scripts/run-data-suites.sh`
- `safe/scripts/check-allocator-hooks.sh`
- `safe/scripts/check-container-primitives.sh`
- `relevant_cves.json`
- `all_cves.json`
- `safe/SAFETY.md`
- `original/jansson-2.14/debian/libjansson4.symbols`
- `original/jansson-2.14/src/jansson.h`

## New Outputs
- compatibility fixes for every reproduced issue
- updated regression metadata showing post-fix pass state
- `safe/tests/regressions/resolution-notes.md`
- updated safety documentation if the unsafe surface changes

## File Changes
- Create `safe/tests/regressions/resolution-notes.md`
- Modify one or more issue-locus files under:
  - `safe/src/load.rs`
  - `safe/src/dump.rs`
  - `safe/src/object.rs`
  - `safe/src/array.rs`
  - `safe/src/scalar.rs`
  - `safe/src/pack.rs`
  - `safe/src/unpack.rs`
  - `safe/src/utf.rs`
  - `safe/src/strconv.rs`
  - `safe/src/raw/table.rs`
  - `safe/src/raw/buf.rs`
  - `safe/src/raw/alloc.rs`
  - `safe/csrc/pack_unpack_shim.c`
  - `safe/csrc/sprintf_shim.c`
  - `safe/build.rs`
  - `safe/jansson.map`
  - `safe/scripts/check-exports.sh`
  - `safe/scripts/check-link-compat.sh`
  - `safe/scripts/build-upstream-api-tests.sh`
  - `safe/scripts/run-upstream-api-tests.sh`
  - `safe/scripts/run-data-suites.sh`
  - `safe/SAFETY.md`
  depending on the actual issue locus

## Implementation Details
- Fix only through reproduced, reviewed behavior deltas. Do not weaken the ABI, header, or symbol-version contract to make a dependent application pass.
- Keep `safe/include/jansson.h`, `safe/build.rs`, `safe/jansson.map`, and `original/jansson-2.14/debian/libjansson4.symbols` aligned conceptually:
  - symbol names unchanged
  - symbol versions unchanged
  - SONAME unchanged
- Any fix that alters unsafe code or introduces new unsafe blocks must update `safe/SAFETY.md`.
- Any fix that touches parser depth or hashing behavior must preserve the guarantees already encoded in `safe/src/load.rs` and `safe/src/object.rs`, and must keep the CVE-related tests passing.
- If Phase 2 and Phase 3 produced no failing issue IDs, this phase still has to run the full fix-review loop, update `safe/tests/regressions/resolution-notes.md` to state that no application incompatibilities required code changes, and commit the verified no-op or harness-only result.
- `safe/tests/regressions/resolution-notes.md` should map each issue ID to:
  - the root cause
  - files changed
  - regression case(s) that now cover it

## Verification Phases
### `check_application_compat_fixes`
Phase ID: `check_application_compat_fixes`
Type: `check`
Fixed `bounce_target`: `impl_application_compat_fixes`
Purpose: Verify that the compatibility fixes make the new regression suite pass and do not break the existing upstream build-tree matrix.

Commands:
```sh
cargo build --manifest-path safe/Cargo.toml --release
cargo test --manifest-path safe/Cargo.toml --release
cargo test --manifest-path safe/Cargo.toml --release parser_depth_limit_ --lib
cargo test --manifest-path safe/Cargo.toml --release container_seed_contract --lib
safe/scripts/run-regressions.sh
safe/scripts/check-allocator-hooks.sh
safe/scripts/check-container-primitives.sh
safe/scripts/build-upstream-api-tests.sh --all
safe/scripts/run-upstream-api-tests.sh --all
safe/scripts/run-data-suites.sh valid invalid invalid-unicode encoding-flags
```

### `check_application_compat_fixes_software_tester`
Phase ID: `check_application_compat_fixes_software_tester`
Type: `check`
Fixed `bounce_target`: `impl_application_compat_fixes`
Purpose: Software-tester review of the fixes using the prepared image and the checked-in regressions.

Commands:
```sh
safe/scripts/build-deb.sh
image_tag="libjansson-safe-matrix:phase4"; safe/scripts/build-dependent-image.sh --implementation safe --tag "$image_tag"
safe/scripts/run-dependent-image-tests.sh --image "$image_tag" --implementation safe --mode runtime
safe/scripts/run-regressions.sh
test -f safe/tests/regressions/discovered-issues.md
test -f safe/tests/regressions/manifest.json
```

### `check_application_compat_fixes_senior_tester`
Phase ID: `check_application_compat_fixes_senior_tester`
Type: `check`
Fixed `bounce_target`: `impl_application_compat_fixes`
Purpose: Senior-tester review of ABI/package compatibility and of any safety-impacting changes made while fixing application findings.

Commands:
```sh
safe/scripts/build-deb.sh
install_root="$PWD/safe/.build/installed-root-phase4"; rm -rf "$install_root"; mkdir -p "$install_root"; dpkg-deb -x safe/dist/libjansson4_*.deb "$install_root"; dpkg-deb -x safe/dist/libjansson-dev_*.deb "$install_root"
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/check-exports.sh --installed-root "$install_root" --check-versions
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/build-upstream-api-tests.sh --installed-dev --installed-root "$install_root" --all
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/run-upstream-api-tests.sh --installed-dev --installed-root "$install_root" --all
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/run-data-suites.sh --installed-dev --installed-root "$install_root" valid invalid invalid-unicode encoding-flags
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/check-link-compat.sh --installed-root "$install_root"
rg -n '\bunsafe\b|extern "C"|no_mangle' safe/src safe/csrc
```

## Success Criteria
- Run the full new regression suite in pass mode.
- Re-run the upstream build-tree API/data checks.
- Re-run the 12-application runtime matrix inside the prepared image.
- Re-run installed-root ABI/source/link checks.
- Pass both software-tester and senior-tester review phases.

## Git Commit Requirement
The implementer must commit work to git before yielding. The commit message must include `impl_application_compat_fixes`.
