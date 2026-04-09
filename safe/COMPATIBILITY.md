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
