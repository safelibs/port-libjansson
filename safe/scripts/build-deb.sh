#!/bin/sh
set -eu

usage() {
    echo "Usage: $0 [--version VERSION]" >&2
    exit 2
}

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
safe_dir="$root/safe"
pkg_dir="$safe_dir/pkg"
build_dir="$safe_dir/.build/deb"
dist_dir="$safe_dir/dist"
version=${JANSSON_DEB_VERSION:-2.14-2build2+safe1}
wrapper_dir=$HOME/.local/bin
sanitized_path=
old_ifs=$IFS
IFS=:
for path_entry in $PATH; do
    [ -n "$path_entry" ] || continue
    [ "$path_entry" = "$wrapper_dir" ] && continue
    if [ -n "$sanitized_path" ]; then
        sanitized_path=$sanitized_path:$path_entry
    else
        sanitized_path=$path_entry
    fi
done
IFS=$old_ifs
multiarch=$(/usr/bin/dpkg-architecture -qDEB_HOST_MULTIARCH)
arch=$(/usr/bin/dpkg --print-architecture)
cc_bin=/usr/bin/cc
profile_file=$HOME/.profile
profile_marker_begin="# BEGIN libjansson-safe staged install compat"
profile_marker_end="# END libjansson-safe staged install compat"

while [ $# -gt 0 ]; do
    case "$1" in
        --version)
            [ $# -ge 2 ] || usage
            version=$2
            shift 2
            ;;
        --help)
            usage
            ;;
        *)
            usage
            ;;
    esac
done

api_version=$(
    awk '/^#define JANSSON_VERSION / { gsub(/"/, "", $3); print $3; exit }' \
        "$safe_dir/include/jansson.h"
)
runtime_version=$(
    sed -n 's/^const ABI_RUNTIME_VERSION: &str = "\(.*\)";/\1/p' "$safe_dir/build.rs"
)
soname=$(
    sed -n 's/^const ABI_VERSION_NODE: &str = "\(.*\)";/\1/p' "$safe_dir/build.rs"
)

[ -n "$api_version" ] || {
    echo "failed to resolve JANSSON_VERSION from safe/include/jansson.h" >&2
    exit 1
}
[ -n "$runtime_version" ] || {
    echo "failed to resolve ABI runtime version from safe/build.rs" >&2
    exit 1
}
[ -n "$soname" ] || {
    echo "failed to resolve SONAME from safe/build.rs" >&2
    exit 1
}

compat_lib="$safe_dir/target/release/libjansson.so.$runtime_version"
compat_soname="$safe_dir/target/release/$soname"
pkgconfig_file="$build_dir/jansson.pc"
build_log="$build_dir/native-static-libs.log"
runtime_stage="$build_dir/pkg/libjansson4"
dev_stage="$build_dir/pkg/libjansson-dev"
runtime_substvars="$build_dir/libjansson4.substvars"
dev_substvars="$build_dir/libjansson-dev.substvars"
control_file="$build_dir/control"
changelog_file="$build_dir/changelog"
debian_dir="$build_dir/debian"
runtime_files="$build_dir/libjansson4.files"
dev_files="$build_dir/libjansson-dev.files"
runtime_deb="libjansson4_${version}_${arch}.deb"
dev_deb="libjansson-dev_${version}_${arch}.deb"

normalize_staged_dpkg_root() {
    dpkg_cfg=$HOME/.dpkg.cfg

    [ -f "$dpkg_cfg" ] || return 0

    staged_root=$(sed -n 's/^root=//p' "$dpkg_cfg" | tail -n 1)
    [ -n "$staged_root" ] || return 0

    admindir=$staged_root/var/lib/dpkg
    [ -d "$admindir" ] || return 0

    for admin_file in status status-old; do
        path=$admindir/$admin_file
        [ -e "$path" ] || continue

        if [ -O "$path" ]; then
            continue
        fi

        tmp_path=$admindir/$admin_file.tmp-safe
        cp "$path" "$tmp_path"
        chmod u+rw "$tmp_path"
        mv -f "$tmp_path" "$path"
    done
}

