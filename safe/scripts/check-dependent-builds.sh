#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEPENDENTS_FILE="${DEPENDENTS_FILE:-${ROOT_DIR}/dependents.json}"
DIST_DIR="${ROOT_DIR}/safe/dist"
JANSSON_IMPLEMENTATION="${JANSSON_IMPLEMENTATION:-safe}"
APT_PIN_FILE="/etc/apt/preferences.d/libjansson-safe.pref"
BUILD_ROOT="$(mktemp -d /tmp/libjansson-dependent-builds.XXXXXX)"

note() {
  printf '\n==> %s\n' "$1"
}

fail() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
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

expected_sources=$'emacs\njanus\njose\njshon\nlibteam\nmtr\nsuricata\ntang\nulogd2\nwayvnc\nwebdis'
actual_sources="$(jq -r '.dependents[].source_package' "${DEPENDENTS_FILE}" | sort -u)"
[ -n "${actual_sources}" ] || fail "No source packages found in ${DEPENDENTS_FILE}"
[ "${actual_sources}" = "${expected_sources}" ] || fail \
  "Unexpected source_package set in ${DEPENDENTS_FILE}:
expected:
${expected_sources}
actual:
${actual_sources}"

mapfile -t SOURCE_PACKAGES < <(printf '%s\n' "${actual_sources}")

SAFE_RUNTIME_VERSION=
SAFE_DEV_VERSION=

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

refresh_apt_metadata
enable_source_repositories
install_selected_packages

note "Building dependent source packages"

for srcpkg in "${SOURCE_PACKAGES[@]}"; do
  pkg_workdir="${BUILD_ROOT}/${srcpkg}"
  rm -rf "${pkg_workdir}"
  mkdir -p "${pkg_workdir}"

  note "Fetching source package ${srcpkg}"
  (
    cd "${pkg_workdir}"
    apt-get source "${srcpkg}"
  )

  assert_selected_versions

  note "Installing build-dependencies for ${srcpkg}"
  apt-get build-dep -y "${srcpkg}"
  assert_selected_versions

  srcdir="$(resolve_source_dir "${pkg_workdir}" "${srcpkg}")"

  note "Compiling ${srcpkg} with DEB_BUILD_OPTIONS=nocheck"
  (
    cd "${srcdir}"
    DEB_BUILD_OPTIONS=nocheck dpkg-buildpackage -B -uc -us
  )

  assert_selected_versions
done

note "Successfully rebuilt ${#SOURCE_PACKAGES[@]} dependent source packages"
