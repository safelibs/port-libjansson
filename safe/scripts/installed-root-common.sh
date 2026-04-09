#!/bin/sh

dpkg_cfg_root() {
    dpkg_cfg=${HOME}/.dpkg.cfg

    [ -f "$dpkg_cfg" ] || return 1
    awk -F= '/^root=/{print $2}' "$dpkg_cfg" | tail -n 1
}

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

    if [ "$requested_root" = "/" ]; then
        cfg_root=$(dpkg_cfg_root || true)
        if [ -n "$cfg_root" ] && [ -d "$cfg_root/usr" ]; then
            printf '%s\n' "$cfg_root"
            return 0
        fi
    fi

    printf '%s\n' "$requested_root"
}
