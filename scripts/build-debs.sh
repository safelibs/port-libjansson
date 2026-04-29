#!/usr/bin/env bash
# Build the safe libjansson port via the port-owned safe/scripts/build-deb.sh
# and collect the resulting *.deb files into dist/.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
dist_dir="$repo_root/dist"

rm -rf -- "$dist_dir"
mkdir -p -- "$dist_dir"

cd "$repo_root"
bash safe/scripts/build-deb.sh

shopt -s nullglob
debs=(safe/dist/*.deb)
shopt -u nullglob
if (( ${#debs[@]} == 0 )); then
  printf 'build-debs: no *.deb files produced under safe/dist/\n' >&2
  exit 1
fi

cp -v "${debs[@]}" "$dist_dir"/
