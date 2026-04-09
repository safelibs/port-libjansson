## Phase Name
Variadic Pack/Unpack/Sprintf Boundary And OOM/Chaos Behavior

## Implement Phase ID
`impl_format_varargs`

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
- `safe/scripts/check-exports.sh`
- `safe/scripts/sync-upstream-tests.sh`
- `safe/scripts/build-upstream-api-tests.sh`
- `safe/scripts/check-allocator-hooks.sh`
- `safe/scripts/run-upstream-api-tests.sh`
- `safe/scripts/check-container-primitives.sh`
- `safe/scripts/run-data-suites.sh`
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
- `safe/tests/upstream-api/test_array.c`
- `safe/tests/upstream-api/test_dump.c`
- `safe/tests/upstream-api/test_memory_funcs.c`
- `safe/tests/upstream-api/test_object.c`
- `safe/tests/upstream-api/test_pack.c`
- `safe/tests/upstream-api/test_unpack.c`
- `safe/tests/upstream-api/test_sprintf.c`
- `safe/tests/upstream-api/test_chaos.c`
- `safe/tests/upstream-api/util.h`
- `original/jansson-2.14/src/pack_unpack.c`
- `original/jansson-2.14/src/value.c`
- `original/jansson-2.14/src/strbuffer.c`

## New Outputs
- working `json_pack*`, `json_unpack*`, and `json_sprintf*` implementations
- a minimal residual C shim for stable-Rust variadic ABI ingress
- proven cleanup behavior under allocator failure

## File Changes
- Update `safe/csrc/pack_unpack_shim.c`
- Update `safe/csrc/sprintf_shim.c`
- Create `safe/src/pack.rs`
- Create `safe/src/unpack.rs`
- Update `safe/build.rs`
- Update `safe/src/lib.rs`

## Implementation Details
- Keep stable Rust. Do not switch the project to nightly solely for c-variadic support.
- Replace the phase-1 placeholder bodies in the residual C shims with the real exported variadic entry points. The exported symbol set and file locations stay unchanged; phase 5 only fills in the real behavior:
  - `json_pack`
  - `json_pack_ex`
  - `json_vpack_ex`
  - `json_unpack`
  - `json_unpack_ex`
  - `json_vunpack_ex`
  - `json_sprintf`
  - `json_vsprintf`
- Keep the C shim narrow:
  - receive `...` or `va_list`
  - walk the format string only as far as needed to marshal arguments into an explicit typed helper representation
  - leave JSON construction, validation, mutation, reference handling, and error management in Rust
- Preserve the appendix-defined ownership of the phase-5 variadic-boundary files:
  - `safe/src/pack.rs` owns Rust-side helpers for pack-format parsing and JSON construction.
  - `safe/src/unpack.rs` owns Rust-side helpers for unpack-format parsing, validation, and error generation.
  - `safe/csrc/pack_unpack_shim.c` remains the residual C ABI shim for `json_pack*` / `json_unpack*` variadic and `va_list` entry points only.
  - `safe/csrc/sprintf_shim.c` remains the residual C ABI shim for `json_sprintf` / `json_vsprintf`, while `safe/src/scalar.rs` continues to own the non-variadic formatting helpers the shim calls after marshaling arguments.
  - `safe/build.rs` and `safe/src/lib.rs` must preserve the existing symbol/export wiring and shim compilation surface while swapping the placeholder implementations for the real ones.
- Preserve upstream format-string semantics and diagnostics:
  - string-length suffixes `#` and `%`
  - concatenation `+`
  - optional / nullable `?` and `*`
  - strict unpack marker `!`
  - `JSON_VALIDATE_ONLY`
  - `JSON_STRICT`
  - `o` versus `O` refcount semantics
  - UTF-8 validation of keys and strings
  - exact error messages, sources, positions, and codes
- Treat `test_chaos.c` as a design constraint. Every partially built array/object/string graph must unwind cleanly through the allocator hooks.
- Keep `json_sprintf*` output buffering on the shared `safe/src/raw/buf.rs` path derived from `original/jansson-2.14/src/strbuffer.c`; do not introduce a second formatting-buffer implementation just for the variadic boundary.
- Treat the deferred upstream tests as acceptance criteria:
  - `test_memory_funcs.c` now proves allocator-hook completeness once `json_pack()` exists
  - `test_array.c`, `test_object.c`, and `test_dump.c` now prove the remaining cross-module interactions between containers, dumping, loading, and variadic construction
- Do not let the residual C surface grow beyond the variadic ingress that Rust cannot express safely on stable.
- Before yielding, commit all phase work to git with a message that begins with `impl_format_varargs:`.

## Verification Phases
### `check_format_varargs`
Phase ID: `check_format_varargs`
Type: `check`
Fixed `bounce_target`: `impl_format_varargs`
Purpose: Validate the remaining format-string-driven ABI and the deferred cross-module upstream tests that only become runnable once variadic entry points exist, including exact error messages, refcount semantics, and all allocation-failure cleanup paths.

Commands:
```sh
cargo build --manifest-path safe/Cargo.toml --release
safe/scripts/build-upstream-api-tests.sh test_array test_dump test_memory_funcs test_object test_pack test_sprintf test_unpack test_chaos
safe/scripts/run-upstream-api-tests.sh test_array test_dump test_memory_funcs test_object test_pack test_sprintf test_unpack test_chaos
safe/scripts/check-exports.sh --names-only
```

## Success Criteria
- `test_array`, `test_dump`, `test_memory_funcs`, `test_object`, `test_pack`, `test_unpack`, `test_sprintf`, and `test_chaos` compile and run successfully.
- Allocation-failure cleanup remains correct.
- The export checker confirms that the variadic shims do not regress the public symbol surface.

## Git Commit Requirement
The implementer must commit work to git before yielding, with a message that begins with `impl_format_varargs:`.
