#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
safe_dir="$root/safe"
build_dir="$safe_dir/.build/container-primitives"
runtime_dir="$build_dir/runtime-lib"
src="$build_dir/container_primitives.c"
exe="$build_dir/container_primitives"

mkdir -p "$build_dir" "$runtime_dir"
ln -sfn "$safe_dir/target/release/libjansson.so" "$runtime_dir/libjansson.so.4"

cargo test --manifest-path "$safe_dir/Cargo.toml" --release container_seed_contract --lib

cat >"$src" <<'EOF'
#include <jansson.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define check(cond, msg)                                                                \
    do {                                                                                \
        if (!(cond)) {                                                                  \
            fprintf(stderr, "%s:%d: %s\n", __FILE__, __LINE__, msg);                    \
            exit(1);                                                                    \
        }                                                                               \
    } while (0)

static void test_iteration_and_roundtrip(void) {
    json_t *object = json_object();
    const char *expected_keys[] = {"alpha", "beta", "gamma"};
    const int expected_values[] = {1, 2, 3};
    size_t index = 0;
    void *iter;

    check(object != NULL, "json_object failed");
    check(json_object_set_new(object, "alpha", json_integer(1)) == 0, "set alpha failed");
    check(json_object_set_new(object, "beta", json_integer(2)) == 0, "set beta failed");
    check(json_object_set_new(object, "gamma", json_integer(3)) == 0, "set gamma failed");
    check(json_object_size(object) == 3, "unexpected object size");

    iter = json_object_iter(object);
    check(iter != NULL, "json_object_iter failed");

    while (iter) {
        const char *key = json_object_iter_key(iter);
        json_t *value = json_object_iter_value(iter);

        check(key != NULL, "json_object_iter_key returned NULL");
        check(json_object_key_to_iter(key) == iter, "key-to-iter roundtrip failed");
        check(strcmp(key, expected_keys[index]) == 0, "iteration order mismatch");
        check(json_object_iter_key_len(iter) == strlen(expected_keys[index]),
              "unexpected key length");
        check(json_is_integer(value), "unexpected iterator value type");
        check(json_integer_value(value) == expected_values[index], "unexpected iterator value");

        if (index == 1)
            check(json_object_iter_at(object, "beta") == iter, "json_object_iter_at failed");

        iter = json_object_iter_next(object, iter);
        index++;
    }

    check(index == 3, "iterator count mismatch");
    json_decref(object);
}

static void test_fixed_keys(void) {
    json_t *object = json_object();
    json_t *shared = json_string("shared");
    const char key1[] = {'u', 't', 'f', '8'};
    const char key2[] = {'b', 'i', 'n', '\0', 'x'};
    const char key3[] = {'t', 'a', 'i', 'l', '\0'};
    const char *expected_keys[] = {key1, key2, key3};
    const size_t expected_lengths[] = {sizeof(key1), sizeof(key2), sizeof(key3)};
    const char *key;
    size_t key_len;
    json_t *value;
    size_t index = 0;

    check(object != NULL, "json_object failed for fixed keys");
    check(shared != NULL, "json_string failed for shared value");

    check(json_object_setn(object, key1, sizeof(key1), shared) == 0, "json_object_setn failed");
    json_decref(shared);
    check(json_is_string(json_object_getn(object, key1, sizeof(key1))),
          "json_object_setn did not preserve its value");

    check(json_object_setn_new_nocheck(object, key2, sizeof(key2), json_true()) == 0,
          "json_object_setn_new_nocheck failed for binary key");
    check(json_object_setn_new_nocheck(object, key3, sizeof(key3), json_false()) == 0,
          "json_object_setn_new_nocheck failed for trailing NUL key");

    check(json_object_get(object, "utf8") == json_object_getn(object, key1, sizeof(key1)),
          "cstring lookup failed for plain utf8 key");
    check(json_object_get(object, "bin") == NULL, "cstring lookup ignored embedded NUL");
    check(json_object_getn(object, key2, sizeof(key2)) == json_true(),
          "binary key lookup failed");
    check(json_object_getn(object, key3, sizeof(key3)) == json_false(),
          "trailing NUL key lookup failed");

    json_object_keylen_foreach(object, key, key_len, value) {
        check(index < 3, "iterated too many fixed keys");
        check(key_len == expected_lengths[index], "iterator key length mismatch");
        check(memcmp(key, expected_keys[index], key_len) == 0, "iterator key bytes mismatch");
        check(json_object_key_to_iter(key) != NULL, "key-to-iter failed for fixed key");
        index++;
    }

    check(index == 3, "did not iterate all fixed keys");
    check(json_object_deln(object, key2, sizeof(key2)) == 0, "json_object_deln failed");
    check(json_object_getn(object, key2, sizeof(key2)) == NULL, "binary key still present");

    json_decref(object);
}

