#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_IMAGE="${DOCKER_IMAGE:-ubuntu:24.04}"
JANSSON_TEST_MODE="${JANSSON_TEST_MODE:-all}"

exec env \
  DOCKER_IMAGE="${DOCKER_IMAGE}" \
  JANSSON_IMPLEMENTATION=safe \
  JANSSON_TEST_MODE="${JANSSON_TEST_MODE}" \
  "${ROOT_DIR}/safe/scripts/run-dependent-image-tests.sh" \
  --image "${DOCKER_IMAGE}" \
  --implementation safe \
  --mode "${JANSSON_TEST_MODE}"
