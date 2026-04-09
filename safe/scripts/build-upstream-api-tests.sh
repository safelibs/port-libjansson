#!/bin/sh
set -eu

usage() {
    echo "Usage: $0 [--all] [--installed-dev] [--tests-dir DIR] [test_name ...]" >&2
    exit 2
}

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
safe_dir="$root/safe"
mode="build-tree"
tests_dir="$safe_dir/tests/upstream-api"
build_all=0

while [ $# -gt 0 ]; do
    case "$1" in
        --all)
            build_all=1
            shift
            ;;
        --installed-dev)
            mode="installed-dev"
            shift
            ;;
        --tests-dir)
            [ $# -ge 2 ] || usage
            tests_dir=$2
            shift 2
            ;;
        --help)
            usage
            ;;
        --)
            shift
            break
            ;;
        -*)
            usage
            ;;
        *)
            break
            ;;
    esac
done

normalize_name() {
    name=$1
    name=${name##*/}
    case "$name" in
        *.c) name=${name%.c} ;;
    esac
    printf '%s\n' "$name"
}

if [ "$build_all" -eq 1 ]; then
    set --
    for test_src in "$tests_dir"/test_*.c; do
        case "$(basename "$test_src")" in
            test_dump.c)
                continue
                ;;
        esac
        set -- "$@" "$test_src"
    done
fi

[ $# -gt 0 ] || usage

build_dir="$safe_dir/.build/api-tests/$mode"
manifest="$build_dir/manifest.txt"
mkdir -p "$build_dir"
: >"$manifest"

cc_bin=${CC:-cc}
runtime_dir="$build_dir/runtime-lib"

verify_safe_linkage() {
    exe=$1
    expected=$(readlink -f "$safe_dir/target/release/libjansson.so")
    actual=$(ldd "$exe" | awk '/libjansson\.so\.4/ { print $3; exit }')

    [ -n "$actual" ] || {
        echo "failed to resolve libjansson.so.4 for $exe" >&2
        exit 1
    }

    actual=$(readlink -f "$actual")
    [ "$actual" = "$expected" ] || {
        echo "expected $exe to use $expected but it resolved to $actual" >&2
        exit 1
    }
}

case "$mode" in
    build-tree)
        cargo build --release --manifest-path "$safe_dir/Cargo.toml"
        mkdir -p "$runtime_dir"
        ln -sfn "$safe_dir/target/release/libjansson.so" "$runtime_dir/libjansson.so.4"
        include_flags="-I$safe_dir/include"
        link_flags="-L$safe_dir/target/release -Wl,-rpath,$runtime_dir -ljansson"
        ;;
    installed-dev)
        include_flags=$(pkg-config --cflags jansson)
        link_flags=$(pkg-config --libs jansson)
        ;;
    *)
        usage
        ;;
esac

for test_name in "$@"; do
    base=$(normalize_name "$test_name")
    src="$tests_dir/$base.c"
    exe="$build_dir/$base"

    [ -f "$src" ] || {
        echo "missing test source: $src" >&2
        exit 1
    }

    "$cc_bin" -std=c99 -Wall -Wextra -Werror $include_flags "$src" -o "$exe" $link_flags

    if [ "$mode" = "build-tree" ]; then
        verify_safe_linkage "$exe"
    fi

    printf '%s\n' "$base" >>"$manifest"
done

printf '%s\n' "$manifest"
