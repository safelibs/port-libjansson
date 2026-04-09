#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

exec env \
  JANSSON_IMPLEMENTATION=safe \
  JANSSON_TEST_MODE="${JANSSON_TEST_MODE:-all}" \
  "${ROOT_DIR}/test-original.sh"
