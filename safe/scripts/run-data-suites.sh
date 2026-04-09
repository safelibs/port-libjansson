#!/bin/sh
set -eu

usage() {
    echo "Usage: $0 [--installed-dev --installed-root ROOT] [suite_name ...]" >&2
    exit 2
}

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
safe_dir="$root/safe"
mode="build-tree"
installed_root=
multiarch=$(dpkg-architecture -qDEB_HOST_MULTIARCH)
runtime_version=$(
    sed -n 's/^const ABI_RUNTIME_VERSION: &str = "\(.*\)";/\1/p' "$safe_dir/build.rs"
)
soname=$(
    sed -n 's/^const ABI_VERSION_NODE: &str = "\(.*\)";/\1/p' "$safe_dir/build.rs"
)

while [ $# -gt 0 ]; do
    case "$1" in
        --installed-dev)
            mode="installed-dev"
            shift
            ;;
        --installed-root)
            [ $# -ge 2 ] || usage
            installed_root=$2
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

if [ $# -eq 0 ]; then
    set -- valid invalid invalid-unicode encoding-flags
fi

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
    build_log=$safe_dir/.build/data-suites.native-static-libs.log

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

verify_shared_linkage() {
    exe=$1
    expected=$2
    actual=$(clean_env ldd "$exe" | awk '/libjansson\.so\.4/ { print $3; exit }')

    [ -n "$actual" ] || {
        echo "failed to resolve libjansson.so.4 for $exe" >&2
        exit 1
    }

    actual=$(readlink -f "$actual")
    expected=$(readlink -f "$expected")
    [ "$actual" = "$expected" ] || {
        echo "expected $exe to use $expected but it resolved to $actual" >&2
        exit 1
    }
}

verify_static_linkage() {
    exe=$1

    if clean_env ldd "$exe" | awk '/libjansson\.so\.4/ { found = 1 } END { exit found ? 0 : 1 }'; then
        echo "expected $exe to consume the installed static archive, but it links libjansson.so.4" >&2
        exit 1
    fi
}

build_tag=$mode
if [ "$mode" = "installed-dev" ]; then
    build_tag=$mode-$(sanitize_root_tag "$installed_root")
fi

build_dir="$safe_dir/.build/data-suites/$build_tag"
bin_dir="$build_dir/bin"
runtime_dir="$build_dir/runtime-lib"
compat_root="$build_dir/top-src"
json_process_src="$safe_dir/tests/upstream-bin/json_process.c"
json_process_bin="$bin_dir/json_process"
cc_bin=${CC:-cc}

mkdir -p "$build_dir" "$bin_dir" "$runtime_dir" "$compat_root/test"

case "$mode" in
    build-tree)
        emit_build_tree_shared
        ln -sfn "$safe_dir/target/release/libjansson.so.$runtime_version" "$runtime_dir/$soname"

        clean_env "$cc_bin" -std=c99 -Wall -Wextra -Werror \
            -I"$safe_dir/include" \
            "$json_process_src" \
            -o "$json_process_bin" \
            -L"$safe_dir/target/release" \
            -Wl,-rpath,"$runtime_dir" \
            -ljansson

        verify_shared_linkage "$json_process_bin" "$safe_dir/target/release/libjansson.so.$runtime_version"
        ;;
    installed-dev)
        installed_include=$installed_root/usr/include/jansson.h
        installed_libdir=$installed_root/usr/lib/$multiarch

        [ -f "$installed_include" ] || {
            echo "missing installed header: $installed_include" >&2
            exit 1
        }
        [ -f "$installed_libdir/libjansson.a" ] || {
            echo "missing installed static archive: $installed_libdir/libjansson.a" >&2
            exit 1
        }

        static_libs=$(pkg_config_cmd pkg-config --static --libs-only-l jansson | \
            sed 's/\(^\|[[:space:]]\)-ljansson\([[:space:]]\|$\)/ /g')
        static_other=$(pkg_config_cmd pkg-config --static --libs-only-other jansson)

        clean_env "$cc_bin" -std=c99 -Wall -Wextra -Werror \
            $(pkg_config_cmd pkg-config --cflags jansson) \
            "$json_process_src" \
            -o "$json_process_bin" \
            "$installed_libdir/libjansson.a" \
            $static_libs \
            $static_other

        verify_static_linkage "$json_process_bin"
        ;;
    *)
        usage
        ;;
esac

ln -sfn "$safe_dir/tests/upstream-scripts" "$compat_root/test/scripts"
ln -sfn "$safe_dir/tests/upstream-suites" "$compat_root/test/suites"

top_srcdir="$compat_root"
suites_srcdir="$safe_dir/tests/upstream-suites"
suites_builddir="$build_dir/suites"
scriptdir="$safe_dir/tests/upstream-scripts"
logdir="$build_dir/logs"
bindir="$bin_dir"
export top_srcdir suites_srcdir suites_builddir scriptdir logdir bindir

passed=0
failed=0
for suite in "$@"; do
    suite_run="$safe_dir/tests/upstream-suites/$suite/run"
    [ -x "$suite_run" ] || {
        echo "No such suite: $suite" >&2
        exit 1
    }

    echo "Suite: $suite"
    if [ "$mode" = "build-tree" ]; then
        if env LD_LIBRARY_PATH="$runtime_dir" "$suite_run" "$suite"; then
            passed=$(expr "$passed" + 1)
        else
            failed=$(expr "$failed" + 1)
            [ "${STOP:-0}" -eq 1 ] && break
        fi
    else
        if clean_env "$suite_run" "$suite"; then
            passed=$(expr "$passed" + 1)
        else
            failed=$(expr "$failed" + 1)
            [ "${STOP:-0}" -eq 1 ] && break
        fi
    fi
done

if [ "$failed" -gt 0 ]; then
    echo "$failed of $(expr "$passed" + "$failed") test suites failed" >&2
    exit 1
fi

echo "$passed test suites passed"
rm -rf "$logdir"
