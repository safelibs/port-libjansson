## Phase Name
Full Parser, Encoder, UTF/Numeric Semantics, And Data Suites

## Implement Phase ID
`impl_parse_dump`

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
- `safe/tests/upstream-bin/json_process.c`
- `safe/tests/upstream-scripts/run-tests.sh`
- `safe/tests/upstream-scripts/valgrind.sh`
- `safe/tests/upstream-suites/valid/`
- `safe/tests/upstream-suites/invalid/`
- `safe/tests/upstream-suites/invalid-unicode/`
- `safe/tests/upstream-suites/encoding-flags/`
- `original/jansson-2.14/src/load.c`
- `original/jansson-2.14/src/dump.c`
- `original/jansson-2.14/src/utf.c`
- `original/jansson-2.14/src/strconv.c`
- `original/jansson-2.14/src/strbuffer.c`
- `original/jansson-2.14/doc/conformance.rst`
- `relevant_cves.json`

## New Outputs
- full upstream-compatible parser
- full upstream-compatible dumper
- locale-independent numeric parse/format helpers
- runner support for the data-driven suites

## File Changes
- Update `safe/src/utf.rs`
- Create `safe/src/strconv.rs`
- Create `safe/src/load.rs`
- Create `safe/src/dump.rs`
- Update `safe/src/raw/buf.rs`
- Update `safe/src/lib.rs`
- Create `safe/scripts/run-data-suites.sh`
- Update `safe/scripts/build-upstream-api-tests.sh`

## Implementation Details
- Match upstream UTF-8 rules exactly, including rejection of:
  - overlong encodings
  - surrogate halves
  - invalid continuation bytes
  - code points outside Unicode range
- Match upstream error reporting details:
  - `source` labels such as `<string>`, `<buffer>`, `<file>`, and `<callback>`
  - character-based columns, not byte offsets
  - special cases like `"end of file expected near 'garbage'"` and `"NUL byte in object key not supported"`
- Implement all load entry points:
  - `json_loads`
  - `json_loadb`
  - `json_loadf`
  - `json_loadfd`
  - `json_load_file`
  - `json_load_callback`
- Implement all decode flags and preserve their interactions:
  - `JSON_REJECT_DUPLICATES`
  - `JSON_DISABLE_EOF_CHECK`
  - `JSON_DECODE_ANY`
  - `JSON_DECODE_INT_AS_REAL`
  - `JSON_ALLOW_NUL`
- Mitigate `CVE-2016-4425` with an explicit heap-based parse stack or an equally strong iterative structure. A recursive-descent parser with only best-effort depth tracking is not acceptable for the final state.
- Even with an iterative parser, preserve the public `JSON_PARSER_MAX_DEPTH` contract from `jansson_config.h`: inputs nested deeper than that limit must fail deterministically with the upstream-compatible stack-overflow error code, source label, and position bookkeeping.
- Preserve upstream numeric semantics from `strconv.c`:
  - overflow errors for unrepresentable integers/reals
  - underflow-to-zero behavior where upstream allows it
  - forcing a decimal/exponent in encoded real values
  - removing `+` and leading zeroes from exponents
  - honoring `JSON_REAL_PRECISION(n)`
- Keep `safe/src/raw/buf.rs` aligned with `original/jansson-2.14/src/strbuffer.c` so parser, dumper, and numeric-formatting paths share the same allocator-aware growth, truncation, and failure behavior.
- Preserve the appendix-defined ownership of the phase-4 parser/dumper files:
  - `safe/src/utf.rs` expands into the shared UTF-8 encode/validate/iterate helper layer matching upstream acceptance and rejection behavior.
  - `safe/src/strconv.rs` owns locale-independent `strtod` / `dtostr` behavior and exponent normalization matching upstream.
  - `safe/src/load.rs` owns the lexer, parser, load entry points, decode flags, source labels, and error-position tracking.
  - `safe/src/dump.rs` owns dump entry points, callback/file/fd sinks, encoding flags, sorted-key handling, circular-reference rejection, and `JSON_EMBED`.
  - Keep `safe/src/raw/table.rs` on the parser/dumper path for circular-reference tracking, visited sets, and other auxiliary sets rather than introducing separate ad-hoc hash tables.
  - `safe/scripts/run-data-suites.sh` is the owned mirrored-suite runner that builds/runs `safe/tests/upstream-bin/json_process.c` against `safe/tests/upstream-suites/**` using the mirrored `safe/tests/upstream-scripts/` helpers.
- Implement all dump entry points and flags:
  - `json_dumps`
  - `json_dumpb`
  - `json_dumpf`
  - `json_dumpfd`
  - `json_dump_file`
  - `json_dump_callback`
  - `JSON_INDENT`, `JSON_COMPACT`, `JSON_ENSURE_ASCII`, `JSON_SORT_KEYS`, `JSON_PRESERVE_ORDER`, `JSON_ENCODE_ANY`, `JSON_ESCAPE_SLASH`, `JSON_REAL_PRECISION(n)`, `JSON_EMBED`
- Preserve the `json_dumpb()` contract: write as much as fits into the caller buffer, but return the full encoded size even on truncation.
- Keep object parsing on the seeded-hash path from phase 3 so attacker-controlled objects do not regress `CVE-2013-6401`.
- Make `safe/scripts/run-data-suites.sh` build the mirrored `safe/tests/upstream-bin/json_process.c` and execute the mirrored suite corpus under `safe/tests/upstream-suites/` by default in build-tree mode, wiring `scriptdir` to the mirrored `safe/tests/upstream-scripts/` helpers instead of reaching back into `original/`. Phase 6 will extend this script with an explicit installed-package mode, but the mirrored-suite inputs must remain the default source of test data.
- Do not gate phase 4 on `test_dump.c`; it also exercises `json_pack()` and moves to phase 5 with the rest of the variadic-boundary coverage.
- Before yielding, commit all phase work to git with a message that begins with `impl_parse_dump:`.

## Verification Phases
### `check_parse_dump`
Phase ID: `check_parse_dump`
Type: `check`
Fixed `bounce_target`: `impl_parse_dump`
Purpose: Validate parsing, dump behavior that does not depend on `json_pack*`, UTF-8 acceptance/rejection, file/fd/callback I/O, numeric formatting, and the full upstream data-suite corpus.

Commands:
```sh
cargo build --manifest-path safe/Cargo.toml --release
safe/scripts/build-upstream-api-tests.sh test_load test_loadb test_load_callback test_dump_callback test_fixed_size test_copy test_equal
safe/scripts/run-upstream-api-tests.sh test_load test_loadb test_load_callback test_dump_callback test_fixed_size test_copy test_equal
safe/scripts/run-data-suites.sh valid invalid invalid-unicode encoding-flags
```

## Success Criteria
- The upstream load, callback-dump, fixed-size, copy, and equality API tests listed above compile and run successfully.
- All four data-suite families run through `json_process`.
- If parser or dumper changes touch shared traversal or string logic, the affected mirrored container tests are rerun successfully.

## Git Commit Requirement
The implementer must commit work to git before yielding, with a message that begins with `impl_parse_dump:`.
