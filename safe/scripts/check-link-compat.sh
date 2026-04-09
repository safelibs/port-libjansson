#!/bin/sh
set -eu

usage() {
    echo "Usage: $0 [--installed-root ROOT]" >&2
    exit 2
}

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
. "$root/safe/scripts/installed-root-common.sh"
safe_dir="$root/safe"
upstream_root="$root/original/jansson-2.14"
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
        --installed-root)
            [ $# -ge 2 ] || usage
            installed_root=$2
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

if [ -z "$installed_root" ]; then
    installed_root=/
fi
installed_root=$(resolve_installed_root "$root" "$installed_root")

[ -n "$runtime_version" ] || {
    echo "failed to resolve ABI runtime version from safe/build.rs" >&2
    exit 1
}
[ -n "$soname" ] || {
    echo "failed to resolve SONAME from safe/build.rs" >&2
    exit 1
}

installed_include="$installed_root/usr/include"
installed_libdir="$installed_root/usr/lib/$multiarch"
header_source="$upstream_root/src/jansson.h"
config_source="$installed_include/jansson_config.h"

[ -f "$header_source" ] || {
    echo "missing original public header: $header_source" >&2
    exit 1
}
[ -f "$config_source" ] || {
    echo "missing installed jansson_config.h: $config_source" >&2
    exit 1
}
[ -f "$installed_libdir/libjansson.so" ] || {
    echo "missing installed development symlink: $installed_libdir/libjansson.so" >&2
    exit 1
}
[ -f "$installed_libdir/libjansson.so.$runtime_version" ] || {
    echo "missing installed runtime library: $installed_libdir/libjansson.so.$runtime_version" >&2
    exit 1
}

sanitize_root_tag() {
    printf '%s' "$1" | sed 's#[^A-Za-z0-9_.-]#_#g'
}

clean_env() {
    env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH -u LIBRARY_PATH -u CPATH -u C_INCLUDE_PATH "$@"
}

verify_installed_linkage() {
    exe=$1
    actual=$(clean_env ldd "$exe" | awk '/libjansson\.so\.4/ { print $3; exit }')

    [ -n "$actual" ] || {
        echo "failed to resolve libjansson.so.4 for $exe" >&2
        exit 1
    }

    actual=$(readlink -f "$actual")
    expected_prefix=$(readlink -f "$installed_libdir")
    case "$actual" in
        "$expected_prefix"/*)
            ;;
        *)
            echo "expected $exe to resolve libjansson.so.4 under $expected_prefix but got $actual" >&2
            exit 1
            ;;
    esac
}

build_tag=$(sanitize_root_tag "$installed_root")
build_dir="$safe_dir/.build/link-compat/$build_tag"
header_dir="$build_dir/include"
obj_dir="$build_dir/obj"
bin_dir="$build_dir/bin"
json_process_bin="$bin_dir/json_process"
simple_parse_bin="$bin_dir/simple_parse"
api_src_dir="$upstream_root/test/suites/api"
cc_bin=${CC:-cc}

rm -rf "$build_dir"
mkdir -p "$header_dir" "$obj_dir" "$bin_dir"

cp "$header_source" "$header_dir/jansson.h"
cp "$config_source" "$header_dir/jansson_config.h"

for test_src in "$api_src_dir"/test_*.c; do
    base=${test_src##*/}
    base=${base%.c}
    clean_env "$cc_bin" -std=c99 -Wall -Wextra -Werror -I"$header_dir" -I"$api_src_dir" \
        -c "$test_src" -o "$obj_dir/$base.o"
done

clean_env "$cc_bin" -std=c99 -Wall -Wextra -Werror -I"$header_dir" \
    -c "$upstream_root/test/bin/json_process.c" -o "$obj_dir/json_process.o"
clean_env "$cc_bin" -std=c99 -Wall -Wextra -Werror -I"$header_dir" \
    -c "$upstream_root/examples/simple_parse.c" -o "$obj_dir/simple_parse.o"

for obj_file in "$obj_dir"/test_*.o; do
    base=${obj_file##*/}
    base=${base%.o}
    clean_env "$cc_bin" "$obj_file" -o "$bin_dir/$base" \
        -L"$installed_libdir" -Wl,-rpath,"$installed_libdir" -ljansson
    verify_installed_linkage "$bin_dir/$base"
    clean_env "$bin_dir/$base"
done

clean_env "$cc_bin" "$obj_dir/json_process.o" -o "$json_process_bin" \
    -L"$installed_libdir" -Wl,-rpath,"$installed_libdir" -ljansson
verify_installed_linkage "$json_process_bin"

clean_env "$cc_bin" "$obj_dir/simple_parse.o" -o "$simple_parse_bin" \
    -L"$installed_libdir" -Wl,-rpath,"$installed_libdir" -ljansson
verify_installed_linkage "$simple_parse_bin"
printf '{"name":"barney"}\n' | clean_env "$simple_parse_bin" >/dev/null

clean_env "$json_process_bin" "$safe_dir/tests/upstream-suites/valid/simple-object"
clean_env "$json_process_bin" "$safe_dir/tests/upstream-suites/invalid/null"
