#!/bin/sh
set -eu

usage() {
    echo "Usage: $0 --sync|--check" >&2
    exit 2
}

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
upstream_root="$root/original/jansson-2.14/test"
mirror_root="$root/safe/tests"

sync_dir() {
    src=$1
    dst=$2
    mkdir -p "$dst"
    rsync -a --delete "$src"/ "$dst"/
}

check_dir() {
    src=$1
    dst=$2
    [ -d "$dst" ] || {
        echo "missing mirror directory: $dst" >&2
        return 1
    }

    if [ -n "$(rsync -ain --delete "$src"/ "$dst"/)" ]; then
        echo "stale mirror directory: $dst" >&2
        return 1
    fi
}

sync_file() {
    src=$1
    dst=$2
    mkdir -p "$(dirname "$dst")"
    rsync -a "$src" "$dst"
}

check_file() {
    src=$1
    dst=$2
    [ -f "$dst" ] || {
        echo "missing mirror file: $dst" >&2
        return 1
    }

    if [ -n "$(rsync -ain "$src" "$dst")" ]; then
        echo "stale mirror file: $dst" >&2
        return 1
    fi
}

run_sync() {
    sync_dir "$upstream_root/suites/api" "$mirror_root/upstream-api"
    sync_dir "$upstream_root/bin" "$mirror_root/upstream-bin"
    sync_dir "$upstream_root/scripts" "$mirror_root/upstream-scripts"
    sync_dir "$upstream_root/suites/valid" "$mirror_root/upstream-suites/valid"
    sync_dir "$upstream_root/suites/invalid" "$mirror_root/upstream-suites/invalid"
    sync_dir "$upstream_root/suites/invalid-unicode" "$mirror_root/upstream-suites/invalid-unicode"
    sync_dir "$upstream_root/suites/encoding-flags" "$mirror_root/upstream-suites/encoding-flags"
    sync_file "$upstream_root/run-suites" "$mirror_root/run-suites"
}

run_check() {
    check_dir "$upstream_root/suites/api" "$mirror_root/upstream-api"
    check_dir "$upstream_root/bin" "$mirror_root/upstream-bin"
    check_dir "$upstream_root/scripts" "$mirror_root/upstream-scripts"
    check_dir "$upstream_root/suites/valid" "$mirror_root/upstream-suites/valid"
    check_dir "$upstream_root/suites/invalid" "$mirror_root/upstream-suites/invalid"
    check_dir "$upstream_root/suites/invalid-unicode" "$mirror_root/upstream-suites/invalid-unicode"
    check_dir "$upstream_root/suites/encoding-flags" "$mirror_root/upstream-suites/encoding-flags"
    check_file "$upstream_root/run-suites" "$mirror_root/run-suites"
}

[ $# -eq 1 ] || usage

case "$1" in
    --sync)
        run_sync
        ;;
    --check)
        run_check
        ;;
    *)
        usage
        ;;
esac
