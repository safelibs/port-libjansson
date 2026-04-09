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
multiarch=$(dpkg-architecture -qDEB_HOST_MULTIARCH)
runtime_version=$(
    sed -n 's/^const ABI_RUNTIME_VERSION: &str = "\(.*\)";/\1/p' "$safe_dir/build.rs"
)
soname=$(
    sed -n 's/^const ABI_VERSION_NODE: &str = "\(.*\)";/\1/p' "$safe_dir/build.rs"
)

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

[ -n "$runtime_version" ] || {
    echo "failed to resolve ABI runtime version from safe/build.rs" >&2
    exit 1
}
[ -n "$soname" ] || {
    echo "failed to resolve SONAME from safe/build.rs" >&2
    exit 1
}

normalize_name() {
    name=$1
    name=${name##*/}
    case "$name" in
        *.c) name=${name%.c} ;;
    esac
    printf '%s\n' "$name"
}

sanitize_root_tag() {
    printf '%s' "$1" | sed 's#[^A-Za-z0-9_.-]#_#g'
}

clean_env() {
    env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH "$@"
}

pkg_config_cmd() {
    pkgconfig_libdir=$installed_root/usr/lib/$multiarch/pkgconfig:$installed_root/usr/lib/pkgconfig:$installed_root/usr/share/pkgconfig
    env \
        -u PKG_CONFIG_PATH \
        -u LD_LIBRARY_PATH \
        -u LIBRARY_PATH \
        -u CPATH \
        -u C_INCLUDE_PATH \
        PKG_CONFIG_DIR= \
        PKG_CONFIG_LIBDIR="$pkgconfig_libdir" \
        PKG_CONFIG_SYSROOT_DIR="$installed_root" \
        "$@"
}

emit_build_tree_shared() {
    build_log=$safe_dir/.build/api-tests.native-static-libs.log

    cargo rustc --manifest-path "$safe_dir/Cargo.toml" --release --crate-type staticlib \
        -- --print native-static-libs >"$build_log" 2>&1
    native_static_libs=$(sed -n 's/^note: native-static-libs: //p' "$build_log" | tail -n 1)
    [ -n "$native_static_libs" ] || {
        cat "$build_log" >&2
        echo "failed to resolve native static library flags" >&2
        exit 1
    }

    "${CC:-cc}" -shared -o "$safe_dir/target/release/libjansson.so.$runtime_version" \
        -Wl,-soname,"$soname" \
        -Wl,--version-script,"$safe_dir/jansson.map" \
        -Wl,--whole-archive "$safe_dir/target/release/libjansson.a" -Wl,--no-whole-archive \
        $native_static_libs
    ln -sfn "libjansson.so.$runtime_version" "$safe_dir/target/release/$soname"
}

verify_linkage() {
    exe=$1
    expected_root=$2
    actual=$(clean_env ldd "$exe" | awk '/libjansson\.so\.4/ { print $3; exit }')

    [ -n "$actual" ] || {
        echo "failed to resolve libjansson.so.4 for $exe" >&2
        exit 1
    }

    actual=$(readlink -f "$actual")
    expected_root=$(readlink -f "$expected_root")

    case "$actual" in
        "$expected_root")
            ;;
        "$expected_root"/*)
            ;;
        *)
            echo "expected $exe to use $expected_root but it resolved to $actual" >&2
            exit 1
            ;;
    esac
}

if [ "$build_all" -eq 1 ]; then
    set --
    for test_src in "$tests_dir"/test_*.c; do
        [ -f "$test_src" ] || continue
        set -- "$@" "$test_src"
    done
fi

[ $# -gt 0 ] || usage

build_tag=$mode
if [ "$mode" = "installed-dev" ]; then
    build_tag=$mode-$(sanitize_root_tag "$installed_root")
fi

build_dir="$safe_dir/.build/api-tests/$build_tag"
manifest="$build_dir/manifest.txt"
runtime_dir="$build_dir/runtime-lib"
mkdir -p "$build_dir"
: >"$manifest"

cc_bin=${CC:-cc}

case "$mode" in
    build-tree)
        emit_build_tree_shared
        mkdir -p "$runtime_dir"
        ln -sfn "$safe_dir/target/release/libjansson.so.$runtime_version" "$runtime_dir/$soname"
        include_flags="-I$safe_dir/include"
        link_flags="-L$safe_dir/target/release -Wl,-rpath,$runtime_dir -ljansson"
        expected_link="$safe_dir/target/release/libjansson.so.$runtime_version"
        ;;
    installed-dev)
        installed_include=$installed_root/usr/include/jansson.h
        installed_libdir=$installed_root/usr/lib/$multiarch

        [ -f "$installed_include" ] || {
            echo "missing installed header: $installed_include" >&2
            exit 1
        }
        [ -f "$installed_libdir/libjansson.so" ] || {
            echo "missing installed development symlink: $installed_libdir/libjansson.so" >&2
            exit 1
        }

        include_flags=$(pkg_config_cmd pkg-config --cflags jansson)
        link_flags="$(pkg_config_cmd pkg-config --libs jansson) -Wl,-rpath,$installed_libdir"
        expected_link=$installed_libdir
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

    clean_env "$cc_bin" -std=c99 -Wall -Wextra -Werror $include_flags "$src" -o "$exe" $link_flags
    verify_linkage "$exe" "$expected_link"
    printf '%s\n' "$base" >>"$manifest"
done

printf '%s\n' "$manifest"
