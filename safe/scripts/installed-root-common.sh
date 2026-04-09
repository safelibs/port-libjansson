#!/bin/sh

resolve_installed_root() {
    repo_root=$1
    requested_root=$2

    case "$requested_root" in
        "")
            printf '\n'
            return 0
            ;;
        /*)
            ;;
        *)
            requested_root=$repo_root/$requested_root
            ;;
    esac

    printf '%s\n' "$requested_root"
}