static void test_copy_equal_and_updates(void) {
    json_t *nested = json_object();
    json_t *array = json_array();
    json_t *array_copy;
    json_t *array_deep;
    json_t *object = json_object();
    json_t *object_copy;
    json_t *object_deep;
    json_t *different_order = json_object();
    json_t *dst = json_object();
    json_t *dst_child = json_object();
    json_t *src = json_object();
    json_t *src_child = json_object();
    json_t *cyclic = json_object();
    json_t *cyclic_child = json_object();
    void *iter;

    check(nested && array && object && different_order && dst && dst_child && src && src_child &&
              cyclic && cyclic_child,
          "failed to allocate test containers");

    check(json_object_set_new(nested, "n", json_integer(7)) == 0, "nested set failed");
    check(json_array_append_new(array, json_integer(1)) == 0, "array append int failed");
    check(json_array_append(array, nested) == 0, "array append object failed");
    check(json_array_append_new(array, json_true()) == 0, "array append bool failed");

    array_copy = json_copy(array);
    array_deep = json_deep_copy(array);
    check(array_copy && array_deep, "array copy failed");
    check(json_equal(array, array_copy), "shallow array copy is not equal");
    check(json_equal(array, array_deep), "deep array copy is not equal");
    check(json_array_get(array, 1) == json_array_get(array_copy, 1),
          "shallow array copy changed child identity");
    check(json_array_get(array, 1) != json_array_get(array_deep, 1),
          "deep array copy preserved child identity");

    check(json_object_set_new(object, "first", json_integer(1)) == 0, "object set first failed");
    check(json_object_set(object, "nested", nested) == 0, "object set nested failed");
    check(json_object_set(object, "array", array) == 0, "object set array failed");

    object_copy = json_copy(object);
    object_deep = json_deep_copy(object);
    check(object_copy && object_deep, "object copy failed");
    check(json_equal(object, object_copy), "shallow object copy is not equal");
    check(json_equal(object, object_deep), "deep object copy is not equal");
    check(json_object_get(object, "nested") == json_object_get(object_copy, "nested"),
          "shallow object copy changed child identity");
    check(json_object_get(object, "nested") != json_object_get(object_deep, "nested"),
          "deep object copy preserved child identity");

    check(json_object_set(different_order, "array", json_object_get(object, "array")) == 0,
          "different_order set array failed");
    check(json_object_set(different_order, "first", json_object_get(object, "first")) == 0,
          "different_order set first failed");
    check(json_object_set(different_order, "nested", json_object_get(object, "nested")) == 0,
          "different_order set nested failed");
    check(json_equal(object, different_order), "object equality depends on insertion order");

    check(json_object_set_new(dst_child, "value", json_integer(1)) == 0, "dst child init failed");
    check(json_object_set_new(dst, "child", dst_child) == 0, "dst set child failed");
    check(json_object_set_new(src_child, "value", json_integer(2)) == 0, "src child value failed");
    check(json_object_set_new(src_child, "extra", json_integer(3)) == 0, "src child extra failed");
    check(json_object_set_new(src, "child", src_child) == 0, "src set child failed");
    check(json_object_update_recursive(dst, src) == 0, "json_object_update_recursive failed");
    check(json_object_get(dst, "child") == dst_child,
          "recursive update did not preserve nested object identity");
    check(json_integer_value(json_object_get(dst_child, "value")) == 2,
          "recursive update did not overwrite nested key");
    check(json_integer_value(json_object_get(dst_child, "extra")) == 3,
          "recursive update did not merge nested key");

    check(json_object_set_new(cyclic_child, "back", json_null()) == 0, "cyclic child init failed");
    iter = json_object_iter_at(cyclic_child, "back");
    check(iter != NULL, "json_object_iter_at failed for cyclic object");
    check(json_object_set_new(cyclic, "child", cyclic_child) == 0, "cyclic set child failed");
    check(json_object_iter_set(cyclic_child, iter, cyclic) == 0, "json_object_iter_set failed");
    check(json_deep_copy(cyclic) == NULL, "json_deep_copy accepted a cycle");
    check(json_object_iter_set_new(cyclic_child, iter, json_null()) == 0,
          "failed to break cycle");

    json_decref(cyclic);
    json_decref(src);
    json_decref(dst);
    json_decref(different_order);
    json_decref(object_deep);
    json_decref(object_copy);
    json_decref(object);
    json_decref(array_deep);
    json_decref(array_copy);
    json_decref(array);
    json_decref(nested);
}

