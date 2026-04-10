#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEPENDENTS_FILE="${DEPENDENTS_FILE:-${ROOT_DIR}/dependents.json}"
DIST_DIR="${ROOT_DIR}/safe/dist"
JANSSON_IMPLEMENTATION="${JANSSON_IMPLEMENTATION:-safe}"
APT_PIN_FILE="/etc/apt/preferences.d/libjansson-safe.pref"
BUILD_ROOT="$(mktemp -d /tmp/libjansson-dependent-builds.XXXXXX)"
LOG_ROOT="${DEPENDENT_MATRIX_LOG_ROOT:-${ROOT_DIR}/safe/.build/dependent-matrix/${JANSSON_IMPLEMENTATION}/build}"
ISSUES_JSONL="${DEPENDENT_MATRIX_ISSUES_JSONL:-${LOG_ROOT}/issues.jsonl}"
RUN_STARTED_AT="${DEPENDENT_MATRIX_RUN_STARTED_AT:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
SOURCE_PACKAGE_FILTER="${DEPENDENT_MATRIX_SOURCE_PACKAGES:-}"

note() {
  printf '\n==> %s\n' "$1"
}

fail() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

csv_contains() {
  local csv="$1"
  local needle="$2"
  local item

  IFS=',' read -r -a items <<<"${csv}"
  for item in "${items[@]}"; do
    [ "${item}" = "${needle}" ] && return 0
  done

  return 1
}

cleanup() {
  rm -rf "${BUILD_ROOT}"
  rm -f "${APT_PIN_FILE}"
  apt-mark unhold libjansson4 libjansson-dev >/dev/null 2>&1 || true
}

trap cleanup EXIT

case "${JANSSON_IMPLEMENTATION}" in
  original|safe)
    ;;
  *)
    fail "Unsupported JANSSON_IMPLEMENTATION=${JANSSON_IMPLEMENTATION} (expected original or safe)"
    ;;
esac

[ -f "${DEPENDENTS_FILE}" ] || fail "Missing dependents manifest: ${DEPENDENTS_FILE}"

expected_sources=$'emacs\njanus\njose\njshon\nlibteam\nmtr\nnghttp2\nsuricata\ntang\nulogd2\nwayvnc\nwebdis'
actual_sources="$(jq -r '.dependents[].source_package' "${DEPENDENTS_FILE}" | sort -u)"
[ -n "${actual_sources}" ] || fail "No source packages found in ${DEPENDENTS_FILE}"
[ "${actual_sources}" = "${expected_sources}" ] || fail \
  "Unexpected source_package set in ${DEPENDENTS_FILE}:
expected:
${expected_sources}
actual:
${actual_sources}"

mapfile -t SOURCE_PACKAGES < <(printf '%s\n' "${actual_sources}")

if [ -n "${SOURCE_PACKAGE_FILTER}" ]; then
  FILTERED_SOURCE_PACKAGES=()
  for srcpkg in "${SOURCE_PACKAGES[@]}"; do
    if csv_contains "${SOURCE_PACKAGE_FILTER}" "${srcpkg}"; then
      FILTERED_SOURCE_PACKAGES+=("${srcpkg}")
    fi
  done
  SOURCE_PACKAGES=("${FILTERED_SOURCE_PACKAGES[@]}")
  [ "${#SOURCE_PACKAGES[@]}" -gt 0 ] || fail \
    "DEPENDENT_MATRIX_SOURCE_PACKAGES did not match any known source package: ${SOURCE_PACKAGE_FILTER}"
  note "Restricting dependent source-package rebuilds to: ${SOURCE_PACKAGES[*]}"
fi

SAFE_RUNTIME_VERSION=
SAFE_DEV_VERSION=
FAILURE_COUNT=0

mkdir -p "${LOG_ROOT}" "$(dirname "${ISSUES_JSONL}")"
rm -rf "${LOG_ROOT:?}/"*
: >"${ISSUES_JSONL}"

relative_path() {
  local path="$1"

  case "${path}" in
    "${ROOT_DIR}/"*)
      printf '%s\n' "${path#${ROOT_DIR}/}"
      ;;
    *)
      printf '%s\n' "${path}"
      ;;
  esac
}

collapse_log_excerpt() {
  local stderr_log="$1"
  local stdout_log="$2"

  python3 - "${stderr_log}" "${stdout_log}" <<'PY'
from pathlib import Path
import sys

chunks = []

for label, path_str in (("stderr", sys.argv[1]), ("stdout", sys.argv[2])):
    path = Path(path_str)
    if not path.exists():
        continue
    lines = [line.strip() for line in path.read_text(encoding="utf-8", errors="replace").splitlines() if line.strip()]
    if not lines:
        continue
    chunks.append(f"{label}: {' | '.join(lines[-8:])}")

summary = " || ".join(chunks) if chunks else "See referenced logs for details."
print(summary[:1000])
PY
}

