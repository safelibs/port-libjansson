#!/bin/sh
set -eu

usage() {
    echo "Usage: $0 [--names-only] [--lib PATH]" >&2
    exit 2
}

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
def_file="$root/original/jansson-2.14/src/jansson.def"
lib_file="$root/safe/target/release/libjansson.so"

while [ $# -gt 0 ]; do
    case "$1" in
        --names-only)
            shift
            ;;
        --lib)
            [ $# -ge 2 ] || usage
            lib_file=$2
            shift 2
            ;;
        *)
            usage
            ;;
    esac
done

expected=$(mktemp)
actual=$(mktemp)
raw=$(mktemp)
trap 'rm -f "$expected" "$actual" "$raw"' EXIT

grep 'json_\|jansson_' "$def_file" | sed 's/ //g' | sort >"$expected"
nm -D --defined-only "$lib_file" >"$raw" 2>/dev/null || exit 77
awk '{print $3}' "$raw" | grep -E '^(json_|jansson_)' | sed 's/@@libjansson.*//' | sort >"$actual"

if ! cmp -s "$expected" "$actual"; then
    diff -u "$expected" "$actual" >&2
    exit 1
fi
