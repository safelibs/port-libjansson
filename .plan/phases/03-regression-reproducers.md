## Phase Name
Repo-Native Regression Reproducers For Application Findings

## Implement Phase ID
`impl_regression_reproducers`

## Preexisting Inputs
Consume these artifacts in place. If they already exist, update them instead of rediscovering or regenerating them.
- `safe/scripts/in-container-dependent-tests.sh`
- `safe/scripts/run-dependent-image-tests.sh`
- `safe/tests/regressions/discovered-issues.md`
- `safe/.build/dependent-matrix/`
- `safe/src/**`
- `safe/csrc/**`
- `safe/tests/upstream-*`
- `safe/scripts/sync-upstream-tests.sh`

## New Outputs
- `safe/tests/regressions/manifest.json`
- `safe/scripts/run-regressions.sh`
- minimized regression cases under `safe/tests/regressions/`
- additional targeted unit tests in issue-specific Rust modules when a C or shell reproducer would be unreasonably indirect

## File Changes
- Create `safe/tests/regressions/manifest.json`
- Create `safe/scripts/run-regressions.sh`
- Create `safe/tests/regressions/cases/**`
- Create `safe/tests/regressions/fixtures/**` as needed
- Modify one or more of:
  - `safe/src/load.rs`
  - `safe/src/dump.rs`
  - `safe/src/object.rs`
  - `safe/src/array.rs`
  - `safe/src/scalar.rs`
  - `safe/src/pack.rs`
  - `safe/src/unpack.rs`
  - `safe/src/utf.rs`
  - `safe/src/strconv.rs`
  - `safe/csrc/pack_unpack_shim.c`
  - `safe/csrc/sprintf_shim.c`
  when module-local tests are the smallest faithful reproducer

## Implementation Details
- Create one checked-in reproducer for every issue ID in `safe/tests/regressions/discovered-issues.md`.
- Keep the mirrored upstream tests under `safe/tests/upstream-*` as the default compatibility corpus while adding repo-native regressions. If that mirrored corpus ever needs refresh, update it only in place via `safe/scripts/sync-upstream-tests.sh`; do not replace it with ad-hoc regenerated fixtures or alternate corpora.
- Prefer the smallest faithful reproducer:
  - C ABI tests for header/ABI/varargs/linkage behaviors
  - Rust unit tests for module-local parser/dumper/container invariants
  - shell or image-backed cases only when the failure is inherently packaging- or application-specific
- `safe/tests/regressions/manifest.json` should record, per case:
  - `issue_id` for any case that corresponds to a discovered incompatibility
  - `runner` (`c`, `rust`, `shell`, or `image`)
  - case path
  - `status_before_fix` (`failing`, `passing`, or `not_applicable`)
- `safe/scripts/run-regressions.sh` must support a pre-fix verification mode that checks the expected pre-fix statuses instead of assuming every case passes immediately.
- Cases that exist only as passing baselines and do not correspond to a discovered incompatibility may omit `issue_id` in `safe/tests/regressions/manifest.json`; the required one-to-one coverage rule applies only to explicit `APP-*` issue IDs recorded in `safe/tests/regressions/discovered-issues.md`.
- If Phase 2 found zero incompatibilities, this phase must still create permanent regression coverage for the new image + `nghttp2` path and mark those cases `status_before_fix=passing`, without inventing fake failure IDs.

## Verification Phases
### `check_regression_reproducers`
Phase ID: `check_regression_reproducers`
Type: `check`
Fixed `bounce_target`: `impl_regression_reproducers`
Purpose: Verify that every discovered application-level issue has a checked-in reproducer and that the regression runner can execute the pre-fix expectations deterministically.

Commands:
```sh
test -f safe/tests/regressions/discovered-issues.md
test -f safe/tests/regressions/manifest.json
python3 - <<'PY'
import json, re, pathlib, sys
issues = set(re.findall(r'^## (APP-[A-Z0-9-]+)$', pathlib.Path('safe/tests/regressions/discovered-issues.md').read_text(), re.M))
manifest = json.loads(pathlib.Path('safe/tests/regressions/manifest.json').read_text())
covered = {item['issue_id'] for item in manifest.get('cases', []) if item.get('issue_id')}
missing = sorted(issues - covered)
if missing:
    raise SystemExit('missing regression coverage for: ' + ', '.join(missing))
PY
safe/scripts/run-regressions.sh --respect-status-before-fix
```

## Success Criteria
- Confirm issue-to-regression coverage is complete.
- Confirm the regression runner reproduces the recorded pre-fix state.
- Keep the regression inventory checked into the repository instead of leaving it only in transient container logs.

## Git Commit Requirement
The implementer must commit work to git before yielding. The commit message must include `impl_regression_reproducers`.
