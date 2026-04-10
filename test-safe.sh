#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
JANSSON_TEST_MODE="${JANSSON_TEST_MODE:-all}"
DEPENDENT_IMAGE_TAG="${DEPENDENT_IMAGE_TAG:-${DOCKER_IMAGE:-libjansson-dependent-matrix:safe}}"
BASE_IMAGE="${BASE_IMAGE:-ubuntu:24.04}"

"${ROOT_DIR}/safe/scripts/build-dependent-image.sh" \
  --implementation safe \
  --tag "${DEPENDENT_IMAGE_TAG}" \
  --base-image "${BASE_IMAGE}"

exec env \
  JANSSON_IMPLEMENTATION=safe \
  JANSSON_TEST_MODE="${JANSSON_TEST_MODE}" \
  "${ROOT_DIR}/safe/scripts/run-dependent-image-tests.sh" \
  --image "${DEPENDENT_IMAGE_TAG}" \
  --implementation safe \
  --mode "${JANSSON_TEST_MODE}"
