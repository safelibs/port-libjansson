# Safety Inventory

`safe/src/lib.rs` enables `#![deny(unsafe_op_in_unsafe_fn)]`, so every remaining unsafe operation is explicit. The final non-safe surface is intentionally limited to the categories required by this port:

- public C ABI entrypoints and public-layout pointer casting
- allocator-hook integration
- raw `FILE *` / fd interaction
- the stable-Rust variadic boundary

Everything else, including parser state transitions, dump formatting decisions, hash selection, and most error handling logic, stays in safe Rust.

## Audit Contract

The final verifier runs `rg -n '\bunsafe\b|extern "C"|no_mangle' safe/src safe/csrc` and then requires every Rust file reported by that audit to remain documented here. The current residual Rust audit file set is:

- `safe/src/abi.rs`
- `safe/src/array.rs`
- `safe/src/dump.rs`
- `safe/src/error.rs`
- `safe/src/load.rs`
- `safe/src/object.rs`
- `safe/src/pack.rs`
- `safe/src/raw/alloc.rs`
- `safe/src/raw/buf.rs`
- `safe/src/raw/list.rs`
- `safe/src/raw/table.rs`
- `safe/src/scalar.rs`
- `safe/src/strconv.rs`
- `safe/src/unpack.rs`
- `safe/src/utf.rs`
- `safe/src/version.rs`

The two checked-in C shims are part of the residual surface as well and are documented separately below, even though they are not selected by the Rust-oriented `rg` pattern.

## Residual C Boundaries

### `safe/csrc/pack_unpack_shim.c`

- Owns the only stable C variadic ingress for `json_pack*` and `json_unpack*`.
- Scans the format string, reads `va_list` arguments, and marshals them into typed `jsonp_pack_arg` / `jsonp_unpack_arg` arrays.
- Defers all JSON graph construction, ownership transfer, validation, and error reporting to Rust via `jsonp_pack_marshaled()` and `jsonp_unpack_marshaled()`.
- Uses Jansson allocator hooks for temporary argument buffers; it does not allocate Rust-owned objects directly.

### `safe/csrc/sprintf_shim.c`

- Owns the `json_sprintf()`, `json_vsprintf()`, `jsonp_error_set()`, and `jsonp_error_vset()` variadic ABI.
- Uses `vsnprintf()` because stable Rust cannot implement C varargs formatting directly.
- Routes allocation through `jsonp_malloc()` and hands the owned buffer to Rust with `jsonp_sprintf_string_own()`.
- Does not implement JSON parsing or container logic.

No other checked-in C sources remain.

## Rust Unsafe Inventory

### `safe/src/abi.rs`

- `is_type()`, `is_object()`, `is_array()`, `is_string()`, `is_integer()`, `is_real()`, `is_true()`, `is_false()`, `is_null()`, `is_number()`, and `type_of()` dereference caller-provided `json_t *` values from the public ABI.
- `atomic_refcount()`, `incref()`, and `decref()` reinterpret the public `json_t.refcount` field as `AtomicUsize` and are the only place that overlays atomic semantics onto the public C layout.
- Justification: upstream exposes `json_t { type; refcount; }` as a public prefix layout, so reference counting and type dispatch must operate on raw public pointers.

### `safe/src/error.rs`

- `jsonp_error_init()`, `jsonp_error_set_source()`, and `jsonp_error_vformat()` write into caller-provided `json_error_t *` buffers and read source/text C strings.
- Justification: the public ABI requires in-place mutation of a C layout struct with fixed-size character arrays.

### `safe/src/raw/alloc.rs`

- `json_get_alloc_funcs()`, `jsonp_malloc()`, `jsonp_free()`, `jsonp_strdup()`, `jsonp_strndup()`, `alloc()`, and `free()` call user-supplied C allocator hooks and move raw buffers across the ABI boundary.
- Justification: Jansson exposes allocator override hooks, so raw function-pointer invocation is required.

### `safe/src/raw/list.rs`

- `init()`, `is_empty()`, `insert_before()`, and `remove()` implement the intrusive doubly linked list used by objects and hash buckets.
- Justification: these nodes are embedded inside C-layout container entries, so list manipulation must use raw self-referential pointers.

### `safe/src/raw/table.rs`

- `RawTable` methods manipulate bucket arrays allocated through `jsonp_malloc()`, walk intrusive list nodes, and rehash by pointer.
- `PointerSet` stores raw graph/object pointers for cycle detection during deep copy and dump.
- Justification: object hashing, dump cycle detection, and deep-copy recursion guards need stable pointer identity and allocator-hook ownership.

### `safe/src/raw/buf.rs`

- `RawBuf` allocates, grows, steals, and frees NUL-terminated buffers with raw pointer copies.
- `dup_bytes()` / `dup_cstr()` duplicate caller-provided byte and C-string buffers.
- Justification: these buffers back C-visible strings, error text, and parser scratch storage, and therefore must preserve allocator-hook semantics and exact NUL termination.

### `safe/src/scalar.rs`