append_issue_jsonl() {
  local phase="$1"
  local application="$2"
  local check="$3"
  local title="$4"
  local command="$5"
  local expected_behavior="$6"
  local observed_behavior="$7"
  local suspected_subsystem="$8"
  local log_path="$9"

  python3 - "${ISSUES_JSONL}" "${phase}" "${application}" "${check}" "${title}" \
    "${command}" "${expected_behavior}" "${observed_behavior}" "${suspected_subsystem}" \
    "${log_path}" "${RUN_STARTED_AT}" <<'PY'
from pathlib import Path
import json
import sys

path = Path(sys.argv[1])
path.parent.mkdir(parents=True, exist_ok=True)

record = {
    "phase": sys.argv[2],
    "application": sys.argv[3],
    "check": sys.argv[4],
    "title": sys.argv[5],
    "command": sys.argv[6],
    "expected_behavior": sys.argv[7],
    "observed_behavior": sys.argv[8],
    "suspected_subsystem": sys.argv[9],
    "log_path": sys.argv[10],
    "recorded_at_utc": sys.argv[11],
}

with path.open("a", encoding="utf-8") as handle:
    json.dump(record, handle, sort_keys=True)
    handle.write("\n")
PY
}

run_logged_stage() {
  local log_dir="$1"
  local stage="$2"
  local command="$3"
  local stdout_log="${log_dir}/${stage}.stdout.log"
  local stderr_log="${log_dir}/${stage}.stderr.log"
  local status_file="${log_dir}/${stage}.status"
  local command_file="${log_dir}/${stage}.command.txt"
  local status

  printf '%s\n' "${command}" >"${command_file}"
  set +e
  bash -lc "${command}" >"${stdout_log}" 2>"${stderr_log}"
  status=$?
  set -e
  printf '%s\n' "${status}" >"${status_file}"
  return "${status}"
}

refresh_apt_metadata() {
  note "Refreshing apt metadata"
  apt-get update
}

