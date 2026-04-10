## Phase Name
Final Hardening And Authoritative Release Verification

## Implement Phase ID
`impl_final_hardening`

## Preexisting Inputs
Consume these artifacts in place. If they already exist, update them instead of rediscovering or regenerating them.
- `safe/tests/regressions/resolution-notes.md`
- `safe/tests/regressions/discovered-issues.md`
- `safe/tests/regressions/manifest.json`
- `safe/tests/regressions/cases/**`
- `safe/tests/regressions/fixtures/** when present`
- `safe/tests/upstream-*`
- `safe/scripts/full-verify.sh`
- `safe/README.md`
- `safe/COMPATIBILITY.md`
- `safe/SAFETY.md`
- `safe/docker/dependent-matrix.Dockerfile`
- `safe/scripts/build-dependent-image.sh`
- `safe/scripts/run-dependent-image-tests.sh`
- `safe/scripts/run-regressions.sh`
- `safe/scripts/build-deb.sh`
- `safe/scripts/sync-upstream-tests.sh`
- `safe/scripts/check-exports.sh`
- `safe/scripts/check-link-compat.sh`
- `safe/scripts/build-upstream-api-tests.sh`
- `safe/scripts/run-upstream-api-tests.sh`
- `safe/scripts/run-data-suites.sh`
- `safe/scripts/check-allocator-hooks.sh`
- `safe/scripts/check-container-primitives.sh`
- `test-safe.sh`
- `dependents.json`
- `safe/src/**`
- `safe/csrc/**`
- `safe/include/**`
- `safe/pkg/**`
- `relevant_cves.json`
- `all_cves.json`
- `original/jansson-2.14/debian/libjansson4.symbols`
- `original/jansson-2.14/src/jansson.h`
- `original/jansson-2.14/examples/simple_parse.c`

## New Outputs
- final authoritative verification script and docs
- final end-to-end clean verification result

## File Changes
- Modify `safe/scripts/full-verify.sh`
- Modify `safe/README.md`
- Modify `safe/COMPATIBILITY.md`
- Modify `safe/SAFETY.md`
- Modify `test-safe.sh`
- Modify any helper scripts whose documented invocation changed in prior phases

## Implementation Details
- Make `safe/scripts/full-verify.sh` the authoritative single-entry final matrix and keep it consistent with `safe/COMPATIBILITY.md`.
- Ensure docs and scripts all describe the same prepared-image workflow, the same 12 dependent applications, the same installed-root verification steps, and the same regression runner.
- Remove any stale documentation that still implies only 11 dependents or only ad-hoc `docker run ubuntu:24.04`.
- The authoritative final script should verify an extracted installed root under `safe/.build/installed-root-final` rather than relying on a mutable host `/` install. If legacy local workflows still support `dpkg -i` into `/`, that remains a convenience path rather than the release gate.
- Keep all installed-library and `pkg-config` paths expressed in terms of `dpkg-architecture -qDEB_HOST_MULTIARCH`, never hard-coded to `x86_64-linux-gnu`.
- The final safety audit is not satisfied by the `rg -n '\bunsafe\b|extern "C"|no_mangle' safe/src safe/csrc` inventory alone. Phase completion requires confirming that every residual unsafe area reported by that audit is still documented and justified in `safe/SAFETY.md`.

## Verification Phases
### `check_final_hardening`
Phase ID: `check_final_hardening`
Type: `check`
Fixed `bounce_target`: `impl_final_hardening`
Purpose: Verify that the repository has one consistent, end-to-end verification path covering source, link, runtime, packaging, dependent builds, dependent runtime, regressions, and safety-sensitive compatibility guarantees.