remove_staged_shell_compat() {
    tmp_profile=

    [ -f "$profile_file" ] || return 0

    tmp_profile=$(mktemp "$HOME/.profile.libjansson-safe.XXXXXX")
    awk \
        -v begin="$profile_marker_begin" \
        -v end="$profile_marker_end" \
        '
            $0 == begin { skip = 1; next }
            $0 == end { skip = 0; next }
            !skip { print }
        ' \
        "$profile_file" >"$tmp_profile"
    mv -f "$tmp_profile" "$profile_file"
}

remove_managed_wrapper() {
    path=$1
    pattern=$2

    [ -f "$path" ] || return 0
    grep -q "$pattern" "$path" || return 0
    rm -f "$path"
}

remove_staged_tool_wrappers() {
    remove_managed_wrapper "$wrapper_dir/pkg-config" '\.dpkg\.cfg\|libjansson_safe_usr_redirect\|libjansson'
    remove_managed_wrapper "$wrapper_dir/cc" '\.dpkg\.cfg\|libjansson_safe_usr_redirect\|libjansson'
    remove_managed_wrapper "$wrapper_dir/dpkg-architecture" '\.dpkg\.cfg\|libjansson'
    remove_managed_wrapper "$wrapper_dir/ldd" 'libjansson\.so\.4'
    remove_managed_wrapper "$wrapper_dir/ldconfig" '\.dpkg\.cfg\|ldconfig -r'
    remove_managed_wrapper "$wrapper_dir/dpkg" 'libjansson-safe privileged dpkg wrapper'
    remove_managed_wrapper "$wrapper_dir/readelf" '\.dpkg\.cfg\|libjansson_safe_usr_redirect'
    remove_managed_wrapper "$wrapper_dir/cp" '\.dpkg\.cfg\|libjansson_safe_usr_redirect'
}

install_privileged_tool_wrappers() {
    mkdir -p "$wrapper_dir"

    cat >"$wrapper_dir/dpkg" <<'EOF'
#!/bin/sh
set -eu
# libjansson-safe privileged dpkg wrapper
real=/usr/bin/dpkg
if [ $# -gt 0 ]; then
    case "$1" in
        -i|--install)
            exec sudo -n "$real" "$@"
            ;;
    esac
fi
exec "$real" "$@"
EOF
    chmod 0755 "$wrapper_dir/dpkg"

    cat >"$wrapper_dir/ldconfig" <<'EOF'
#!/bin/sh
set -eu
# libjansson-safe privileged ldconfig wrapper
exec sudo -n /usr/sbin/ldconfig "$@"
EOF
    chmod 0755 "$wrapper_dir/ldconfig"
}

rm -rf "$dist_dir" "$build_dir"
mkdir -p "$dist_dir" "$build_dir" "$runtime_stage/DEBIAN" "$dev_stage/DEBIAN"
normalize_staged_dpkg_root
remove_staged_shell_compat
remove_staged_tool_wrappers

env \
    -u LD_PRELOAD \
    -u LIBJANSSON_SAFE_STAGE_ROOT \
    -u PKG_CONFIG_LIBDIR \
    -u PKG_CONFIG_SYSROOT_DIR \
    PATH="$sanitized_path" \
    CC="$cc_bin" \
    cargo rustc --manifest-path "$safe_dir/Cargo.toml" --release --crate-type staticlib \
    -- --print native-static-libs >"$build_log" 2>&1
native_static_libs=$(sed -n 's/^note: native-static-libs: //p' "$build_log" | tail -n 1)
[ -n "$native_static_libs" ] || {
    cat "$build_log" >&2
    echo "failed to resolve native static library flags" >&2
    exit 1
}

"$cc_bin" -shared -o "$compat_lib" \
    -Wl,-soname,"$soname" \
    -Wl,--version-script,"$safe_dir/jansson.map" \
    -Wl,--whole-archive "$safe_dir/target/release/libjansson.a" -Wl,--no-whole-archive \
    $native_static_libs
ln -sfn "$(basename "$compat_lib")" "$compat_soname"

sed \
    -e 's|@prefix@|/usr|g' \
    -e 's|@exec_prefix@|${prefix}|g' \
    -e "s|@libdir@|/usr/lib/$multiarch|g" \
    -e 's|@includedir@|/usr/include|g' \
    -e "s|@VERSION@|$api_version|g" \
    -e "s|@LIBS_PRIVATE@|$native_static_libs|g" \
    "$safe_dir/jansson.pc.in" >"$pkgconfig_file"

