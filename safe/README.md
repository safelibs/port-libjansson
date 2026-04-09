# libjansson-safe scaffold

This crate bootstraps the Rust port of `libjansson` under `safe/` while preserving the upstream C ABI surface from the first phase.

Build the phase-1 scaffold with:

```sh
cargo build --manifest-path safe/Cargo.toml --release
```

Verify the exported symbol names and the mirrored upstream test corpus with:

```sh
safe/scripts/check-exports.sh --names-only
safe/scripts/sync-upstream-tests.sh --check
```

The installed public header surface lives in `safe/include/`, the shared object is built with SONAME `libjansson.so.4`, and the checked-in `safe/jansson.map` is derived from `original/jansson-2.14/src/jansson.def`.

Compile the upstream public-header example against the scaffold with:

```sh
cc -I safe/include original/jansson-2.14/examples/simple_parse.c -L safe/target/release -Wl,-rpath,$PWD/safe/target/release -ljansson -o /tmp/jansson-simple-parse
```