enable_source_repositories() {
  local sources_file
  local list_file

  if apt-cache showsrc bash >/dev/null 2>&1; then
    return 0
  fi

  note "Enabling Ubuntu source repositories"

  if [ -f /etc/apt/sources.list.d/ubuntu.sources ]; then
    sed -i '/^Types:/ {
      /deb-src/! s/$/ deb-src/
    }' /etc/apt/sources.list.d/ubuntu.sources
  fi

  shopt -s nullglob
  for sources_file in /etc/apt/sources.list.d/*.sources; do
    sed -i '/^Types:/ {
      /deb-src/! s/$/ deb-src/
    }' "${sources_file}"
  done

  for list_file in /etc/apt/sources.list /etc/apt/sources.list.d/*.list; do
    [ -f "${list_file}" ] || continue
    sed -i -E 's/^[#[:space:]]*deb-src[[:space:]]+/deb-src /' "${list_file}"
  done
  shopt -u nullglob

  refresh_apt_metadata
  apt-cache showsrc bash >/dev/null 2>&1 || fail "Source repositories are still unavailable after enabling deb-src entries"
}

assert_selected_versions() {
  local runtime_version
  local dev_version

  runtime_version="$(dpkg-query -W -f='${Version}' libjansson4 2>/dev/null || true)"
  dev_version="$(dpkg-query -W -f='${Version}' libjansson-dev 2>/dev/null || true)"

  case "${JANSSON_IMPLEMENTATION}" in
    safe)
      [ "${runtime_version}" = "${SAFE_RUNTIME_VERSION}" ] || fail \
        "libjansson4 version changed to ${runtime_version:-<missing>} (expected ${SAFE_RUNTIME_VERSION})"
      [ "${dev_version}" = "${SAFE_DEV_VERSION}" ] || fail \
        "libjansson-dev version changed to ${dev_version:-<missing>} (expected ${SAFE_DEV_VERSION})"
      ;;
    original)
      [ -n "${runtime_version}" ] || fail "libjansson4 is not installed"
      [ -n "${dev_version}" ] || fail "libjansson-dev is not installed"
      ;;
  esac
}

install_selected_packages() {
  local runtime_deb
  local dev_deb

  case "${JANSSON_IMPLEMENTATION}" in
    safe)
      runtime_deb="$(find "${DIST_DIR}" -maxdepth 1 -type f -name 'libjansson4_*.deb' | sort | tail -n 1)"
      dev_deb="$(find "${DIST_DIR}" -maxdepth 1 -type f -name 'libjansson-dev_*.deb' | sort | tail -n 1)"
      [ -n "${runtime_deb}" ] || fail "Missing safe runtime package under ${DIST_DIR}; run safe/scripts/build-deb.sh first"
      [ -n "${dev_deb}" ] || fail "Missing safe development package under ${DIST_DIR}; run safe/scripts/build-deb.sh first"

      SAFE_RUNTIME_VERSION="$(dpkg-deb -f "${runtime_deb}" Version)"
      SAFE_DEV_VERSION="$(dpkg-deb -f "${dev_deb}" Version)"

      note "Installing locally built safe libjansson packages"
      dpkg -i "${runtime_deb}" "${dev_deb}"

      cat >"${APT_PIN_FILE}" <<EOF
Package: libjansson4
Pin: version ${SAFE_RUNTIME_VERSION}
Pin-Priority: 1001

Package: libjansson-dev
Pin: version ${SAFE_DEV_VERSION}
Pin-Priority: 1001
EOF

      apt-mark hold libjansson4 libjansson-dev >/dev/null
      ;;
    original)
      note "Installing Ubuntu archive libjansson packages for the package-manager baseline"
      apt-get install -y --no-install-recommends libjansson4 libjansson-dev
      ;;
  esac

  ldconfig
  assert_selected_versions
}

resolve_source_dir() {
  local parent_dir="$1"
  local srcpkg="$2"
  local extracted_dir
  local extracted_dirs=()

  mapfile -t extracted_dirs < <(find "${parent_dir}" -mindepth 1 -maxdepth 1 -type d | sort)
  [ "${#extracted_dirs[@]}" -eq 1 ] || fail \
    "Expected exactly one extracted source directory for ${srcpkg}, found ${#extracted_dirs[@]}"
  extracted_dir="${extracted_dirs[0]}"
  [ -n "${extracted_dir}" ] || fail "Failed to locate extracted source directory for ${srcpkg}"
  printf '%s\n' "${extracted_dir}"
}

build_command_for() {
  local srcdir="$1"
  local srcpkg="$2"
  local command
  local build_options="nocheck nostrip noautodbgsym"

  case "${srcpkg}" in
    emacs)
      printf -v command 'cd %q && EMACS_INHIBIT_NATIVE_COMPILATION=1 DEB_BUILD_OPTIONS=%q dpkg-buildpackage -B -uc -us' \
        "${srcdir}" "${build_options}"
      ;;
    *)
      printf -v command 'cd %q && DEB_BUILD_OPTIONS=%q dpkg-buildpackage -B -uc -us' \
        "${srcdir}" "${build_options}"
      ;;
  esac

  printf '%s\n' "${command}"
}

record_stage_failure() {
  local srcpkg="$1"
  local stage="$2"
  local title="$3"
  local command="$4"
  local expected_behavior="$5"
  local log_dir="$6"
  local observed_behavior
  local log_path

  observed_behavior="$(collapse_log_excerpt "${log_dir}/${stage}.stderr.log" "${log_dir}/${stage}.stdout.log")"
  log_path="$(relative_path "${log_dir}/${stage}.stderr.log")"
  append_issue_jsonl build "${srcpkg}" "${stage}" "${title}" "${command}" \
    "${expected_behavior}" "${observed_behavior}" packaging "${log_path}"
}

refresh_apt_metadata
enable_source_repositories
install_selected_packages

note "Building dependent source packages"

for srcpkg in "${SOURCE_PACKAGES[@]}"; do
  pkg_workdir="${BUILD_ROOT}/${srcpkg}"
  pkg_log_dir="${LOG_ROOT}/${srcpkg}"
  srcdir=
  fetch_cmd=
  build_dep_cmd=
  build_cmd=

  rm -rf "${pkg_workdir}" "${pkg_log_dir}"
  mkdir -p "${pkg_workdir}" "${pkg_log_dir}"

  printf -v fetch_cmd 'cd %q && apt-get source %q' "${pkg_workdir}" "${srcpkg}"
  note "Fetching source package ${srcpkg}"
  if ! run_logged_stage "${pkg_log_dir}" source-fetch "${fetch_cmd}"; then
    record_stage_failure "${srcpkg}" source-fetch \
      "Fetch Ubuntu source package ${srcpkg}" \
      "${fetch_cmd}" \
      "apt-get source ${srcpkg} should succeed so the dependent matrix can rebuild the package." \
      "${pkg_log_dir}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
    continue
  fi

  assert_selected_versions

  build_dep_cmd="apt-get build-dep -y ${srcpkg}"
  note "Installing build-dependencies for ${srcpkg}"
  if ! run_logged_stage "${pkg_log_dir}" build-dependencies "${build_dep_cmd}"; then
    record_stage_failure "${srcpkg}" build-dependencies \
      "Install build-dependencies for ${srcpkg}" \
      "${build_dep_cmd}" \
      "apt-get build-dep -y ${srcpkg} should succeed without replacing the selected libjansson packages." \
      "${pkg_log_dir}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
    continue
  fi

  assert_selected_versions
  srcdir="$(resolve_source_dir "${pkg_workdir}" "${srcpkg}")"
  build_cmd="$(build_command_for "${srcdir}" "${srcpkg}")"

  note "Compiling ${srcpkg}"
  if ! run_logged_stage "${pkg_log_dir}" source-build "${build_cmd}"; then
    record_stage_failure "${srcpkg}" source-build \
      "Rebuild source package ${srcpkg}" \
      "${build_cmd}" \
      "${srcpkg} should rebuild successfully against ${JANSSON_IMPLEMENTATION} libjansson." \
      "${pkg_log_dir}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
    continue
  fi

  assert_selected_versions
done

if [ "${FAILURE_COUNT}" -ne 0 ]; then
  fail "Encountered ${FAILURE_COUNT} dependent source-package build failure(s)"
fi

note "Successfully rebuilt ${#SOURCE_PACKAGES[@]} dependent source packages"