static void test_binary_key_foreach_compat(void) {
    const char binary_key[] = {'b', '\0', 'x'};
    json_t *binary_only_a = json_object();
    json_t *binary_only_b = json_object();
    json_t *plain = json_object();
    json_t *copy;
    json_t *deep;
    json_t *update_dst = json_object();
    json_t *update_missing_dst = json_object();

    check(binary_only_a && binary_only_b && plain && update_dst && update_missing_dst,
          "failed to allocate binary-key compat objects");

    check(json_object_setn_new_nocheck(binary_only_a, binary_key, sizeof(binary_key), json_true()) == 0,
          "binary_only_a init failed");
    check(json_object_setn_new_nocheck(binary_only_b, binary_key, sizeof(binary_key), json_true()) == 0,
          "binary_only_b init failed");
    check(json_object_set_new(plain, "b", json_true()) == 0, "plain init failed");

    copy = json_copy(binary_only_a);
    deep = json_deep_copy(binary_only_a);
    check(copy && deep, "binary-key copy/deep-copy failed");
    check(json_object_get(copy, "b") == json_true(), "json_copy did not use cstring key semantics");
    check(json_object_getn(copy, binary_key, sizeof(binary_key)) == NULL,
          "json_copy preserved full binary key unexpectedly");
    check(json_object_get(deep, "b") == json_true(),
          "json_deep_copy did not use cstring key semantics");
    check(json_object_getn(deep, binary_key, sizeof(binary_key)) == NULL,
          "json_deep_copy preserved full binary key unexpectedly");

    check(!json_equal(binary_only_a, binary_only_b),
          "json_equal unexpectedly preserved full binary key length");
    check(json_equal(binary_only_a, plain),
          "json_equal should compare via cstring iterator keys");

    check(json_object_update(update_dst, binary_only_a) == 0, "json_object_update failed");
    check(json_object_get(update_dst, "b") == json_true(),
          "json_object_update did not use cstring key semantics");
    check(json_object_getn(update_dst, binary_key, sizeof(binary_key)) == NULL,
          "json_object_update preserved full binary key unexpectedly");

    check(json_object_update_missing(update_missing_dst, binary_only_a) == 0,
          "json_object_update_missing failed");
    check(json_object_get(update_missing_dst, "b") == json_true(),
          "json_object_update_missing did not use cstring key semantics");
    check(json_object_getn(update_missing_dst, binary_key, sizeof(binary_key)) == NULL,
          "json_object_update_missing preserved full binary key unexpectedly");

    json_decref(update_missing_dst);
    json_decref(update_dst);
    json_decref(deep);
    json_decref(copy);
    json_decref(plain);
    json_decref(binary_only_b);
    json_decref(binary_only_a);
}

int main(void) {
    test_iteration_and_roundtrip();
    test_fixed_keys();
    test_copy_equal_and_updates();
    test_binary_key_foreach_compat();
    return 0;
}
EOF

cc_bin=${CC:-cc}
"$cc_bin" -std=c99 -Wall -Wextra -Werror -I"$safe_dir/include" \
    "$src" -o "$exe" \
    -L"$safe_dir/target/release" -Wl,-rpath,"$runtime_dir" -ljansson

LD_LIBRARY_PATH="$runtime_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" "$exe"