- `init_json()`, the `as_*` cast helpers, `string_create()`, clone/delete helpers, `jsonp_stringn_nocheck_own()`, `jsonp_sprintf_string_own()`, string setters/getters, and numeric constructors/setters cast between the public `json_t` prefix and concrete heap objects.
- `json_delete()` dispatches destruction by reading the public type tag from a raw pointer.
- Justification: scalar values are heap allocations whose first field is the public `json_t` header, matching upstream layout and ownership rules.

### `safe/src/array.rs`

- `as_array_ptr()`, `as_array_mut()`, `array_move()`, `array_copy()`, `array_grow()`, `append_borrowed()`, `delete_array()`, `equal_array()`, `copy_array()`, `deep_copy_array()`, and the public `json_array_*` ABI work against a raw `*mut json_t` element table.
- Justification: arrays expose C ABI pointers, hold heterogenous `json_t *` entries, and must preserve exact reference-count and ownership semantics.

### `safe/src/object.rs`

- `as_object_ptr()`, `as_object_mut()`, entry/link/key conversion helpers, ordered-iteration helpers, `hash_key()`, entry lookup/allocation/free helpers, object copy/update/delete helpers, and the public `json_object_*` ABI manipulate intrusive object entries and bucket links through raw pointers.
- `json_object()` calls `ensure_seed(None)` before first object creation; `json_object_seed()` calls `ensure_seed(Some(seed))` and becomes a no-op after the first seed is established.
- Justification: object iteration cookies, insertion-order preservation, and keyed hashing all depend on public-layout pointer casting and intrusive storage.

### `safe/src/utf.rs`

- `validate_ptr()` converts a caller-provided `char *` plus length into a byte slice.
- Justification: the public ABI still accepts raw C strings and explicit lengths.

### `safe/src/strconv.rs`

- `errno_location()`, `set_errno()`, `get_errno()`, and `strtod()` interoperate with libc locale conversion state, mutable parser scratch buffers, and `errno`.
- Justification: numeric parsing and formatting must match the C ABI and system locale behavior that downstream callers expect.

### `safe/src/load.rs`

- `errno_location()`, `set_errno()`, and `get_errno()` read and write libc `errno`.
- `Lexer` stream helpers read from raw string/buffer pointers, `FILE *`, file descriptors, and callback-provided byte buffers.
- `cleanup_frames()`, `push_value_frame()`, `parse_json()`, and `parse_with_source()` transfer ownership of partially built `json_t *` graphs and parser scratch buffers.
- `json_loads()`, `json_loadb()`, `json_loadf()`, `json_loadfd()`, `json_load_file()`, and `json_load_callback()` are the public C ingress points.
- Justification: parsing must accept upstream-compatible C sources, but the actual parse is iterative. `parse_json()` uses a heap `Vec<Frame>` plus `JSON_PARSER_MAX_DEPTH = 2048`, so attacker-controlled nesting cannot recurse on the process stack.

### `safe/src/dump.rs`

- `dump_to_strbuffer()`, `dump_to_buffer()`, `dump_to_file()`, and `dump_to_fd()` call raw callback sinks and libc file/fd APIs.
- `emit_bytes()`, `dump_indent()`, `dump_string()`, `enter_parent()`, and `do_dump()` walk raw `json_t *` graphs, track parent pointers, and emit into C-visible sinks.
- `json_dumpf()`, `json_dumpfd()`, `json_dump_file()`, and `json_dump_callback()` expose the public dump ABI.
- Justification: serialization must support user callbacks, `FILE *`, and fd targets, and cycle detection uses raw pointer identity.

### `safe/src/pack.rs`

- `next_cstr()`, `next_int()`, `next_size()`, `next_double()`, `next_json()`, `validate_utf8()`, `read_string()`, `pack_integer()`, `pack_real()`, `pack_object_inter()`, `pack_string()`, `pack_object()`, `pack_array()`, `pack()`, and `jsonp_pack_marshaled()` read typed marshaled arguments emitted by the C shim.
- Justification: the Rust side does not touch `va_list` directly; it consumes the shim’s typed representation and then performs ordinary JSON construction.

### `safe/src/unpack.rs`

- `next_key()`, `next_string_target()`, `next_size_target()`, `next_int_target()`, `next_json_int_target()`, `next_double_target()`, `next_json_target()`, `remember_key()`, `append_unrecognized_key()`, the `unpack_*` helpers, and `jsonp_unpack_marshaled()` read typed target pointers emitted by the C shim and write results back through the C ABI.
- Justification: unpacking must store directly into caller-provided C destinations, but the Rust side still avoids direct `va_list` handling.

### `safe/src/version.rs`

- This file appears in the final audit because it exports `#[no_mangle] extern "C"` version entrypoints.
- No unsafe code remains.

## Final Constraints

- Randomized object hashing is the default. The only deterministic override is `json_object_seed()`, and it only takes effect before the first object use initializes the process-global seed.
- Parsing is iterative and depth-bounded; no remaining unsafe path reintroduces recursive descent on attacker-controlled input.
- The mirrored upstream corpus under `safe/tests/` is kept synchronized by `safe/scripts/sync-upstream-tests.sh` and is the default execution corpus for the API and data-suite harnesses used by the final verifier.
