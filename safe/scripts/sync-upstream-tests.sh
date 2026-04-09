#!/bin/sh
set -eu

usage() {
    echo "Usage: $0 --sync|--check" >&2
    exit 2
}

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
upstream_root="$root/original/jansson-2.14/test"
mirror_root="$root/safe/tests"
mirror_entries='
dir:suites/api:upstream-api
dir:bin:upstream-bin
dir:scripts:upstream-scripts
dir:suites/valid:upstream-suites/valid
dir:suites/invalid:upstream-suites/invalid
dir:suites/invalid-unicode:upstream-suites/invalid-unicode
dir:suites/encoding-flags:upstream-suites/encoding-flags
file:run-suites:run-suites
'

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
    printf '%s\n' "$mirror_entries" | while IFS=: read -r kind src dst; do
        [ -n "$kind" ] || continue
        case "$kind" in
            dir)
                sync_dir "$upstream_root/$src" "$mirror_root/$dst"
                ;;
            file)
                sync_file "$upstream_root/$src" "$mirror_root/$dst"
                ;;
            *)
                echo "unknown mirror entry kind: $kind" >&2
                exit 1
                ;;
        esac
    done
}

run_check() {
    printf '%s\n' "$mirror_entries" | while IFS=: read -r kind src dst; do
        [ -n "$kind" ] || continue
        case "$kind" in
            dir)
                check_dir "$upstream_root/$src" "$mirror_root/$dst"
                ;;
            file)
                check_file "$upstream_root/$src" "$mirror_root/$dst"
                ;;
            *)
                echo "unknown mirror entry kind: $kind" >&2
                exit 1
                ;;
        esac
    done
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
