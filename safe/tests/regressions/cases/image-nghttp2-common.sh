#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
IMAGE_TAG="${REGRESSION_IMAGE_TAG:-libjansson-regressions:safe}"
RUN_ROOT="${REGRESSION_RUN_ROOT:-${ROOT_DIR}/safe/.build/regressions/manual}"
HOST_CASE_ROOT="${RUN_ROOT}/${CASE_NAME}"

case "${HOST_CASE_ROOT}" in
  "${ROOT_DIR}/"*)
    CONTAINER_CASE_ROOT="/work/${HOST_CASE_ROOT#${ROOT_DIR}/}"
    ;;
  *)
    printf 'ERROR: REGRESSION_RUN_ROOT must stay under %s so the container can write into the mounted repo\n' "${ROOT_DIR}" >&2
    exit 1
    ;;
esac

STATUS_FILE="${HOST_CASE_ROOT}/dependent-matrix/safe/runtime/nghttp2/${CHECK_NAME}.status"
HAR_FILE="${HOST_CASE_ROOT}/dependent-matrix/safe/runtime/nghttp2/capture.har"

if ! docker image inspect "${IMAGE_TAG}" >/dev/null 2>&1; then
  "${ROOT_DIR}/safe/scripts/build-dependent-image.sh" --implementation safe --tag "${IMAGE_TAG}" >/dev/null
fi

rm -rf "${HOST_CASE_ROOT}"
mkdir -p "${HOST_CASE_ROOT}"

env \
  JANSSON_RUNTIME_APPLICATIONS=nghttp2 \
  JANSSON_RUNTIME_CHECKS="${CHECK_NAME}" \
  DEPENDENT_MATRIX_LOG_ROOT_BASE="${CONTAINER_CASE_ROOT}/dependent-matrix/safe" \
  DEPENDENT_MATRIX_ISSUE_FILE="${CONTAINER_CASE_ROOT}/discovered-issues.md" \
  "${ROOT_DIR}/safe/scripts/run-dependent-image-tests.sh" \
  --image "${IMAGE_TAG}" \
  --implementation safe \
  --mode runtime

[ -f "${STATUS_FILE}" ] || {
  printf 'ERROR: Missing status file %s\n' "${STATUS_FILE}" >&2
  exit 1
}
[ "$(cat "${STATUS_FILE}")" = "0" ] || {
  printf 'ERROR: Expected %s to pass\n' "${CHECK_NAME}" >&2
  exit 1
}

if [ "${CHECK_NAME}" = "har-json-structure" ]; then
  [ -f "${HAR_FILE}" ] || {
    printf 'ERROR: Missing HAR output %s\n' "${HAR_FILE}" >&2
    exit 1
  }
  python3 - "${HAR_FILE}" <<'PY'
import json
import sys

har = json.load(open(sys.argv[1], "r", encoding="utf-8"))
assert isinstance(har.get("log"), dict)
assert isinstance(har["log"].get("entries"), list)
PY
fi
