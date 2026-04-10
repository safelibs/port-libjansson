#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DOCKER_IMAGE="${DOCKER_IMAGE:-ubuntu:24.04}"
JANSSON_IMPLEMENTATION="${JANSSON_IMPLEMENTATION:-original}"
JANSSON_TEST_MODE="${JANSSON_TEST_MODE:-runtime}"

usage() {
  cat <<'EOF' >&2
Usage: safe/scripts/run-dependent-image-tests.sh [--image IMAGE] [--implementation original|safe] [--mode build|runtime|all]
EOF
  exit 2
}

fail() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --image)
      [ "$#" -ge 2 ] || usage
      DOCKER_IMAGE="$2"
      shift 2
      ;;
    --implementation)
      [ "$#" -ge 2 ] || usage
      JANSSON_IMPLEMENTATION="$2"
      shift 2
      ;;
    --mode)
      [ "$#" -ge 2 ] || usage
      JANSSON_TEST_MODE="$2"
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

case "${JANSSON_IMPLEMENTATION}" in
  original|safe)
    ;;
  *)
    fail "Unsupported JANSSON_IMPLEMENTATION=${JANSSON_IMPLEMENTATION} (expected original or safe)"
    ;;
esac

case "${JANSSON_TEST_MODE}" in
  build|runtime|all)
    ;;
  *)
    fail "Unsupported JANSSON_TEST_MODE=${JANSSON_TEST_MODE} (expected build, runtime, or all)"
    ;;
esac

if ! command -v docker >/dev/null 2>&1; then
  fail "docker is required to run the dependent matrix"
fi

docker run --rm -i \
  -e HOST_UID="$(id -u)" \
  -e HOST_GID="$(id -g)" \
  -e JANSSON_IMPLEMENTATION="${JANSSON_IMPLEMENTATION}" \
  -e JANSSON_TEST_MODE="${JANSSON_TEST_MODE}" \
  -v "${ROOT_DIR}:/work" \
  -w /work \
  "${DOCKER_IMAGE}" \
  /work/safe/scripts/in-container-dependent-tests.sh
