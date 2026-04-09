## Phase Name
Arrays, Objects, Iterator ABI, Copy/Equality, And Seeded Hashing

## Implement Phase ID
`impl_containers_iterators`

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
- `safe/src/lib.rs`
- `safe/src/abi.rs`
- `safe/src/error.rs`
- `safe/src/version.rs`
- `safe/src/raw/alloc.rs`
- `safe/src/utf.rs`
- `safe/src/scalar.rs`
- `safe/tests/upstream-api/test_array.c`
- `safe/tests/upstream-api/test_object.c`
- `safe/tests/upstream-api/test_fixed_size.c`
- `safe/tests/upstream-api/test_copy.c`
- `safe/tests/upstream-api/test_equal.c`
- `original/jansson-2.14/src/value.c`
- `original/jansson-2.14/src/jansson_private.h`
- `original/jansson-2.14/src/hashtable.h`
- `original/jansson-2.14/src/hashtable.c`
- `original/jansson-2.14/src/hashtable_seed.c`
- `original/jansson-2.14/src/strbuffer.c`
- `original/jansson-2.14/doc/threadsafety.rst`
- `relevant_cves.json`

## New Outputs
- allocator-aware array storage
- object storage that preserves both hash lookup and insertion order
- stable iterator/key ABI compatible with upstream macros
- copy, deep-copy, and equality logic for containers
- seeded-hashing implementation that preserves the upstream `json_object_seed()` contract
- a focused container/iterator compatibility checker that does not depend on `json_load*`, `json_dump*`, or `json_pack*`

## File Changes
- Create `safe/src/raw/buf.rs`
- Create `safe/src/raw/list.rs`
- Create `safe/src/raw/table.rs`
- Create `safe/src/array.rs`
- Create `safe/src/object.rs`
- Update `safe/src/scalar.rs`
- Update `safe/src/lib.rs`
- Create `safe/scripts/check-container-primitives.sh`

## Implementation Details
- Implement arrays with manual growable storage, not `Vec`, so all Jansson-owned allocations continue to respect `json_set_alloc_funcs()`.
- Use `original/jansson-2.14/src/strbuffer.c` as the source of truth for the allocator-aware buffer primitives introduced in `safe/src/raw/buf.rs`; later phases should extend that shared buffer layer in place instead of introducing parallel buffer implementations.
- Preserve the appendix-defined ownership of the phase-3 container files:
  - `safe/src/raw/buf.rs` is the allocator-aware growable byte buffer shared by strings now and later by dump buffers, token capture, and formatting helpers; extend it in place.
  - `safe/src/raw/list.rs` owns the intrusive doubly linked list primitives for insertion-ordered object iteration.
  - `safe/src/raw/table.rs` owns the manual bucket table and must be structured so later phases can reuse it for transient visited sets, circular-reference tracking, and parser/dumper auxiliary sets instead of creating separate tables.
  - `safe/src/array.rs` owns array allocation, growth, insertion, removal, extension, shallow copy, deep copy, and deletion semantics.
  - `safe/src/object.rs` owns object entry allocation with trailing key bytes, lookup/update/delete, iterator APIs, recursive update logic, and hash seeding.
  - `safe/scripts/check-container-primitives.sh` is the focused pre-parser/pre-variadic compatibility checker for iterator ABI, fixed-size key semantics, manual copy/equality coverage, and the `json_object_seed()` contract.
- Implement objects with two linked structures, just as upstream does:
  - bucket chains for lookup
  - an ordered list for iteration order
- Preserve the key-pointer ABI from `hashtable_key_to_iter()`:
  - object entries must be single allocations that contain iterator links, metadata, and trailing key bytes in one block
  - `json_object_iter_key()` must return a pointer into that block
  - `json_object_key_to_iter()` must recover the iterator from the key pointer alone
- Preserve upstream ownership and key-length semantics exactly:
  - `json_array_set_new`, `json_array_append_new`, `json_array_insert_new`, `json_object_set_new*`, `json_object_setn_new*`, and `json_object_iter_set_new` steal the passed reference on success and decref it on failure where upstream does
  - the inline non-`_new` wrappers preserve caller ownership by `json_incref()`ing before they call the `_new` entry points
  - the C-string key APIs (`json_object_get`, `json_object_set_new`, `json_object_set_new_nocheck`, `json_object_del`, and the inline wrappers built on them) use `strlen(key)` and therefore stop at the first embedded NUL
  - only the explicit-length `*n*` APIs preserve embedded NUL and arbitrary bytes; checked `*n*` variants validate UTF-8 across the full supplied length, while `_nocheck` `*n*` variants skip UTF-8 validation but still honor the supplied length verbatim
  - array insert/remove/set behavior and self-reference rejection match the bounds and cycle checks in `value.c`
- Implement `json_object_update()`, `json_object_update_existing()`, `json_object_update_missing()`, and `json_object_update_recursive()` with the exact overwrite/merge semantics from upstream tests, including preserving nested object identity when both sides contain objects.
- Implement `json_copy()` and `json_deep_copy()` so:
  - shallow copies of arrays/objects preserve child identity
  - deep copies duplicate children
  - circular references are rejected instead of recursing forever
- Implement `json_equal()` with upstream null/type handling and insertion-order-independent object comparison.
- Implement randomized seeded hashing by default, with `json_object_seed(size_t seed)` honoring the upstream rule that seeding only happens before the first object materializes and that `json_object()` implicitly autoseeds if no explicit seed was set. Use OS randomness first and keep the upstream time/pid fallback so `CVE-2013-6401` stays mitigated even without entropy sources.
- Create `safe/scripts/check-container-primitives.sh` in this phase. It should build and run focused compatibility checks that do not depend on `json_load*`, `json_dump*`, or `json_pack*`, covering:
  - object iteration, insertion order, and key-pointer round-tripping through `json_object_iter_key()` and `json_object_key_to_iter()`
  - fixed-size and binary-key behavior through `json_object_getn`, `json_object_setn*`, `json_object_setn*_nocheck`, `json_object_deln`, and `json_object_iter_key_len`
  - array/object copy and equality on manually constructed values
  - a white-box seed-contract check showing that `json_object_seed()` only takes effect before the first object and that `json_object()` auto-seeds when no explicit seed was supplied
- Do not gate this phase on the full upstream API executables. `test_fixed_size`, `test_copy`, and `test_equal` first become runnable in phase 4 after `json_dump*` / `json_load*` exist. `test_array` and `test_object` first become runnable in phase 5 after `json_pack*` exists.

## Verification Phases
### `check_containers_iterators`
Phase ID: `check_containers_iterators`
Type: `check`
Fixed `bounce_target`: `impl_containers_iterators`
Purpose: Validate container mutation semantics, fixed-length and binary keys, insertion-order iteration, copy/deep-copy/equality behavior, and hash seeding without depending on parser, dumper, or variadic format APIs that are not implemented yet.

Commands:
```sh
cargo build --manifest-path safe/Cargo.toml --release
safe/scripts/check-container-primitives.sh
safe/scripts/check-exports.sh --names-only
```

## Success Criteria
- The focused phase-local container and iterator checker passes.
- The export surface remains intact.
- `json_object_seed()` continues to satisfy the seeded-hashing requirement from `relevant_cves.json`.

## Git Commit Requirement
The implementer must commit work to git before yielding, with a message that begins with `impl_containers_iterators:`.
