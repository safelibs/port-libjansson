#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
safe_dir="$root/safe"
build_dir="$safe_dir/.build/checks"
mkdir -p "$build_dir"

cargo build --release --manifest-path "$safe_dir/Cargo.toml"

src="$build_dir/check_allocator_hooks.c"
exe="$build_dir/check_allocator_hooks"

cat >"$src" <<'EOF'
#include <jansson.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static size_t malloc_calls = 0;
static size_t free_calls = 0;

static void fail(const char *msg) {
    fprintf(stderr, "%s\n", msg);
    exit(1);
}

static void *tracking_malloc(size_t size) {
    void *ptr = malloc(size);
    if (ptr)
        malloc_calls++;
    return ptr;
}

static void tracking_free(void *ptr) {
    if (ptr)
        free_calls++;
    free(ptr);
}

int main(void) {
    json_malloc_t malloc_fn = NULL;
    json_free_t free_fn = NULL;
    json_t *utf8 = NULL;
    json_t *embedded_nul = NULL;
    json_t *invalid = NULL;
    json_t *integer = NULL;
    json_t *real = NULL;

    json_set_alloc_funcs(tracking_malloc, tracking_free);
    json_get_alloc_funcs(&malloc_fn, &free_fn);

    if (malloc_fn != tracking_malloc || free_fn != tracking_free)
        fail("allocator hook round-trip failed");

    utf8 = json_string("snowman \342\230\203");
    embedded_nul = json_stringn("hi\0ho", 5);
    invalid = json_string_nocheck("qu\377");
    integer = json_integer(42);
    real = json_real(3.25);

    if (!utf8 || !embedded_nul || !invalid || !integer || !real)
        fail("failed to allocate representative scalar values");

    if (malloc_calls < 7)
        fail("custom malloc hook did not observe scalar allocations");

    json_decref(utf8);
    json_decref(embedded_nul);
    json_decref(invalid);
    json_decref(integer);
    json_decref(real);

    if (free_calls < 7)
        fail("custom free hook did not observe scalar deallocation");

    json_set_alloc_funcs(malloc, free);
    return 0;
}
EOF

"${CC:-cc}" -std=c99 -Wall -Wextra -Werror \
    -I"$safe_dir/include" \
    "$src" \
    -L"$safe_dir/target/release" \
    -Wl,-rpath,"$safe_dir/target/release" \
    -ljansson \
    -o "$exe"

LD_LIBRARY_PATH="$safe_dir/target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" "$exe"
