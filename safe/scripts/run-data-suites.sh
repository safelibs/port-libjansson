#!/bin/sh
set -eu

usage() {
    echo "Usage: $0 [suite_name ...]" >&2
    exit 2
}

root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
safe_dir="$root/safe"
build_dir="$safe_dir/.build/data-suites"
bin_dir="$build_dir/bin"
runtime_dir="$build_dir/runtime-lib"
compat_root="$build_dir/top-src"
json_process_src="$safe_dir/tests/upstream-bin/json_process.c"
json_process_bin="$bin_dir/json_process"
cc_bin=${CC:-cc}

if [ $# -eq 0 ]; then
    set -- valid invalid invalid-unicode encoding-flags
fi

mkdir -p "$build_dir" "$bin_dir" "$runtime_dir" "$compat_root/test"

cargo build --release --manifest-path "$safe_dir/Cargo.toml"
ln -sfn "$safe_dir/target/release/libjansson.so" "$runtime_dir/libjansson.so.4"

"$cc_bin" -std=c99 -Wall -Wextra -Werror \
    -I"$safe_dir/include" \
    "$json_process_src" \
    -o "$json_process_bin" \
    -L"$safe_dir/target/release" \
    -Wl,-rpath,"$runtime_dir" \
    -ljansson

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
    if LD_LIBRARY_PATH="$runtime_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" "$suite_run" "$suite"; then
        passed=$(expr "$passed" + 1)
    else
        failed=$(expr "$failed" + 1)
        [ "${STOP:-0}" -eq 1 ] && break
    fi
done

if [ "$failed" -gt 0 ]; then
    echo "$failed of $(expr "$passed" + "$failed") test suites failed" >&2
    exit 1
fi

echo "$passed test suites passed"
rm -rf "$logdir"
