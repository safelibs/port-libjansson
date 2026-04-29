# port-libjansson

SafeLibs port of `libjansson` for Ubuntu 24.04. Built via the port-owned `safe/scripts/build-deb.sh`.

This repository follows the [`safelibs/port-template`](https://github.com/safelibs/port-template) contract. See [`AGENTS.md`](AGENTS.md) for the canonical layout, hook-script contracts, and CI sequence.

## Layout

- `original/` — pinned upstream `jansson` source for differential testing.
- `safe/` — Rust-based safe implementation plus `safe/scripts/` build and test harnesses.
- `test-original.sh`, `test-safe.sh` — port-internal differential test runners that drive a docker-based dependent matrix.
- `scripts/` — template hook scripts (`install-build-deps.sh`, `build-debs.sh`, etc.).
- `packaging/package.env` — `SAFELIBS_LIBRARY` identifier for the validator hook; the `DEB_*` fields are scaffolding (the real metadata lives in the port's own packaging).

## Local Build

```sh
bash scripts/install-build-deps.sh
bash scripts/check-layout.sh
rm -rf build dist
bash scripts/build-debs.sh
```

`.deb` artifacts land in `dist/`.
