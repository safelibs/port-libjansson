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

build_script="$safe_dir/scripts/build-upstream-api-tests.sh"
build_dir="$safe_dir/.build/api-tests/$mode"
manifest="$build_dir/manifest.txt"
runtime_dir="$build_dir/runtime-lib"

run_build() {
    if [ "$build_all" -eq 1 ]; then
        if [ "$mode" = "installed-dev" ]; then
            "$build_script" --all --installed-dev --tests-dir "$tests_dir" "$@"
        else
            "$build_script" --all --tests-dir "$tests_dir" "$@"
        fi
    else
        if [ "$mode" = "installed-dev" ]; then
            "$build_script" --installed-dev --tests-dir "$tests_dir" "$@"
        else
            "$build_script" --tests-dir "$tests_dir" "$@"
        fi
    fi
}

if [ "$build_all" -eq 1 ] || [ $# -gt 0 ] || [ ! -f "$manifest" ]; then
    run_build "$@"
fi

[ -f "$manifest" ] || usage

if [ $# -eq 0 ]; then
    set --
    while IFS= read -r test_name; do
        set -- "$@" "$test_name"
    done <"$manifest"
fi

lib_path="$safe_dir/target/release"

for test_name in "$@"; do
    case "$test_name" in
        *.c) test_name=${test_name%.c} ;;
    esac

    exe="$build_dir/${test_name##*/}"
    [ -x "$exe" ] || {
        run_build "$test_name"
    }

    if [ "$mode" = "build-tree" ]; then
        [ -d "$runtime_dir" ] || run_build "$test_name"
        LD_LIBRARY_PATH="$runtime_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" "$exe"
    else
        "$exe"
    fi
done
