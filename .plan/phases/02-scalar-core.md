## Phase Name
Scalar Core, Refcounting, Allocator Hooks, Errors, And Version APIs

## Implement Phase ID
`impl_scalar_core`

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
- `safe/src/lib.rs`
- `safe/tests/upstream-api/test_simple.c`
- `safe/tests/upstream-api/test_number.c`
- `safe/tests/upstream-api/test_memory_funcs.c`
- `safe/tests/upstream-api/test_version.c`
- `safe/tests/upstream-api/util.h`
- `safe/README.md`
- `original/jansson-2.14/src/jansson.h`
- `original/jansson-2.14/src/jansson_private.h`
- `original/jansson-2.14/src/memory.c`
- `original/jansson-2.14/src/error.c`
- `original/jansson-2.14/src/version.c`

## New Outputs
- working scalar value implementations in Rust
- allocator-hook infrastructure that all later phases can reuse
- exact upstream-compatible `json_error_t` helpers
- version entry points returning the upstream API version
- upstream API test build/run harness for incremental compatibility verification
- a focused allocator-hook compatibility checker that does not depend on `json_pack*`

## File Changes
- Create `safe/src/abi.rs`
- Create `safe/src/error.rs`
- Create `safe/src/version.rs`
- Create `safe/src/raw/alloc.rs`
- Create `safe/src/utf.rs`
- Create `safe/src/scalar.rs`
- Update `safe/src/lib.rs`
- Create `safe/scripts/build-upstream-api-tests.sh`
- Create `safe/scripts/check-allocator-hooks.sh`
- Create `safe/scripts/run-upstream-api-tests.sh`

## Implementation Details
- Define `#[repr(C)]` Rust types whose leading fields exactly match `json_t` and `json_error_t`.
- Implement the three singleton values as process-stable static objects with `refcount == usize::MAX`, matching upstream `json_true()`, `json_false()`, and `json_null()` in `original/jansson-2.14/src/value.c:966-983`.
- For heap values, make the public `json_t` header the first field of each heap allocation so `json_delete()` can dispatch by `type` exactly like upstream.
- Treat `refcount` as externally mutable because C callers compiled against the header will inline refcount increments and decrements. On the Rust side, only use narrowly-scoped `unsafe` conversions to atomic access where the platform macros in `jansson_config.h` promise thread-safe refcounting.
- Implement global allocator hooks following `memory.c` exactly:
  - default to libc `malloc` / `free`
  - store function pointers globally
  - implement both `json_set_alloc_funcs()` and `json_get_alloc_funcs()`
  - route every Jansson-owned object allocation and internal buffer allocation through those hooks
  - preserve upstream null-handling behavior for zero-size allocations and null frees
- Implement string, integer, and real creation/set/get APIs and `json_number_value()` with upstream return conventions:
  - bad-type getters return `NULL`, `0`, or `0.0`
  - bad-type setters return `-1`
  - `json_real()` and `json_real_set()` reject `NaN` and `Inf`
  - `json_string()` validates UTF-8; `_nocheck` variants do not
- Implement `jsonp_error_init`, `jsonp_error_set_source`, and `jsonp_error_vset` exactly, including:
  - truncating `source` with a `...suffix` strategy
  - storing the numeric error code in the last byte of `error.text`
  - preserving already-set errors
- Keep `JANSSON_VERSION`, `jansson_version_str()`, and `jansson_version_cmp()` reporting upstream `2.14`, even if the Debian package version later gains a `+safe` suffix.
- Preserve the appendix-defined ownership of the phase-2 core files:
  - `safe/src/abi.rs` owns `#[repr(C)]` public layouts, constants, and raw ABI helpers for `json_t`, `json_error_t`, and shared type tags.
  - `safe/src/error.rs` owns exact `json_error_t` initialization, source truncation, error-code packing, and formatting logic.
  - `safe/src/version.rs` owns `jansson_version_str()` and `jansson_version_cmp()` behavior fixed to the upstream API version.
  - `safe/src/raw/alloc.rs` owns global allocator hook storage and raw allocation wrappers used by all Jansson-owned allocations.
  - `safe/src/utf.rs` starts the shared UTF-8 encode/validate/iterate helper layer that later phases must extend in place instead of replacing.
  - `safe/src/scalar.rs` owns strings, integers, reals, singletons, scalar getters/setters, scalar copy/equality/delete logic, and the non-variadic `json_sprintf` helpers that the C shim will call once phase 5 wires the variadic ingress.
  - `safe/src/lib.rs` must register the new ABI modules and keep the public export wiring centralized as functionality lands.
  - `safe/scripts/build-upstream-api-tests.sh` and `safe/scripts/run-upstream-api-tests.sh` are now the owned mirrored-API build/run harness, while `safe/scripts/check-allocator-hooks.sh` is the focused ABI-level checker for allocator routing before `json_pack*` exists.
- Create `safe/scripts/build-upstream-api-tests.sh` and `safe/scripts/run-upstream-api-tests.sh` together so later phases and the final regression sweep have an owned API-test execution path:
  - `build-upstream-api-tests.sh` should compile named tests from `safe/tests/upstream-api/` into `safe/.build/api-tests/`, emit a manifest of built executables, default to a build-tree mode that uses `safe/include` plus `safe/target/release`, and accept future `--all` and `--installed-dev` modes once later phases wire them
  - `run-upstream-api-tests.sh` should execute named tests from that manifest/build directory, default to the same build-tree mode, and fail fast on the first error
  - both scripts may accept an explicit override back to `original/jansson-2.14/test/suites/api` for debugging, but from this phase onward every normal mode must default to the mirrored `safe/tests/upstream-api/` tree
- Create `safe/scripts/check-allocator-hooks.sh` in this phase. It should compile and run a small C program against `safe/include/` and the built library that:
  - installs custom `malloc` / `free` hooks
  - round-trips them through `json_get_alloc_funcs()`
  - allocates and frees representative scalar values and UTF-8 strings
  - fails if the custom hooks are not observed on those phase-2-supported operations
- Do not run upstream `test_memory_funcs` until phase 5; it depends on `json_pack()`.
- Use the initial `safe/src/utf.rs` created in this phase for the UTF-8 validation needed by `json_string*()` and extend it later instead of duplicating ad-hoc validators.

## Verification Phases
### `check_scalar_core`
Phase ID: `check_scalar_core`
Type: `check`
Fixed `bounce_target`: `impl_scalar_core`
Purpose: Validate the fundamental C-visible value layout, singleton behavior, allocator indirection, error packing, and version APIs.

Commands:
```sh
cargo build --manifest-path safe/Cargo.toml --release
safe/scripts/build-upstream-api-tests.sh test_simple test_number test_version
safe/scripts/run-upstream-api-tests.sh test_simple test_number test_version
safe/scripts/check-allocator-hooks.sh
safe/scripts/check-exports.sh --names-only
```

## Success Criteria
- `test_simple`, `test_number`, and `test_version` compile and run successfully.
- The phase-local allocator-hook checker passes.
- The export-name checker reruns successfully with no symbol-surface regression.

## Git Commit Requirement
The implementer must commit work to git before yielding, with a message that begins with `impl_scalar_core:`.
