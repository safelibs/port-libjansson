# Compatibility Guarantees

This phase establishes the first end-to-end compatibility contract for the safe port as an Ubuntu drop-in replacement for upstream `libjansson`.

## Source Compatibility

- The installed public headers stay at `/usr/include/jansson.h` and `/usr/include/jansson_config.h`.
- Installed-dev verification compiles the mirrored API tests through `pkg-config` against those installed headers.
- Link-compat verification stages the original upstream `jansson.h` text together with the installed `jansson_config.h`, then compiles upstream consumer objects against that mixed header surface.

## Link Compatibility

- The shared library SONAME remains `libjansson.so.4`.
- Export names are checked against `original/jansson-2.14/src/jansson.def`.
- Exported symbol versions are checked against `original/jansson-2.14/debian/libjansson4.symbols`.
- `safe/scripts/check-link-compat.sh` links upstream API-test objects, the upstream `json_process` object, and the upstream `simple_parse` example object against the installed safe shared library and rejects any `ldd` resolution outside the selected installed root.

## Runtime Compatibility

- `safe/scripts/run-upstream-api-tests.sh --all` runs all 18 mirrored API tests against the build tree.
- `safe/scripts/run-upstream-api-tests.sh --installed-dev --installed-root <root> --all` runs those same 18 mirrored API tests against the installed shared library selected through the installed package's `pkg-config` file.
- `safe/scripts/run-data-suites.sh valid invalid invalid-unicode encoding-flags` uses the mirrored `safe/tests/upstream-suites/**` corpus in build-tree mode.
- `safe/scripts/run-data-suites.sh --installed-dev --installed-root <root> ...` uses that same mirrored corpus while compiling `json_process` against the installed static archive surface.
- [`test-original.sh`](/home/yans/safelibs/port-libjansson/test-original.sh) is the authoritative downstream runtime entrypoint and now accepts:
  - `JANSSON_IMPLEMENTATION=original|safe`
  - `JANSSON_TEST_MODE=build|runtime|all`
- The default invocation (`./test-original.sh`) preserves the original runtime baseline: it builds `original/jansson-2.14` into `/usr/local`, exports `LD_LIBRARY_PATH`, and runs the downstream smoke tests against that overlay.
- Safe runtime/build modes consume the prebuilt local `.deb` packages from `safe/dist/`, install them with `dpkg -i`, and then run the same dependent smoke tests or dependent rebuild harness against the actual system-package replacement.
- The ulogd JSON-plugin linkage probe no longer hard-codes an architecture path; it resolves the active plugin location from the installed package contents before asserting linkage.

## Downstream Build Compatibility

- [`safe/scripts/check-dependent-builds.sh`](/home/yans/safelibs/port-libjansson/safe/scripts/check-dependent-builds.sh) is the authoritative compile-compatibility harness for every unique `source_package` named in [`dependents.json`](/home/yans/safelibs/port-libjansson/dependents.json).
- `JANSSON_IMPLEMENTATION=original` uses Ubuntu's archive `libjansson4` and `libjansson-dev` packages as the package-manager compile baseline; `JANSSON_IMPLEMENTATION=safe` switches that same harness to the locally built replacement packages.
- The harness parses the manifest with `jq -r '.dependents[].source_package' dependents.json | sort -u` and currently rebuilds exactly these 11 Ubuntu 24.04 source packages:
  - `emacs`
  - `janus`
  - `jose`
  - `jshon`
  - `libteam`
  - `mtr`
  - `suricata`
  - `tang`
  - `ulogd2`
  - `wayvnc`
  - `webdis`
- In safe mode the harness installs the locally built `libjansson4` and `libjansson-dev` packages first, pins their exact versions in `/etc/apt/preferences.d/`, and marks them held so `apt-get build-dep` cannot silently replace them with the Ubuntu archive build.
- The rebuild sequence for each dependent is:
  - enable `deb-src` entries when the container image lacks source repositories
  - `apt-get source "$srcpkg"`
  - `apt-get build-dep -y "$srcpkg"`
  - `DEB_BUILD_OPTIONS=nocheck dpkg-buildpackage -B -uc -us`
- The Emacs rebuild uses Debian's packaged `EMACS_INHIBIT_NATIVE_COMPILATION=1` switch during `dpkg-buildpackage` so containerized rebuilds do not fail in `dh_strip` on transient `.eln` artifacts unrelated to the libjansson dependency edge under test.
- Any build failure, missing source package, or Jansson package-version drift aborts the harness immediately.

## Packaging Compatibility

- The emitted binary package names match Ubuntu exactly: `libjansson4` and `libjansson-dev`.
- The checked-in package templates under `safe/pkg/DEBIAN/` preserve the upstream runtime and development package relationships.
- Multiarch installation paths are resolved from `dpkg-architecture -qDEB_HOST_MULTIARCH`; no checked-in manifest hard-codes a literal triplet.
- The installed development surface matches Ubuntu conventions:
  - `/usr/lib/$multiarch/libjansson.so.4.14.0`
  - `/usr/lib/$multiarch/libjansson.so.4`
  - `/usr/lib/$multiarch/libjansson.so`
  - `/usr/lib/$multiarch/libjansson.a`
  - `/usr/include/jansson.h`
  - `/usr/include/jansson_config.h`
  - `/usr/lib/$multiarch/pkgconfig/jansson.pc`

## Versioning Rules

- The Debian package version may advance independently to sort higher than Ubuntu's archive version.
- The upstream API version stays fixed at `2.14`.
- `jansson_version_str()` and `pkg-config --modversion jansson` therefore continue reporting `2.14`.

Later phases should extend this document instead of replacing it.
