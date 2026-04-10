#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
JANSSON_IMPLEMENTATION="${JANSSON_IMPLEMENTATION:-original}"
JANSSON_TEST_MODE="${JANSSON_TEST_MODE:-runtime}"
DEPENDENT_IMAGE_TAG="${DEPENDENT_IMAGE_TAG:-${DOCKER_IMAGE:-libjansson-dependent-matrix:${JANSSON_IMPLEMENTATION}}}"
BASE_IMAGE="${BASE_IMAGE:-ubuntu:24.04}"

"${ROOT_DIR}/safe/scripts/build-dependent-image.sh" \
  --implementation "${JANSSON_IMPLEMENTATION}" \
  --tag "${DEPENDENT_IMAGE_TAG}" \
  --base-image "${BASE_IMAGE}"

exec env \
  JANSSON_IMPLEMENTATION="${JANSSON_IMPLEMENTATION}" \
  JANSSON_TEST_MODE="${JANSSON_TEST_MODE}" \
  "${ROOT_DIR}/safe/scripts/run-dependent-image-tests.sh" \
  --image "${DEPENDENT_IMAGE_TAG}" \
  --implementation "${JANSSON_IMPLEMENTATION}" \
  --mode "${JANSSON_TEST_MODE}"
