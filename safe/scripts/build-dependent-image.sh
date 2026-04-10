#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DOCKERFILE="${ROOT_DIR}/safe/docker/dependent-matrix.Dockerfile"
DEPENDENTS_FILE="${ROOT_DIR}/dependents.json"
DIST_DIR="${ROOT_DIR}/safe/dist"
IMPLEMENTATION="safe"
IMAGE_TAG=
BASE_IMAGE="ubuntu:24.04"
HELPER_BINARY_PACKAGES="nghttp2-server"

usage() {
  cat <<'EOF' >&2
Usage: safe/scripts/build-dependent-image.sh [--implementation original|safe] [--tag TAG] [--base-image IMAGE]
EOF
  exit 2
}

note() {
  printf '\n==> %s\n' "$1"
}

fail() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

join_with_spaces() {
  local joined

  [ "$#" -gt 0 ] || fail "Refusing to build an image with an empty package list"
  printf -v joined '%s ' "$@"
  printf '%s\n' "${joined% }"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --implementation)
      [ "$#" -ge 2 ] || usage
      IMPLEMENTATION="$2"
      shift 2
      ;;
    --tag)
      [ "$#" -ge 2 ] || usage
      IMAGE_TAG="$2"
      shift 2
      ;;
    --base-image)
      [ "$#" -ge 2 ] || usage
      BASE_IMAGE="$2"
      shift 2
      ;;
    --help|-h)
      usage
      ;;
    *)
      usage
      ;;
  esac
done

case "${IMPLEMENTATION}" in
  original|safe)
    ;;
  *)
    fail "Unsupported --implementation=${IMPLEMENTATION} (expected original or safe)"
    ;;
esac

[ -f "${DOCKERFILE}" ] || fail "Missing Dockerfile: ${DOCKERFILE}"
[ -f "${DEPENDENTS_FILE}" ] || fail "Missing dependents manifest: ${DEPENDENTS_FILE}"

if ! command -v docker >/dev/null 2>&1; then
  fail "docker is required to build the dependent image"
fi

if [ -z "${IMAGE_TAG}" ]; then
  IMAGE_TAG="libjansson-dependent-matrix:${IMPLEMENTATION}"
fi

mapfile -t PRIMARY_BINARY_PACKAGES < <(jq -r '.dependents[].binary_package' "${DEPENDENTS_FILE}" | sort -u)
[ "${#PRIMARY_BINARY_PACKAGES[@]}" -gt 0 ] || fail "No binary_package entries found in ${DEPENDENTS_FILE}"
DEPENDENT_BINARY_PACKAGES="$(join_with_spaces "${PRIMARY_BINARY_PACKAGES[@]}")"

SAFE_RUNTIME_DEB=
SAFE_DEV_DEB=

if [ "${IMPLEMENTATION}" = "safe" ]; then
  SAFE_RUNTIME_DEB="$(find "${DIST_DIR}" -maxdepth 1 -type f -name 'libjansson4_*.deb' | sort | tail -n 1)"
  SAFE_DEV_DEB="$(find "${DIST_DIR}" -maxdepth 1 -type f -name 'libjansson-dev_*.deb' | sort | tail -n 1)"

  if [ -z "${SAFE_RUNTIME_DEB}" ] || [ -z "${SAFE_DEV_DEB}" ]; then
    note "Building safe Debian packages because safe/dist/ is incomplete"
    "${ROOT_DIR}/safe/scripts/build-deb.sh"
    SAFE_RUNTIME_DEB="$(find "${DIST_DIR}" -maxdepth 1 -type f -name 'libjansson4_*.deb' | sort | tail -n 1)"
    SAFE_DEV_DEB="$(find "${DIST_DIR}" -maxdepth 1 -type f -name 'libjansson-dev_*.deb' | sort | tail -n 1)"
  fi

  [ -n "${SAFE_RUNTIME_DEB}" ] || fail "Missing safe runtime package under ${DIST_DIR}"
  [ -n "${SAFE_DEV_DEB}" ] || fail "Missing safe development package under ${DIST_DIR}"
  [ -f "${SAFE_RUNTIME_DEB}" ] || fail "Missing safe runtime package ${SAFE_RUNTIME_DEB}"
  [ -f "${SAFE_DEV_DEB}" ] || fail "Missing safe development package ${SAFE_DEV_DEB}"
fi

BUILD_CONTEXT="$(mktemp -d "${TMPDIR:-/tmp}/libjansson-dependent-image.XXXXXX")"
cleanup() {
  rm -rf "${BUILD_CONTEXT}"
}
trap cleanup EXIT

mkdir -p "${BUILD_CONTEXT}/safe-dist"
touch "${BUILD_CONTEXT}/safe-dist/.keep"

if [ "${IMPLEMENTATION}" = "safe" ]; then
  cp -f "${SAFE_RUNTIME_DEB}" "${BUILD_CONTEXT}/safe-dist/"
  cp -f "${SAFE_DEV_DEB}" "${BUILD_CONTEXT}/safe-dist/"
fi

note "Building Docker image ${IMAGE_TAG}"
docker build \
  --build-arg "BASE_IMAGE=${BASE_IMAGE}" \
  --build-arg "JANSSON_IMPLEMENTATION=${IMPLEMENTATION}" \
  --build-arg "DEPENDENT_BINARY_PACKAGES=${DEPENDENT_BINARY_PACKAGES}" \
  --build-arg "HELPER_BINARY_PACKAGES=${HELPER_BINARY_PACKAGES}" \
  --tag "${IMAGE_TAG}" \
  --file "${DOCKERFILE}" \
  "${BUILD_CONTEXT}"

printf '%s\n' "${IMAGE_TAG}"