Commands:
```sh
cargo build --manifest-path safe/Cargo.toml --release
cargo test --manifest-path safe/Cargo.toml --release
cargo test --manifest-path safe/Cargo.toml --release parser_depth_limit_ --lib
cargo test --manifest-path safe/Cargo.toml --release container_seed_contract --lib
safe/scripts/sync-upstream-tests.sh --check
safe/scripts/check-allocator-hooks.sh
safe/scripts/check-container-primitives.sh
safe/scripts/check-exports.sh --check-versions
safe/scripts/build-upstream-api-tests.sh --all
safe/scripts/run-upstream-api-tests.sh --all
safe/scripts/run-data-suites.sh valid invalid invalid-unicode encoding-flags
safe/scripts/run-regressions.sh
safe/scripts/build-deb.sh
test "$(dpkg-deb -f safe/dist/libjansson4_*.deb Package)" = "libjansson4"
test "$(dpkg-deb -f safe/dist/libjansson-dev_*.deb Package)" = "libjansson-dev"
test "$(dpkg-deb -f safe/dist/libjansson4_*.deb Architecture)" = "$(dpkg --print-architecture)"
test "$(dpkg-deb -f safe/dist/libjansson-dev_*.deb Architecture)" = "$(dpkg --print-architecture)"
test "$(dpkg-deb -f safe/dist/libjansson4_*.deb Multi-Arch)" = "same"
test "$(dpkg-deb -f safe/dist/libjansson-dev_*.deb Multi-Arch)" = "same"
install_root="$PWD/safe/.build/installed-root-final"; rm -rf "$install_root"; mkdir -p "$install_root"; dpkg-deb -x safe/dist/libjansson4_*.deb "$install_root"; dpkg-deb -x safe/dist/libjansson-dev_*.deb "$install_root"
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/check-exports.sh --installed-root "$install_root" --check-versions
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/build-upstream-api-tests.sh --installed-dev --installed-root "$install_root" --all
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/run-upstream-api-tests.sh --installed-dev --installed-root "$install_root" --all
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/run-data-suites.sh --installed-dev --installed-root "$install_root" valid invalid invalid-unicode encoding-flags
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"; pkgcfg="$install_root/usr/lib/$multiarch/pkgconfig:$install_root/usr/lib/pkgconfig:$install_root/usr/share/pkgconfig"; env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH PKG_CONFIG_DIR= PKG_CONFIG_LIBDIR="$pkgcfg" PKG_CONFIG_SYSROOT_DIR="$install_root" pkg-config --modversion jansson | grep -Fx '2.14'
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"; test -f "$install_root/usr/lib/$multiarch/libjansson.a"
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"; pkgcfg="$install_root/usr/lib/$multiarch/pkgconfig:$install_root/usr/lib/pkgconfig:$install_root/usr/share/pkgconfig"; env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH PKG_CONFIG_DIR= PKG_CONFIG_LIBDIR="$pkgcfg" PKG_CONFIG_SYSROOT_DIR="$install_root" sh -c 'install_root="$1"; multiarch="$2"; cc $(pkg-config --cflags jansson) original/jansson-2.14/examples/simple_parse.c $(pkg-config --libs jansson) -Wl,-rpath,"$install_root/usr/lib/$multiarch" -o /tmp/jansson-simple-parse-dynamic-final' sh "$install_root" "$multiarch"
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"; env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH cc -I"$install_root/usr/include" original/jansson-2.14/examples/simple_parse.c "$install_root/usr/lib/$multiarch/libjansson.a" -o /tmp/jansson-simple-parse-static-final
env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH safe/scripts/check-link-compat.sh --installed-root "$install_root"
image_tag="libjansson-safe-matrix:final"; safe/scripts/build-dependent-image.sh --implementation safe --tag "$image_tag"
safe/scripts/run-dependent-image-tests.sh --image "$image_tag" --implementation safe --mode build
safe/scripts/run-dependent-image-tests.sh --image "$image_tag" --implementation safe --mode runtime
rg -n '\bunsafe\b|extern "C"|no_mangle' safe/src safe/csrc
```

Additional required verification: after the final `rg` audit runs, confirm that every residual unsafe area it reports is still covered by `safe/SAFETY.md`. The phase fails if any residual unsafe area lacks `safe/SAFETY.md` coverage.

## Success Criteria
- Run the full authoritative matrix without relying on build-tree fallbacks.
- Confirm documentation matches the actual final script entrypoints and commands.
- Confirm the final safety audit leaves no residual unsafe area undocumented in `safe/SAFETY.md`.
- Cover source, packaging, installed-root compatibility, downstream build/runtime compatibility, regressions, and the residual unsafe surface in one consistent release gate.

## Git Commit Requirement
The implementer must commit work to git before yielding. The commit message must include `impl_final_hardening`.
