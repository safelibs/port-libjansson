## Phase Name
Dockerized 12-Application Build/Runtime Matrix And Issue Capture

## Implement Phase ID
`impl_dependent_matrix_baseline`

## Preexisting Inputs
Consume these artifacts in place. If they already exist, update them instead of rediscovering or regenerating them.
- `safe/docker/dependent-matrix.Dockerfile`
- `safe/scripts/build-dependent-image.sh`
- `dependents.json`
- `safe/scripts/check-dependent-builds.sh`
- `safe/scripts/full-verify.sh`
- `test-original.sh`
- `test-safe.sh`
- `safe/README.md`
- `safe/COMPATIBILITY.md`
- `safe/dist/*.deb`

## New Outputs
- `safe/scripts/in-container-dependent-tests.sh`
- `safe/scripts/run-dependent-image-tests.sh`
- runtime smoke coverage for all 12 manifest entries, with `nghttp2-server` used only as a helper to exercise the `nghttp2-client` manifest entry
- persistent issue inventory under `safe/tests/regressions/discovered-issues.md`
- transient logs under `safe/.build/dependent-matrix/`

## File Changes
- Create `safe/scripts/in-container-dependent-tests.sh`
- Create `safe/scripts/run-dependent-image-tests.sh`
- Modify `test-original.sh`
- Modify `test-safe.sh`
- Modify `safe/scripts/check-dependent-builds.sh`
- Modify `safe/README.md`
- Modify `safe/COMPATIBILITY.md`
- Create `safe/tests/regressions/discovered-issues.md`

## Implementation Details
- Extract the current in-container body from `test-original.sh` into a reusable script under `safe/scripts/` so both the legacy wrapper and the new prepared-image runner use one authoritative implementation.
- Make `safe/scripts/run-dependent-image-tests.sh` the authoritative host-side runner for the prepared-image path. `test-original.sh` and `test-safe.sh` may remain as compatibility shims, but they must call the extracted shared harness rather than keeping a second copy of the smoke-test logic.
- Keep the existing environment contract:
  - `JANSSON_IMPLEMENTATION=original|safe`
  - `JANSSON_TEST_MODE=build|runtime|all`
  - `DOCKER_IMAGE=...`
- Add `nghttp2` coverage:
  - install `nghttp2-client` as the counted manifest entry and `nghttp2-server` only as the local test fixture needed to drive it
  - add a runtime smoke test that starts `nghttpd`, requests a resource with `nghttp --har=...`, and verifies that the emitted HAR file parses as JSON and contains the expected top-level `log` structure
  - assert that the exercised `nghttp` binary resolves the selected `libjansson.so.4`
- Keep the existing 11 application smoke tests and resolution checks intact. Do not weaken or replace them with simpler probes.
- Persist raw logs under a deterministic per-run tree such as `safe/.build/dependent-matrix/<implementation>/<mode>/<application>/...`, and make every issue entry point at one of those paths.
- After every build/runtime matrix run, write or update `safe/tests/regressions/discovered-issues.md`:
  - one stable issue ID per incompatibility, for example `APP-NGHTTP2-HAR-001`
  - failing command
  - expected behavior
  - observed behavior
  - suspected subsystem (`load`, `dump`, `pack`, `unpack`, `object`, packaging, linker, etc.)
  - log path under `safe/.build/dependent-matrix/`
  - If no failures are found, record that explicitly instead of leaving the file absent.
- Later phases must consume and update `safe/tests/regressions/discovered-issues.md` in place. They must not discard prior issue IDs by regenerating the file from scratch.

## Verification Phases
### `check_dependent_matrix_build`
Phase ID: `check_dependent_matrix_build`
Type: `check`
Fixed `bounce_target`: `impl_dependent_matrix_baseline`
Purpose: Verify that the prepared image can rebuild all 12 dependent source packages against the safe development package.

Commands:
```sh
safe/scripts/build-deb.sh
image_tag="libjansson-safe-matrix:phase2"; safe/scripts/build-dependent-image.sh --implementation safe --tag "$image_tag"
safe/scripts/run-dependent-image-tests.sh --image "$image_tag" --implementation safe --mode build
```

### `check_dependent_matrix_runtime`
Phase ID: `check_dependent_matrix_runtime`
Type: `check`
Fixed `bounce_target`: `impl_dependent_matrix_baseline`
Purpose: Verify that the prepared image can run the full 12-application runtime smoke matrix and persist discovered incompatibilities into a checked-in issue artifact.

Commands:
```sh
safe/scripts/build-deb.sh
image_tag="libjansson-safe-matrix:phase2"; safe/scripts/build-dependent-image.sh --implementation safe --tag "$image_tag"
safe/scripts/run-dependent-image-tests.sh --image "$image_tag" --implementation safe --mode runtime
test -f safe/tests/regressions/discovered-issues.md
grep -Eq 'No application-level regressions found|^## APP-' safe/tests/regressions/discovered-issues.md
```

## Success Criteria
- Rebuild all 12 source packages against safe `libjansson-dev`.
- Run all 12 runtime smoke tests against the safe package inside the prepared image.
- Confirm that the issue inventory file exists and states whether the run is failing or clean.

## Git Commit Requirement
The implementer must commit work to git before yielding. The commit message must include `impl_dependent_matrix_baseline`.