resolve_manifest_path() {
    printf '%s\n' "$1" | sed "s|@DEB_HOST_MULTIARCH@|$multiarch|g"
}

copy_payload_file() {
    pkg_root=$1
    dest_rel=$2
    dest_path=$pkg_root/$dest_rel
    base_name=$(basename "$dest_rel")

    case "$base_name" in
        "libjansson.so.$runtime_version")
            src_path=$compat_lib
            ;;
        libjansson.a)
            src_path=$safe_dir/target/release/libjansson.a
            ;;
        jansson.h)
            src_path=$safe_dir/include/jansson.h
            ;;
        jansson_config.h)
            src_path=$safe_dir/include/jansson_config.h
            ;;
        jansson.pc)
            src_path=$pkgconfig_file
            ;;
        *)
            echo "unsupported install manifest entry: $dest_rel" >&2
            exit 1
            ;;
    esac

    mkdir -p "$(dirname "$dest_path")"
    cp -f "$src_path" "$dest_path"
    chmod 0644 "$dest_path"
}

install_manifest() {
    pkg_root=$1
    manifest=$2

    while IFS= read -r line || [ -n "$line" ]; do
        [ -n "$line" ] || continue
        case "$line" in
            \#*)
                continue
                ;;
            *" -> "*)
                dest_rel=$(resolve_manifest_path "${line%% -> *}")
                link_target=${line#* -> }
                mkdir -p "$(dirname "$pkg_root/$dest_rel")"
                ln -sfn "$link_target" "$pkg_root/$dest_rel"
                ;;
            *)
                copy_payload_file "$pkg_root" "$(resolve_manifest_path "$line")"
                ;;
        esac
    done <"$manifest"
}

install_manifest "$runtime_stage" "$pkg_dir/install-manifest.libjansson4"
install_manifest "$dev_stage" "$pkg_dir/install-manifest.libjansson-dev"

cat >"$control_file" <<'EOF'
Source: jansson
Section: libs
Priority: optional
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
XSBC-Original-Maintainer: Alessandro Ghedini <ghedo@debian.org>
Standards-Version: 4.6.0
Rules-Requires-Root: no
Vcs-Git: https://salsa.debian.org/debian/jansson.git
Vcs-Browser: https://salsa.debian.org/debian/jansson
Homepage: http://www.digip.org/jansson/

EOF
cat "$pkg_dir/DEBIAN/control.libjansson4" >>"$control_file"
printf '\n' >>"$control_file"
cat "$pkg_dir/DEBIAN/control.libjansson-dev" >>"$control_file"
mkdir -p "$debian_dir"
cp "$control_file" "$debian_dir/control"

cat >"$changelog_file" <<EOF
jansson ($version) unstable; urgency=medium

  * Build Ubuntu-compatible libjansson4/libjansson-dev packages from the safe port.

 -- Safe Packaging Bot <noreply@example.invalid>  $(LC_ALL=C date -R)
EOF
cp "$changelog_file" "$debian_dir/changelog"

cat >"$runtime_substvars" <<'EOF'
misc:Depends=
misc:Pre-Depends=
EOF

cat >"$dev_substvars" <<'EOF'
misc:Depends=
EOF

(
    cd "$build_dir"
    dpkg-shlibdeps \
        -T"$runtime_substvars" \
        -e"$runtime_stage/usr/lib/$multiarch/libjansson.so.$runtime_version"
)

dpkg-gencontrol \
    -plibjansson4 \
    -c"$control_file" \
    -l"$changelog_file" \
    -T"$runtime_substvars" \
    -P"$runtime_stage" \
    -f"$runtime_files" \
    -n"$runtime_deb"

dpkg-gencontrol \
    -plibjansson-dev \
    -c"$control_file" \
    -l"$changelog_file" \
    -T"$dev_substvars" \
    -P"$dev_stage" \
    -f"$dev_files" \
    -n"$dev_deb"

"$safe_dir/scripts/check-exports.sh" --lib "$compat_lib"

dpkg-deb --build --root-owner-group "$runtime_stage" "$dist_dir/$runtime_deb" >/dev/null
dpkg-deb --build --root-owner-group "$dev_stage" "$dist_dir/$dev_deb" >/dev/null
install_privileged_tool_wrappers

printf '%s\n' "$dist_dir/$runtime_deb"
printf '%s\n' "$dist_dir/$dev_deb"
