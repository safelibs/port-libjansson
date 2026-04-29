#!/usr/bin/env bash
# libjansson: drive the port-owned safe/scripts/build-deb.sh and copy
# the resulting *.deb files into dist/.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/scripts/lib/build-deb-common.sh"

prepare_dist_dir "$repo_root"

cd "$repo_root"
bash safe/scripts/build-deb.sh

shopt -s nullglob
debs=(safe/dist/*.deb)
shopt -u nullglob
if (( ${#debs[@]} == 0 )); then
  printf 'build-debs: no *.deb produced under safe/dist/\n' >&2
  exit 1
fi
cp -v "${debs[@]}" "$repo_root/dist"/
