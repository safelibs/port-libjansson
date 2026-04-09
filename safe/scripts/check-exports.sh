#!/bin/sh
set -eu

usage() {
    echo "Usage: $0 [--names-only] [--lib PATH] [--installed-root ROOT]" >&2
    exit 2
}

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
safe_dir="$root/safe"
def_file="$root/original/jansson-2.14/src/jansson.def"
symbols_file="$root/original/jansson-2.14/debian/libjansson4.symbols"
multiarch=$(dpkg-architecture -qDEB_HOST_MULTIARCH)
runtime_version=$(
    sed -n 's/^const ABI_RUNTIME_VERSION: &str = "\(.*\)";/\1/p' "$safe_dir/build.rs"
)
soname=$(
    sed -n 's/^const ABI_VERSION_NODE: &str = "\(.*\)";/\1/p' "$safe_dir/build.rs"
)
check_versions=1
lib_file=
installed_root=

while [ $# -gt 0 ]; do
    case "$1" in
        --names-only)
            check_versions=0
            shift
            ;;
        --lib)
            [ $# -ge 2 ] || usage
            lib_file=$2
            shift 2
            ;;
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

[ -n "$runtime_version" ] || {
    echo "failed to resolve ABI runtime version from safe/build.rs" >&2
    exit 1
}
[ -n "$soname" ] || {
    echo "failed to resolve SONAME from safe/build.rs" >&2
    exit 1
}

case "$installed_root" in
    "")
        ;;
    /*)
        ;;
    *)
        installed_root=$root/$installed_root
        ;;
esac

build_log="$safe_dir/.build/check-exports.native-static-libs.log"

emit_build_tree_shared() {
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

if [ -n "$lib_file" ] && [ -n "$installed_root" ]; then
    echo "--lib and --installed-root are mutually exclusive" >&2
    exit 2
fi

if [ -z "$lib_file" ]; then
    if [ -n "$installed_root" ]; then
        lib_file="$installed_root/usr/lib/$multiarch/libjansson.so.$runtime_version"
    else
        emit_build_tree_shared
        lib_file="$safe_dir/target/release/libjansson.so.$runtime_version"
    fi
fi

[ -f "$lib_file" ] || {
    echo "missing shared library: $lib_file" >&2
    exit 1
}

expected_names=$(mktemp)
actual_names=$(mktemp)
expected_versions=$(mktemp)
actual_versions=$(mktemp)
trap 'rm -f "$expected_names" "$actual_names" "$expected_versions" "$actual_versions"' EXIT

grep 'json_\|jansson_' "$def_file" | sed 's/ //g' | sort >"$expected_names"

readelf --dyn-syms --wide "$lib_file" 2>/dev/null | awk '
    $5 == "GLOBAL" && $7 != "UND" {
        sym = $8
        name = sym
        version = "Base"

        if (sym == "libjansson.so.4") {
            print sym " " sym
            next
        }

        if (index(sym, "@@") > 0) {
            split(sym, parts, "@@")
            name = parts[1]
            version = parts[2]
        } else if (index(sym, "@") > 0) {
            split(sym, parts, "@")
            name = parts[1]
            version = parts[2]
        }

        if (name ~ /^(json_|jansson_)/)
            print name " " version
    }
' | sort >"$actual_versions"

awk '{ print $1 }' "$actual_versions" | grep -E '^(json_|jansson_)' | sort >"$actual_names"

if ! cmp -s "$expected_names" "$actual_names"; then
    diff -u "$expected_names" "$actual_names" >&2
    exit 1
fi

if [ "$check_versions" -eq 0 ]; then
    exit 0
fi

awk '
    $1 ~ /^(json_|jansson_|libjansson\.so\.4@libjansson\.so\.4$)/ {
        split($1, parts, "@")
        print parts[1] " " parts[2]
    }
' "$symbols_file" | sort >"$expected_versions"

if ! cmp -s "$expected_versions" "$actual_versions"; then
    diff -u "$expected_versions" "$actual_versions" >&2
    exit 1
fi
