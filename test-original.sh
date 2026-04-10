#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_IMAGE="${DOCKER_IMAGE:-ubuntu:24.04}"
JANSSON_IMPLEMENTATION="${JANSSON_IMPLEMENTATION:-original}"
JANSSON_TEST_MODE="${JANSSON_TEST_MODE:-runtime}"

exec env \
  DOCKER_IMAGE="${DOCKER_IMAGE}" \
  JANSSON_IMPLEMENTATION="${JANSSON_IMPLEMENTATION}" \
  JANSSON_TEST_MODE="${JANSSON_TEST_MODE}" \
  "${ROOT_DIR}/safe/scripts/run-dependent-image-tests.sh" \
  --image "${DOCKER_IMAGE}" \
  --implementation "${JANSSON_IMPLEMENTATION}" \
  --mode "${JANSSON_TEST_MODE}"
