#!/bin/sh
set -eu

usage() {
    echo "Usage: $0 [--all] [--installed-dev --installed-root ROOT] [--tests-dir DIR] [test_name ...]" >&2
    exit 2
}

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
safe_dir="$root/safe"
mode="build-tree"
installed_root=
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
        --installed-root)
            [ $# -ge 2 ] || usage
            installed_root=$2
            shift 2
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

case "$installed_root" in
    "")
        ;;
    /*)
        ;;
    *)
        installed_root=$root/$installed_root
        ;;
esac

if [ -n "$installed_root" ] && [ "$mode" != "installed-dev" ]; then
    echo "--installed-root requires --installed-dev" >&2
    exit 2
fi

if [ "$mode" = "installed-dev" ] && [ -z "$installed_root" ]; then
    installed_root=/
fi

sanitize_root_tag() {
    printf '%s' "$1" | sed 's#[^A-Za-z0-9_.-]#_#g'
}

clean_env() {
    env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH "$@"
}

build_script="$safe_dir/scripts/build-upstream-api-tests.sh"
build_tag=$mode
if [ "$mode" = "installed-dev" ]; then
    build_tag=$mode-$(sanitize_root_tag "$installed_root")
fi
build_dir="$safe_dir/.build/api-tests/$build_tag"
manifest="$build_dir/manifest.txt"
runtime_dir="$build_dir/runtime-lib"

run_build() {
    if [ "$build_all" -eq 1 ]; then
        set -- --all "$@"
    fi

    if [ "$mode" = "installed-dev" ]; then
        "$build_script" --installed-dev --installed-root "$installed_root" --tests-dir "$tests_dir" "$@"
    else
        "$build_script" --tests-dir "$tests_dir" "$@"
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
        env LD_LIBRARY_PATH="$runtime_dir" "$exe"
    else
        clean_env "$exe"
    fi
done
