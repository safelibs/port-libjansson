## Phase Name
Dependent Image Scaffold And 12th-Application Inventory

## Implement Phase ID
`impl_dependent_image_scaffold`

## Preexisting Inputs
Consume these artifacts in place. If they already exist, update them instead of rediscovering or regenerating them.
- `dependents.json`
- `test-original.sh`
- `test-safe.sh`
- `safe/scripts/check-dependent-builds.sh`
- `safe/scripts/build-deb.sh`
- `safe/README.md`
- `safe/COMPATIBILITY.md`
- `safe/dist/*.deb when present`

## New Outputs
- `safe/docker/dependent-matrix.Dockerfile`
- `safe/scripts/build-dependent-image.sh`
- updated `dependents.json` with a twelfth source package entry for `nghttp2`
- updated documentation that names the 12-application matrix and image workflow

## File Changes
- Create `safe/docker/dependent-matrix.Dockerfile`
- Create `safe/scripts/build-dependent-image.sh`
- Modify `dependents.json`
- Modify `safe/scripts/check-dependent-builds.sh`
- Modify `safe/README.md`
- Modify `safe/COMPATIBILITY.md`

## Implementation Details
- Add `nghttp2-client` / `nghttp2` to `dependents.json`, preserving the existing manifest schema:
  - `binary_package`: `nghttp2-client`
  - `source_package`: `nghttp2`
  - `category`: `HTTP/2 client`
  - runtime functionality should reference HAR generation and/or HPACK JSON tooling
  - `source_evidence` strings should cite the Ubuntu 24.04 `nghttp2` source locations that use Jansson in `src/nghttp.cc` and `src/deflatehd.cc`
- Count downstream matrix members strictly by `dependents.json`. `nghttp2-client` / `nghttp2` is the twelfth counted application. `nghttp2-server` may be installed in the image only as a helper fixture for the `nghttp2-client` smoke test and must not become a thirteenth manifest entry.
- Update `safe/scripts/check-dependent-builds.sh` so its authoritative expected source-package set matches the expanded manifest instead of failing on the 12th package.
- Create a reusable Dockerfile under `safe/docker/` that installs:
  - the existing union of build/runtime prerequisites already encoded in `test-original.sh`
  - the 12 primary application binaries defined by `dependents.json`
  - only those extra helper binaries that are required to exercise one of the 12 manifest entries, such as `nghttp2-server`
  - the safe `libjansson4` and `libjansson-dev` packages from `safe/dist/` when `--implementation safe` is selected
- The image builder must consume preexisting `safe/dist/*.deb` artifacts if they already exist and only rebuild them in place through `safe/scripts/build-deb.sh` when necessary.
- The image builder must not invent a second package-install path. The final image must use the same Debian packages the rest of the workflow verifies.
- Update user-facing docs so they describe the image builder, the 12-package manifest, and the fact that `dependents.json` remains the one source of truth.

## Verification Phases
### `check_dependent_image_scaffold`
Phase ID: `check_dependent_image_scaffold`
Type: `check`
Fixed `bounce_target`: `impl_dependent_image_scaffold`
Purpose: Verify that the repository has a reusable prepared-image path and that the downstream manifest expands from 11 to 12 concrete Ubuntu source packages.

Commands:
```sh
jq -r '.dependents[].source_package' dependents.json | sort -u >/tmp/libjansson-dependent-sources.txt
test "$(wc -l </tmp/libjansson-dependent-sources.txt)" -eq 12
grep -Fx 'nghttp2' /tmp/libjansson-dependent-sources.txt
safe/scripts/build-deb.sh
image_tag="libjansson-safe-matrix:phase1"; safe/scripts/build-dependent-image.sh --implementation safe --tag "$image_tag"
docker run --rm "$image_tag" sh -lc 'dpkg-query -W libjansson4 libjansson-dev emacs-nox janus jose jshon mtr-tiny suricata tang-common libteam-utils ulogd2 ulogd2-json wayvnc webdis nghttp2-client nghttp2-server >/dev/null'
docker run --rm "$image_tag" sh -lc 'test -f /usr/include/jansson.h && test -f /usr/include/jansson_config.h'
```

## Success Criteria
- `dependents.json` contains exactly 12 unique `source_package` entries and includes `nghttp2`.
- The prepared image builds successfully through `safe/scripts/build-dependent-image.sh`.
- The prepared image contains the safe Debian packages, all required downstream binaries, and the installed public headers.

## Git Commit Requirement
The implementer must commit work to git before yielding. The commit message must include `impl_dependent_image_scaffold`.
