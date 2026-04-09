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
    set -- "$tests_dir"/test_*.c
fi

[ $# -gt 0 ] || usage

build_dir="$safe_dir/.build/api-tests/$mode"
manifest="$build_dir/manifest.txt"
mkdir -p "$build_dir"
: >"$manifest"

cc_bin=${CC:-cc}

case "$mode" in
    build-tree)
        cargo build --release --manifest-path "$safe_dir/Cargo.toml"
        include_flags="-I$safe_dir/include"
        link_flags="-L$safe_dir/target/release -Wl,-rpath,$safe_dir/target/release -ljansson"
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
    printf '%s\n' "$base" >>"$manifest"
done

printf '%s\n' "$manifest"
