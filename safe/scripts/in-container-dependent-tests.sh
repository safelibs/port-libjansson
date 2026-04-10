#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
export DEBIAN_FRONTEND=noninteractive

JANSSON_IMPLEMENTATION="${JANSSON_IMPLEMENTATION:-original}"
JANSSON_TEST_MODE="${JANSSON_TEST_MODE:-runtime}"
HOST_UID="${HOST_UID:-}"
HOST_GID="${HOST_GID:-}"
JANSSON_RUNTIME_APPLICATIONS="${JANSSON_RUNTIME_APPLICATIONS:-}"
JANSSON_RUNTIME_CHECKS="${JANSSON_RUNTIME_CHECKS:-}"
JANSSON_BUILD_SOURCE_PACKAGES="${JANSSON_BUILD_SOURCE_PACKAGES:-}"
DEPENDENT_MATRIX_LOG_ROOT_BASE="${DEPENDENT_MATRIX_LOG_ROOT_BASE:-}"
DEPENDENT_MATRIX_ISSUE_FILE="${DEPENDENT_MATRIX_ISSUE_FILE:-}"

case "${JANSSON_IMPLEMENTATION}" in
  original|safe)
    ;;
  *)
    printf 'ERROR: Unsupported JANSSON_IMPLEMENTATION=%s (expected original or safe)\n' \
      "${JANSSON_IMPLEMENTATION}" >&2
    exit 1
    ;;
esac

case "${JANSSON_TEST_MODE}" in
  build|runtime|all)
    ;;
  *)
    printf 'ERROR: Unsupported JANSSON_TEST_MODE=%s (expected build, runtime, or all)\n' \
      "${JANSSON_TEST_MODE}" >&2
    exit 1
    ;;
esac

RUN_BUILD=0
RUN_RUNTIME=0

case "${JANSSON_TEST_MODE}" in
  build)
    RUN_BUILD=1
    ;;
  runtime)
    RUN_RUNTIME=1
    ;;
  all)
    RUN_BUILD=1
    RUN_RUNTIME=1
    ;;
esac

MATRIX_RUN_STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
LOG_ROOT_BASE="${DEPENDENT_MATRIX_LOG_ROOT_BASE:-${ROOT_DIR}/safe/.build/dependent-matrix/${JANSSON_IMPLEMENTATION}}"
BUILD_LOG_ROOT="${LOG_ROOT_BASE}/build"
RUNTIME_LOG_ROOT="${LOG_ROOT_BASE}/runtime"
BUILD_ISSUES_JSONL="${BUILD_LOG_ROOT}/issues.jsonl"
RUNTIME_ISSUES_JSONL="${RUNTIME_LOG_ROOT}/issues.jsonl"
ISSUE_FILE="${DEPENDENT_MATRIX_ISSUE_FILE:-${ROOT_DIR}/safe/tests/regressions/discovered-issues.md}"

MULTIARCH=
SELECTED_JANSSON=
SELECTED_LABEL=
SAFE_RUNTIME_DEB=
SAFE_DEV_DEB=
ULOGD_PLUGIN_DIR=
ULOGD_INPUT_PLUGIN=
ULOGD_BASE_PLUGIN=
ULOGD_OUTPUT_PLUGIN=
APP_LOG_DIR=
RUNTIME_SELECTED_COUNT=0

note() {
  printf '\n==> %s\n' "$1"
}

fail() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

repair_workspace_permissions() {
  [ -n "${HOST_UID}" ] || return 0
  [ -n "${HOST_GID}" ] || return 0

  chown -R "${HOST_UID}:${HOST_GID}" \
    "${ROOT_DIR}/safe/.build" \
    "${ROOT_DIR}/safe/dist" \
    "${ROOT_DIR}/safe/target" \
    "${ROOT_DIR}/safe/tests/regressions" \
    2>/dev/null || true
}

trap repair_workspace_permissions EXIT

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

should_run_runtime_check() {
  local application="$1"
  local check="$2"

  if [ -n "${JANSSON_RUNTIME_APPLICATIONS}" ] && \
    ! csv_contains "${JANSSON_RUNTIME_APPLICATIONS}" "${application}"; then
    return 1
  fi

  if [ -z "${JANSSON_RUNTIME_CHECKS}" ]; then
    return 0
  fi

  if csv_contains "${JANSSON_RUNTIME_CHECKS}" "${check}" || \
    csv_contains "${JANSSON_RUNTIME_CHECKS}" "${application}:${check}" || \
    csv_contains "${JANSSON_RUNTIME_CHECKS}" "${application}/${check}"; then
    return 0
  fi

  return 1
}

random_port() {
  python3 - <<'PY'
import socket

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.bind(("127.0.0.1", 0))
print(sock.getsockname()[1])
sock.close()
PY
}

wait_for_url() {
  local url="$1"
  local attempts="${2:-80}"
  local i

  for i in $(seq 1 "${attempts}"); do
    if curl -fsS "${url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done

  fail "Timed out waiting for ${url}"
}

wait_for_socket() {
  local path="$1"
  local attempts="${2:-80}"
  local i

  for i in $(seq 1 "${attempts}"); do
    if [ -S "${path}" ]; then
      return 0
    fi
    sleep 0.1
  done

  fail "Timed out waiting for socket ${path}"
}

wait_for_tcp_port() {
  local host="$1"
  local port="$2"
  local attempts="${3:-80}"
  local i

  for i in $(seq 1 "${attempts}"); do
    if python3 - "${host}" "${port}" <<'PY'
import socket
import sys

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.settimeout(0.2)
try:
    sock.connect((sys.argv[1], int(sys.argv[2])))
except OSError:
    raise SystemExit(1)
finally:
    sock.close()
PY
    then
      return 0
    fi
    sleep 0.1
  done

  fail "Timed out waiting for TCP socket ${host}:${port}"
}

reset_log_root() {
  local path="$1"

  rm -rf "${path}"
  mkdir -p "${path}"
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
    excerpt = " | ".join(lines[-8:])
    chunks.append(f"{label}: {excerpt}")

summary = " || ".join(chunks) if chunks else "See referenced logs for details."
print(summary[:1000])
PY
}

append_issue_jsonl() {
  local jsonl_path="$1"
  local phase="$2"
  local application="$3"
  local check="$4"
  local title="$5"
  local command="$6"
  local expected_behavior="$7"
  local observed_behavior="$8"
  local suspected_subsystem="$9"
  local log_path="${10}"

  python3 - "${jsonl_path}" "${phase}" "${application}" "${check}" "${title}" \
    "${command}" "${expected_behavior}" "${observed_behavior}" "${suspected_subsystem}" \
    "${log_path}" "${MATRIX_RUN_STARTED_AT}" <<'PY'
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

record_runtime_issue() {
  append_issue_jsonl "${RUNTIME_ISSUES_JSONL}" "$@"
}

assert_uses_selected_jansson() {
  local bin="$1"
  local resolved

  resolved="$(ldd "${bin}" | awk '/libjansson\.so\.4/ { print $3; exit }')"
  [ -n "${resolved}" ] || fail "${bin} does not resolve libjansson.so.4"

  resolved="$(readlink -f "${resolved}")"
  [ "${resolved}" = "${SELECTED_JANSSON}" ] || fail \
    "${bin} resolved libjansson.so.4 to ${resolved}, expected ${SELECTED_JANSSON} (${SELECTED_LABEL})"
}

install_mode_packages() {
  local packages=()

  if [ "${RUN_BUILD}" -eq 1 ]; then
    packages+=(
      build-essential
      ca-certificates
      dpkg-dev
      jq
      python3
    )
  fi

  if [ "${RUN_RUNTIME}" -eq 1 ]; then
    packages+=(
      ca-certificates
      curl
      dpkg-dev
      emacs-nox
      iproute2
      janus
      jose
      jshon
      libteam-utils
      mtr-tiny
      nghttp2-client
      nghttp2-server
      procps
      python3
      redis-server
      suricata
      tang-common
      ulogd2
      ulogd2-json
      wayvnc
      webdis
    )
  fi

  case "${JANSSON_IMPLEMENTATION}" in
    original)
      if [ "${RUN_RUNTIME}" -eq 1 ]; then
        packages+=(
          autoconf
          automake
          build-essential
          libtool
        )
      fi
      ;;
    safe)
      packages+=(
        build-essential
        cargo
        rustc
      )
      ;;
  esac

  note "Installing toolchain and package prerequisites"
  apt-get update
  apt-get install -y --no-install-recommends "${packages[@]}"

  MULTIARCH="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
}

resolve_safe_packages() {
  note "Resolving prebuilt safe replacement packages"
  SAFE_RUNTIME_DEB="$(find "${ROOT_DIR}/safe/dist" -maxdepth 1 -type f -name 'libjansson4_*.deb' | sort | tail -n 1)"
  SAFE_DEV_DEB="$(find "${ROOT_DIR}/safe/dist" -maxdepth 1 -type f -name 'libjansson-dev_*.deb' | sort | tail -n 1)"
  [ -n "${SAFE_RUNTIME_DEB}" ] || fail "Missing prebuilt safe runtime package under ${ROOT_DIR}/safe/dist; run safe/scripts/build-deb.sh first"
  [ -n "${SAFE_DEV_DEB}" ] || fail "Missing prebuilt safe development package under ${ROOT_DIR}/safe/dist; run safe/scripts/build-deb.sh first"
  [ -f "${SAFE_RUNTIME_DEB}" ] || fail "Missing safe runtime package ${SAFE_RUNTIME_DEB}"
  [ -f "${SAFE_DEV_DEB}" ] || fail "Missing safe development package ${SAFE_DEV_DEB}"
}

install_original_jansson() {
  note "Building and installing the original Jansson source"
  rm -rf /tmp/jansson-src
  cp -a "${ROOT_DIR}/original/jansson-2.14" /tmp/jansson-src
  (
    cd /tmp/jansson-src
    autoreconf -fi
    ./configure --prefix=/usr/local
    make -j"$(getconf _NPROCESSORS_ONLN)"
    make install
  )
  ldconfig

  SELECTED_JANSSON="$(find /usr/local -name 'libjansson.so.4' -type l | head -n 1)"
  [ -n "${SELECTED_JANSSON}" ] || fail "Failed to find the installed /usr/local libjansson.so.4"
  export LD_LIBRARY_PATH="$(dirname "${SELECTED_JANSSON}")${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
  SELECTED_JANSSON="$(readlink -f "${SELECTED_JANSSON}")"
  SELECTED_LABEL="the original /usr/local build"
}

install_safe_jansson() {
  [ -n "${SAFE_RUNTIME_DEB}" ] || fail "Safe runtime package path is unset"
  [ -n "${SAFE_DEV_DEB}" ] || fail "Safe development package path is unset"

  note "Installing locally built safe libjansson packages with dpkg -i"
  dpkg -i "${SAFE_RUNTIME_DEB}" "${SAFE_DEV_DEB}"
  ldconfig

  SELECTED_JANSSON="$(dpkg-query -L libjansson4 | awk '/\/libjansson\.so\.4\.[0-9]/ { print; exit }')"
  [ -n "${SELECTED_JANSSON}" ] || fail "Failed to resolve the installed safe libjansson runtime path"
  SELECTED_JANSSON="$(readlink -f "${SELECTED_JANSSON}")"
  SELECTED_LABEL="the installed safe libjansson package"
}

resolve_ulogd_plugins() {
  ULOGD_OUTPUT_PLUGIN="$(dpkg-query -L ulogd2-json | awk '/\/ulogd_output_JSON\.so$/ { print; exit }')"
  [ -n "${ULOGD_OUTPUT_PLUGIN}" ] || fail "Failed to resolve ulogd_output_JSON.so from ulogd2-json"

  ULOGD_PLUGIN_DIR="$(dirname "${ULOGD_OUTPUT_PLUGIN}")"
  ULOGD_INPUT_PLUGIN="${ULOGD_PLUGIN_DIR}/ulogd_inppkt_UNIXSOCK.so"
  ULOGD_BASE_PLUGIN="${ULOGD_PLUGIN_DIR}/ulogd_raw2packet_BASE.so"

  [ -f "${ULOGD_INPUT_PLUGIN}" ] || fail "Missing ${ULOGD_INPUT_PLUGIN}"
  [ -f "${ULOGD_BASE_PLUGIN}" ] || fail "Missing ${ULOGD_BASE_PLUGIN}"
  [ -f "${ULOGD_OUTPUT_PLUGIN}" ] || fail "Missing ${ULOGD_OUTPUT_PLUGIN}"
}

run_build_harness() {
  note "Rebuilding dependent Ubuntu source packages against ${JANSSON_IMPLEMENTATION} libjansson"
  reset_log_root "${BUILD_LOG_ROOT}"
  : >"${BUILD_ISSUES_JSONL}"

  DEPENDENT_MATRIX_LOG_ROOT="${BUILD_LOG_ROOT}" \
  DEPENDENT_MATRIX_ISSUES_JSONL="${BUILD_ISSUES_JSONL}" \
  DEPENDENT_MATRIX_RUN_STARTED_AT="${MATRIX_RUN_STARTED_AT}" \
  DEPENDENT_MATRIX_SOURCE_PACKAGES="${JANSSON_BUILD_SOURCE_PACKAGES}" \
  JANSSON_IMPLEMENTATION="${JANSSON_IMPLEMENTATION}" \
  "${ROOT_DIR}/safe/scripts/check-dependent-builds.sh"
}

run_runtime_check() {
  local application="$1"
  local check="$2"
  local title="$3"
  local command="$4"
  local expected_behavior="$5"
  local suspected_subsystem="$6"
  local function_name="$7"
  local app_dir="${RUNTIME_LOG_ROOT}/${application}"
  local stdout_log="${app_dir}/${check}.stdout.log"
  local stderr_log="${app_dir}/${check}.stderr.log"
  local status_file="${app_dir}/${check}.status"
  local command_file="${app_dir}/${check}.command.txt"
  local log_path
  local observed_behavior
  local status

  mkdir -p "${app_dir}"
  printf '%s\n' "${command}" >"${command_file}"

  set +e
  (
    export APP_LOG_DIR="${app_dir}"
    "${function_name}"
  ) >"${stdout_log}" 2>"${stderr_log}"
  status=$?
  set -e

  printf '%s\n' "${status}" >"${status_file}"
  if [ "${status}" -eq 0 ]; then
    return 0
  fi

  observed_behavior="$(collapse_log_excerpt "${stderr_log}" "${stdout_log}")"
  log_path="$(relative_path "${stderr_log}")"
  record_runtime_issue runtime "${application}" "${check}" "${title}" "${command}" \
    "${expected_behavior}" "${observed_behavior}" "${suspected_subsystem}" "${log_path}"
  return 1
}

run_selected_runtime_check() {
  local application="$1"
  local check="$2"
  local title="$3"
  local command="$4"
  local expected_behavior="$5"
  local suspected_subsystem="$6"
  local function_name="$7"

  if ! should_run_runtime_check "${application}" "${check}"; then
    return 0
  fi

  RUNTIME_SELECTED_COUNT=$((RUNTIME_SELECTED_COUNT + 1))
  run_runtime_check \
    "${application}" \
    "${check}" \
    "${title}" \
    "${command}" \
    "${expected_behavior}" \
    "${suspected_subsystem}" \
    "${function_name}"
}

check_emacs_resolution() {
  assert_uses_selected_jansson /usr/bin/emacs
}

check_janus_resolution() {
  assert_uses_selected_jansson /usr/bin/janus
}

check_jose_resolution() {
  assert_uses_selected_jansson /usr/bin/jose
}

check_jshon_resolution() {
  assert_uses_selected_jansson /usr/bin/jshon
}

check_mtr_resolution() {
  assert_uses_selected_jansson /usr/bin/mtr
}

check_nghttp2_resolution() {
  assert_uses_selected_jansson /usr/bin/nghttp
}

check_suricata_resolution() {
  assert_uses_selected_jansson /usr/bin/suricata
}

check_tang_resolution() {
  assert_uses_selected_jansson /usr/libexec/tangd
}

check_libteam_resolution() {
  assert_uses_selected_jansson /usr/bin/teamd
  assert_uses_selected_jansson /usr/bin/teamdctl
}

check_ulogd_resolution() {
  assert_uses_selected_jansson "${ULOGD_OUTPUT_PLUGIN}"
}

check_wayvnc_resolution() {
  assert_uses_selected_jansson /usr/bin/wayvnc
  assert_uses_selected_jansson /usr/bin/wayvncctl
}

check_webdis_resolution() {
  assert_uses_selected_jansson /usr/bin/webdis
}

test_emacs() {
  note "Testing Emacs JSON support"
  local out

  out="$(emacs --batch --eval '(princ (json-serialize (json-parse-string "{\"x\":1}")))' 2>/dev/null)"
  printf '%s\n' "${out}" >"${APP_LOG_DIR}/emacs.out"
  [ "${out}" = '{"x":1}' ] || fail "Emacs JSON round-trip failed: ${out}"
}

test_janus() (
  note "Testing Janus HTTP JSON API"

  local pid=""
  local log="${APP_LOG_DIR}/janus.log"
  local janus_cfg_dir="${APP_LOG_DIR}/janus-config"

  trap 'kill "${pid}" 2>/dev/null || true; wait "${pid}" 2>/dev/null || true' EXIT

  rm -rf "${janus_cfg_dir}"
  mkdir -p "${janus_cfg_dir}"
  cp -a /etc/janus/. "${janus_cfg_dir}/"
  python3 - "${janus_cfg_dir}/janus.transport.http.jcfg" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
lines = path.read_text(encoding="utf-8").splitlines()
result = []
in_general = False
inserted = False

for line in lines:
    stripped = line.strip()
    if stripped.startswith("general:"):
        in_general = True
    if in_general and (stripped.startswith("#ip =") or stripped.startswith("ip =")):
        if not inserted:
            result.append('\tip = "127.0.0.1"\t\t\t\t# Force IPv4 for containerized smoke tests')
            inserted = True
        continue
    if in_general and stripped == "}":
        if not inserted:
            result.append('\tip = "127.0.0.1"\t\t\t\t# Force IPv4 for containerized smoke tests')
            inserted = True
        in_general = False
    result.append(line)

path.write_text("\n".join(result) + "\n", encoding="utf-8")
PY

  janus -F "${janus_cfg_dir}" >"${log}" 2>&1 &
  pid="$!"

  wait_for_url "http://127.0.0.1:8088/janus/info"
  curl -fsS http://127.0.0.1:8088/janus/info >"${APP_LOG_DIR}/janus-info.json"
  curl -fsS \
    -H 'Content-Type: application/json' \
    -d '{"janus":"create","transaction":"txn1"}' \
    http://127.0.0.1:8088/janus >"${APP_LOG_DIR}/janus-create.json"

  python3 - "${APP_LOG_DIR}/janus-info.json" "${APP_LOG_DIR}/janus-create.json" <<'PY'
import json
import sys

info = json.load(open(sys.argv[1], "r", encoding="utf-8"))
create = json.load(open(sys.argv[2], "r", encoding="utf-8"))

assert info["janus"] == "server_info"
assert info["dependencies"]["jansson"] == "2.14"
assert create["janus"] == "success"
assert isinstance(create["data"]["id"], int)
PY
)

test_jshon() {
  note "Testing jshon JSON parsing"
  local out

  out="$(printf '%s' '{"foo":1,"bar":[2,3]}' | jshon -e bar -a -u)"
  printf '%s\n' "${out}" >"${APP_LOG_DIR}/jshon.out"
  [ "${out}" = $'2\n3' ] || fail "jshon returned unexpected output: ${out}"
}

test_jose() {
  note "Testing jose JSON/JWK handling"

  jose jwk gen -i '{"alg":"ES256"}' -o "${APP_LOG_DIR}/jose.jwk" >/dev/null
  python3 - "${APP_LOG_DIR}/jose.jwk" <<'PY'
import json
import sys

jwk = json.load(open(sys.argv[1], "r", encoding="utf-8"))
assert jwk["alg"] == "ES256"
assert jwk["kty"] == "EC"
assert "sign" in jwk["key_ops"]
PY
}

test_mtr() {
  note "Testing MTR JSON reporting"

  mtr -r -j -n -c 1 127.0.0.1 >"${APP_LOG_DIR}/mtr.json"
  python3 - "${APP_LOG_DIR}/mtr.json" <<'PY'
import json
import sys

report = json.load(open(sys.argv[1], "r", encoding="utf-8"))
hubs = report["report"]["hubs"]
assert hubs
assert hubs[0]["host"] == "127.0.0.1"
PY
}

test_nghttp2() (
  note "Testing nghttp2 HAR generation"

  local pid=""
  local port
  local docroot="${APP_LOG_DIR}/htdocs"
  local har_path="${APP_LOG_DIR}/capture.har"
  local fixture="${ROOT_DIR}/safe/tests/regressions/fixtures/nghttp2/index.json"

  trap 'kill "${pid}" 2>/dev/null || true; wait "${pid}" 2>/dev/null || true' EXIT

  rm -rf "${docroot}"
  mkdir -p "${docroot}"
  if [ -f "${fixture}" ]; then
    cp "${fixture}" "${docroot}/index.json"
  else
    printf '{"ok":true}\n' >"${docroot}/index.json"
  fi

  port="$(random_port)"
  nghttpd --no-tls -d "${docroot}" -a 127.0.0.1 "${port}" \
    >"${APP_LOG_DIR}/nghttpd.stdout.log" \
    2>"${APP_LOG_DIR}/nghttpd.stderr.log" &
  pid="$!"

  wait_for_tcp_port 127.0.0.1 "${port}"
  nghttp -ans --har="${har_path}" "http://127.0.0.1:${port}/index.json" \
    >"${APP_LOG_DIR}/nghttp.stdout.log" \
    2>"${APP_LOG_DIR}/nghttp.stderr.log"

  python3 - "${har_path}" <<'PY'
import json
import sys

har = json.load(open(sys.argv[1], "r", encoding="utf-8"))
assert isinstance(har, dict)
assert isinstance(har.get("log"), dict)
assert isinstance(har["log"].get("entries"), list)
PY
)

test_suricata() {
  note "Testing Suricata EVE JSON logging"

  rm -rf "${APP_LOG_DIR}/suricata-out"
  mkdir -p "${APP_LOG_DIR}/suricata-out"

  python3 - "${APP_LOG_DIR}/suricata-test.pcap" <<'PY'
import struct
import sys
import time

pcap = bytearray()
pcap += struct.pack("<IHHIIII", 0xA1B2C3D4, 2, 4, 0, 0, 65535, 1)

eth = bytes.fromhex("00112233445566778899aabb0800")
ip = bytearray(20)
ip[0] = 0x45
ip_total_len = 28
ip[2:4] = struct.pack("!H", ip_total_len)
ip[4:6] = b"\x00\x01"
ip[8] = 64
ip[9] = 1
ip[12:16] = bytes([127, 0, 0, 1])
ip[16:20] = bytes([127, 0, 0, 1])

checksum = 0
for i in range(0, 20, 2):
    checksum += (ip[i] << 8) + ip[i + 1]
while checksum >> 16:
    checksum = (checksum & 0xFFFF) + (checksum >> 16)
ip[10:12] = struct.pack("!H", (~checksum) & 0xFFFF)

icmp = bytearray(8)
icmp[0] = 8
icmp[4:6] = b"\x12\x34"
icmp[6:8] = b"\x00\x01"
checksum = 0
for i in range(0, 8, 2):
    checksum += (icmp[i] << 8) + icmp[i + 1]
while checksum >> 16:
    checksum = (checksum & 0xFFFF) + (checksum >> 16)
icmp[2:4] = struct.pack("!H", (~checksum) & 0xFFFF)

packet = eth + ip + icmp
pcap += struct.pack("<IIII", int(time.time()), 0, len(packet), len(packet))
pcap += packet

with open(sys.argv[1], "wb") as handle:
    handle.write(pcap)
PY

  suricata \
    -r "${APP_LOG_DIR}/suricata-test.pcap" \
    -l "${APP_LOG_DIR}/suricata-out" \
    -c /etc/suricata/suricata.yaml \
    >"${APP_LOG_DIR}/suricata.stdout.log" \
    2>"${APP_LOG_DIR}/suricata.stderr.log"

  python3 - "${APP_LOG_DIR}/suricata-out/eve.json" <<'PY'
import json
import sys

events = [json.loads(line) for line in open(sys.argv[1], "r", encoding="utf-8")]
assert any(event.get("event_type") == "flow" for event in events)
PY
}

test_tang() (
  note "Testing Tang advertisement and recovery endpoints"

  local tmpdir="${APP_LOG_DIR}/tangd"
  local port
  local pid=""
  local exc_kid
  local template
  local good
  local reply

  trap 'kill "${pid}" 2>/dev/null || true; wait "${pid}" 2>/dev/null || true' EXIT

  rm -rf "${tmpdir}"
  mkdir -p "${tmpdir}/db"
  /usr/libexec/tangd-keygen "${tmpdir}/db" sig exc >/dev/null

  exc_kid="$(jose jwk thp -i "${tmpdir}/db/exc.jwk")"
  template="$(jose fmt -j "${tmpdir}/db/exc.jwk" -Od x -d y -d d -o-)"
  jose jwk gen -i "${template}" -o "${tmpdir}/exc.jwk" >/dev/null
  jose jwk pub -i "${tmpdir}/exc.jwk" -o "${tmpdir}/exc.pub.jwk" >/dev/null

  port="$(random_port)"
  /usr/libexec/tangd -l -p "${port}" "${tmpdir}/db" >"${APP_LOG_DIR}/tangd.log" 2>&1 &
  pid="$!"

  wait_for_url "http://127.0.0.1:${port}/adv"
  curl -fsS "http://127.0.0.1:${port}/adv" >"${tmpdir}/adv.jose"
  jose jws ver -i "${tmpdir}/adv.jose" -k "${tmpdir}/db/sig.jwk" >/dev/null

  good="$(jose jwk exc -i '{"alg":"ECMR","key_ops":["deriveKey"]}' -l "${tmpdir}/exc.jwk" -r "${tmpdir}/db/exc.jwk")"
  reply="$(curl -fsS -X POST \
    -H 'Content-Type: application/jwk+json' \
    --data-binary @"${tmpdir}/exc.pub.jwk" \
    "http://127.0.0.1:${port}/rec/${exc_kid}")"

  printf '%s\n' "${reply}" >"${APP_LOG_DIR}/tang-reply.json"
  [ "${good}" = "${reply}" ] || fail "Tang recovery response did not match the expected exchanged key"
)

test_teamd() (
  note "Testing teamd JSON config parsing and teamdctl JSON dump parsing"

  local teamd_log="${APP_LOG_DIR}/teamd.log"
  local server_pid=""
  local out

  teamd -t team0 -n -U -c '{"runner":{"name":"activebackup"}}' >"${teamd_log}" 2>&1 || true
  grep -F 'Failed to create team device.' "${teamd_log}" >/dev/null || fail "teamd did not reach device-creation after parsing JSON config"

  mkdir -p /var/run/teamd
  rm -f /var/run/teamd/lo.sock

  python3 - <<'PY' &
import json
import os
import socket

path = "/var/run/teamd/lo.sock"
responses = {
    "ConfigDump": {"device": "lo", "runner": {"name": "activebackup"}, "ports": {}},
    "ConfigDumpActual": {"device": "lo", "runner": {"name": "activebackup"}, "ports": {}},
    "StateDump": {"setup": {"runner_name": "activebackup"}, "ports": {}, "runner": {"active_port": None}},
}

try:
    os.unlink(path)
except FileNotFoundError:
    pass

server = socket.socket(socket.AF_UNIX, socket.SOCK_SEQPACKET)
server.bind(path)
server.listen(1)
conn, _ = server.accept()

while True:
    request = conn.recv(4096)
    if not request:
        break
    lines = [line for line in request.decode().split("\n") if line]
    method = lines[1] if len(lines) > 1 else ""
    payload = json.dumps(responses.get(method, {}), separators=(",", ":"))
    conn.sendall(f"REPLY_SUCCESS\n{payload}".encode())

conn.close()
server.close()
PY
  server_pid="$!"
  trap 'kill "${server_pid}" 2>/dev/null || true; wait "${server_pid}" 2>/dev/null || true; rm -f /var/run/teamd/lo.sock' EXIT

  wait_for_socket /var/run/teamd/lo.sock
  out="$(teamdctl -U lo state dump)"
  printf '%s\n' "${out}" >"${APP_LOG_DIR}/teamdctl-state.json"
  wait "${server_pid}"

  python3 - <<'PY' "${out}"
import json
import sys

data = json.loads(sys.argv[1])
assert data["setup"]["runner_name"] == "activebackup"
PY
)

test_ulogd() (
  note "Testing ulogd2 JSON output plugin"

  local pid=""
  local socket_path="${APP_LOG_DIR}/ulogd-test.sock"
  local config_path="${APP_LOG_DIR}/ulogd-test.conf"
  local json_path="${APP_LOG_DIR}/ulogd-test.json"

  trap 'kill "${pid}" 2>/dev/null || true; wait "${pid}" 2>/dev/null || true; rm -f "${socket_path}"' EXIT

  cat >"${config_path}" <<EOF
[global]
logfile="stdout"
loglevel=3
plugin="${ULOGD_INPUT_PLUGIN}"
plugin="${ULOGD_BASE_PLUGIN}"
plugin="${ULOGD_OUTPUT_PLUGIN}"
stack=us1:UNIXSOCK,base1:BASE,json1:JSON

[us1]
socket_path="${socket_path}"

[json1]
sync=1
file="${json_path}"
EOF

  rm -f "${socket_path}" "${json_path}"
  ulogd -v -c "${config_path}" >"${APP_LOG_DIR}/ulogd.stdout.log" 2>"${APP_LOG_DIR}/ulogd.stderr.log" &
  pid="$!"

  wait_for_socket "${socket_path}"
  python3 - "${socket_path}" <<'PY'
import socket
import struct
import sys

path = sys.argv[1]
sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect(path)

payload = bytearray(28)
payload[0] = 0x45
payload[2:4] = struct.pack("!H", 28)
payload[4:6] = b"\x00\x01"
payload[8] = 64
payload[9] = 1
payload[12:16] = bytes([127, 0, 0, 1])
payload[16:20] = bytes([127, 0, 0, 1])

checksum = 0
for i in range(0, 20, 2):
    checksum += (payload[i] << 8) + payload[i + 1]
while checksum >> 16:
    checksum = (checksum & 0xFFFF) + (checksum >> 16)
payload[10:12] = struct.pack("!H", (~checksum) & 0xFFFF)

payload[20] = 8
payload[24:26] = b"\x12\x34"
payload[26:28] = b"\x00\x01"

checksum = 0
for i in range(20, 28, 2):
    checksum += (payload[i] << 8) + payload[i + 1]
while checksum >> 16:
    checksum = (checksum & 0xFFFF) + (checksum >> 16)
payload[22:24] = struct.pack("!H", (~checksum) & 0xFFFF)

def align(length: int) -> int:
    return (length + 7) & ~7

frame = bytearray()
frame += struct.pack("!I", 0x41C90FD4)
frame += b"\x00\x00"
frame += struct.pack("!I", 0)
frame += struct.pack("!H", len(payload))
frame += payload
frame += b"\x00" * (align(len(payload)) - len(payload))

for option_id, option_value in ((2, b"eth0"), (3, b"")):
    frame += struct.pack("!II", option_id, len(option_value))
    frame += option_value
    frame += b"\x00" * (align(len(option_value)) - len(option_value))

frame[4:6] = struct.pack("!H", len(frame) - 4)
sock.sendall(frame)
sock.close()
PY

  local i
  for i in $(seq 1 50); do
    if [ -s "${json_path}" ]; then
      break
    fi
    sleep 0.1
  done

  python3 - "${json_path}" <<'PY'
import json
import sys

event = json.load(open(sys.argv[1], "r", encoding="utf-8"))
assert event["raw.pktlen"] == 28
assert event["oob.in"] == "eth0"
assert event["icmp.type"] == 8
PY
)

test_wayvnc() (
  note "Testing WayVNC JSON control client"

  local socket_path="${APP_LOG_DIR}/wayvnc-test.sock"
  local server_pid=""
  local out

  wayvnc --version >/dev/null
  rm -f "${socket_path}"

  python3 - "${socket_path}" <<'PY' &
import json
import os
import socket
import sys

path = sys.argv[1]

try:
    os.unlink(path)
except FileNotFoundError:
    pass

server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(path)
server.listen(1)
conn, _ = server.accept()
request = json.loads(conn.recv(4096).decode())
assert request["method"] == "version"
response = {
    "code": 0,
    "id": request.get("id"),
    "data": {"wayvnc": "0.7.2", "neatvnc": "0.7.1", "aml": "0.3.0"},
}
conn.sendall(json.dumps(response, separators=(",", ":")).encode())
conn.close()
server.close()
PY
  server_pid="$!"
  trap 'kill "${server_pid}" 2>/dev/null || true; wait "${server_pid}" 2>/dev/null || true; rm -f "${socket_path}"' EXIT

  wait_for_socket "${socket_path}"
  out="$(wayvncctl --json -S "${socket_path}" version)"
  printf '%s\n' "${out}" >"${APP_LOG_DIR}/wayvncctl-version.json"
  wait "${server_pid}"

  python3 - <<'PY' "${out}"
import json
import sys

data = json.loads(sys.argv[1])
assert data["wayvnc"] == "0.7.2"
assert "neatvnc" in data
assert "aml" in data
PY
)

test_webdis() (
  note "Testing Webdis JSON HTTP responses"

  local redis_port
  local http_port
  local pid=""

  redis_port="$(random_port)"
  http_port="$(random_port)"

  trap 'kill "${pid}" 2>/dev/null || true; wait "${pid}" 2>/dev/null || true; pkill -x webdis 2>/dev/null || true; redis-cli -p "${redis_port}" shutdown nosave >/dev/null 2>&1 || true' EXIT

  cat >"${APP_LOG_DIR}/webdis.json" <<EOF
{
  "redis_host": "127.0.0.1",
  "redis_port": ${redis_port},
  "http_host": "127.0.0.1",
  "http_port": ${http_port},
  "threads": 1,
  "daemonize": false,
  "database": 0,
  "verbosity": 3,
  "logfile": "${APP_LOG_DIR}/webdis.log"
}
EOF

  redis-server --save "" --appendonly no --daemonize yes --bind 127.0.0.1 --port "${redis_port}"
  webdis "${APP_LOG_DIR}/webdis.json" >"${APP_LOG_DIR}/webdis.stdout.log" 2>"${APP_LOG_DIR}/webdis.stderr.log" &
  pid="$!"

  wait_for_url "http://127.0.0.1:${http_port}/PING.json"
  curl -fsS "http://127.0.0.1:${http_port}/SET/testkey/testvalue.json" >"${APP_LOG_DIR}/webdis.set"
  curl -fsS "http://127.0.0.1:${http_port}/GET/testkey.json" >"${APP_LOG_DIR}/webdis.get"

  python3 - "${APP_LOG_DIR}/webdis.set" "${APP_LOG_DIR}/webdis.get" <<'PY'
import json
import sys

set_result = json.load(open(sys.argv[1], "r", encoding="utf-8"))
get_result = json.load(open(sys.argv[2], "r", encoding="utf-8"))

assert set_result["SET"][0] is True
assert get_result["GET"] == "testvalue"
PY
)

run_runtime_smoke_tests() {
  local runtime_status=0

  reset_log_root "${RUNTIME_LOG_ROOT}"
  : >"${RUNTIME_ISSUES_JSONL}"
  resolve_ulogd_plugins
  RUNTIME_SELECTED_COUNT=0

  if [ -n "${JANSSON_RUNTIME_APPLICATIONS}" ] || [ -n "${JANSSON_RUNTIME_CHECKS}" ]; then
    note "Restricting runtime smoke tests to applications [${JANSSON_RUNTIME_APPLICATIONS:-all}] and checks [${JANSSON_RUNTIME_CHECKS:-all}]"
  fi

  note "Verifying that each exercised binary resolves libjansson from ${SELECTED_LABEL}"

  if ! run_selected_runtime_check emacs selected-libjansson-resolution \
    "Emacs resolves the selected libjansson" \
    "ldd /usr/bin/emacs | awk '/libjansson\\.so\\.4/ { print \$3; exit }'" \
    "The exercised Emacs binary should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_emacs_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check emacs json-roundtrip \
    "Emacs JSON round-trip" \
    "emacs --batch --eval '(princ (json-serialize (json-parse-string \"{\\\"x\\\":1}\")))'" \
    "Emacs should parse and serialize JSON without changing the payload." \
    dump test_emacs; then
    runtime_status=1
  fi

  if ! run_selected_runtime_check janus selected-libjansson-resolution \
    "Janus resolves the selected libjansson" \
    "ldd /usr/bin/janus | awk '/libjansson\\.so\\.4/ { print \$3; exit }'" \
    "The exercised Janus binary should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_janus_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check janus http-json-api \
    "Janus HTTP JSON API" \
    "curl -fsS http://127.0.0.1:8088/janus/info && curl -fsS -H 'Content-Type: application/json' -d '{\"janus\":\"create\",\"transaction\":\"txn1\"}' http://127.0.0.1:8088/janus" \
    "Janus should serve JSON info and create-session responses that parse correctly." \
    load test_janus; then
    runtime_status=1
  fi

  if ! run_selected_runtime_check jshon selected-libjansson-resolution \
    "jshon resolves the selected libjansson" \
    "ldd /usr/bin/jshon | awk '/libjansson\\.so\\.4/ { print \$3; exit }'" \
    "The exercised jshon binary should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_jshon_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check jshon cli-parse \
    "jshon parses array members" \
    "printf '{\"foo\":1,\"bar\":[2,3]}' | jshon -e bar -a -u" \
    "jshon should emit the expected array members from parsed JSON input." \
    load test_jshon; then
    runtime_status=1
  fi

  if ! run_selected_runtime_check jose selected-libjansson-resolution \
    "jose resolves the selected libjansson" \
    "ldd /usr/bin/jose | awk '/libjansson\\.so\\.4/ { print \$3; exit }'" \
    "The exercised jose binary should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_jose_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check jose jwk-generation \
    "jose generates a JWK" \
    "jose jwk gen -i '{\"alg\":\"ES256\"}' -o jose.jwk" \
    "jose should generate a JSON JWK whose parsed fields match the requested algorithm." \
    object test_jose; then
    runtime_status=1
  fi

  if ! run_selected_runtime_check mtr selected-libjansson-resolution \
    "mtr resolves the selected libjansson" \
    "ldd /usr/bin/mtr | awk '/libjansson\\.so\\.4/ { print \$3; exit }'" \
    "The exercised mtr binary should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_mtr_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check mtr json-report \
    "mtr emits a JSON report" \
    "mtr -r -j -n -c 1 127.0.0.1" \
    "mtr should emit JSON output whose report.hubs entry contains 127.0.0.1." \
    dump test_mtr; then
    runtime_status=1
  fi

  if ! run_selected_runtime_check nghttp2 selected-libjansson-resolution \
    "nghttp resolves the selected libjansson" \
    "ldd /usr/bin/nghttp | awk '/libjansson\\.so\\.4/ { print \$3; exit }'" \
    "The exercised nghttp binary should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_nghttp2_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check nghttp2 har-json-structure \
    "nghttp emits a HAR document with a top-level log object" \
    "nghttp -ans --har=capture.har http://127.0.0.1:<port>/index.json" \
    "nghttp should write a HAR file that parses as JSON and contains a top-level log object with entries." \
    dump test_nghttp2; then
    runtime_status=1
  fi

  if ! run_selected_runtime_check suricata selected-libjansson-resolution \
    "Suricata resolves the selected libjansson" \
    "ldd /usr/bin/suricata | awk '/libjansson\\.so\\.4/ { print \$3; exit }'" \
    "The exercised Suricata binary should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_suricata_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check suricata eve-json \
    "Suricata emits EVE JSON" \
    "suricata -r suricata-test.pcap -l suricata-out -c /etc/suricata/suricata.yaml" \
    "Suricata should emit EVE JSON events containing a flow event." \
    dump test_suricata; then
    runtime_status=1
  fi

  if ! run_selected_runtime_check tang selected-libjansson-resolution \
    "Tang resolves the selected libjansson" \
    "ldd /usr/libexec/tangd | awk '/libjansson\\.so\\.4/ { print \$3; exit }'" \
    "The exercised Tang daemon should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_tang_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check tang advertisement-and-recovery \
    "Tang advertises and recovers JWK material" \
    "curl -fsS http://127.0.0.1:<port>/adv && curl -fsS -X POST -H 'Content-Type: application/jwk+json' --data-binary @exc.pub.jwk http://127.0.0.1:<port>/rec/<kid>" \
    "Tang should serve a signed advertisement and return the expected recovery payload." \
    object test_tang; then
    runtime_status=1
  fi

  if ! run_selected_runtime_check libteam selected-libjansson-resolution \
    "teamd and teamdctl resolve the selected libjansson" \
    "ldd /usr/bin/teamd && ldd /usr/bin/teamdctl" \
    "The exercised teamd and teamdctl binaries should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_libteam_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check libteam teamdctl-state-dump \
    "teamd parses config and teamdctl parses JSON state" \
    "teamd -t team0 -n -U -c '{\"runner\":{\"name\":\"activebackup\"}}' && teamdctl -U lo state dump" \
    "teamd should parse its JSON config and teamdctl should parse the JSON state dump." \
    load test_teamd; then
    runtime_status=1
  fi

  if ! run_selected_runtime_check ulogd2 selected-libjansson-resolution \
    "ulogd JSON plugin resolves the selected libjansson" \
    "ldd ${ULOGD_OUTPUT_PLUGIN} | awk '/libjansson\\.so\\.4/ { print \$3; exit }'" \
    "The exercised ulogd JSON plugin should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_ulogd_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check ulogd2 json-output-plugin \
    "ulogd JSON plugin writes a parsed event" \
    "ulogd -v -c ulogd-test.conf" \
    "ulogd should write a JSON event whose parsed packet fields match the injected payload." \
    dump test_ulogd; then
    runtime_status=1
  fi

  if ! run_selected_runtime_check wayvnc selected-libjansson-resolution \
    "wayvnc and wayvncctl resolve the selected libjansson" \
    "ldd /usr/bin/wayvnc && ldd /usr/bin/wayvncctl" \
    "The exercised wayvnc and wayvncctl binaries should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_wayvnc_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check wayvnc json-control \
    "wayvncctl parses JSON control output" \
    "wayvncctl --json -S wayvnc-test.sock version" \
    "wayvncctl should parse the JSON response from the control socket." \
    load test_wayvnc; then
    runtime_status=1
  fi

  if ! run_selected_runtime_check webdis selected-libjansson-resolution \
    "webdis resolves the selected libjansson" \
    "ldd /usr/bin/webdis | awk '/libjansson\\.so\\.4/ { print \$3; exit }'" \
    "The exercised webdis binary should resolve libjansson.so.4 from ${SELECTED_LABEL}." \
    linker check_webdis_resolution; then
    runtime_status=1
  fi
  if ! run_selected_runtime_check webdis json-http-response \
    "webdis serves JSON HTTP responses" \
    "curl -fsS http://127.0.0.1:<port>/SET/testkey/testvalue.json && curl -fsS http://127.0.0.1:<port>/GET/testkey.json" \
    "webdis should expose Redis writes and reads through JSON HTTP responses." \
    dump test_webdis; then
    runtime_status=1
  fi

  if [ "${RUNTIME_SELECTED_COUNT}" -eq 0 ]; then
    fail "Runtime filters selected no checks to execute"
  fi

  if [ "${runtime_status}" -eq 0 ]; then
    note "All dependent smoke tests passed against ${SELECTED_LABEL}"
  else
    note "Dependent smoke tests recorded one or more failures against ${SELECTED_LABEL}"
  fi

  return "${runtime_status}"
}

update_discovered_issues_inventory() {
  local executed_phases_csv="$1"
  local log_roots_csv="$2"
  shift 2

  python3 - "${ISSUE_FILE}" "${MATRIX_RUN_STARTED_AT}" "${JANSSON_IMPLEMENTATION}" "${JANSSON_TEST_MODE}" \
    "${executed_phases_csv}" "${log_roots_csv}" "$@" <<'PY'
from pathlib import Path
import json
import re
import sys

issue_file = Path(sys.argv[1])
timestamp = sys.argv[2]
implementation = sys.argv[3]
requested_mode = sys.argv[4]
executed_phases = [item for item in sys.argv[5].split(",") if item]
log_roots = [item for item in sys.argv[6].split(",") if item]
issue_paths = [Path(item) for item in sys.argv[7:]]


def slug(value: str) -> str:
    text = re.sub(r"[^A-Z0-9]+", "-", value.upper()).strip("-")
    return text or "UNKNOWN"


def sanitize(value: str) -> str:
    text = " ".join(value.replace("\r", "\n").split())
    return text[:1000] if len(text) > 1000 else text


def parse_existing(path: Path):
    if not path.exists():
        return {}

    text = path.read_text(encoding="utf-8")
    pattern = re.compile(r"^## (APP-[A-Z0-9-]+)\n(.*?)(?=^## APP-|\Z)", re.M | re.S)
    entries = {}

    for issue_id, body in pattern.findall(text):
        meta_match = re.search(r"^<!-- dependent-matrix: (.+) -->$", body, re.M)
        metadata = json.loads(meta_match.group(1)) if meta_match else {}
        fields = {}
        for label, value in re.findall(r"^- ([^:]+): (.*)$", body, re.M):
            key = label.lower().replace(" ", "_").replace("(", "").replace(")", "")
            fields[key] = value.strip()

        entry = {
            "issue_id": issue_id,
            "phase": metadata.get("phase", fields.get("phase", "")),
            "application": metadata.get("application", fields.get("application", "")),
            "check": metadata.get("check", fields.get("check", "")),
            "title": fields.get("title", ""),
            "command": fields.get("failing_command", ""),
            "expected_behavior": fields.get("expected_behavior", ""),
            "observed_behavior": fields.get("observed_behavior", ""),
            "suspected_subsystem": metadata.get("suspected_subsystem", fields.get("suspected_subsystem", "")),
            "log_path": fields.get("log_path", ""),
            "first_seen_utc": fields.get("first_seen_utc", ""),
            "last_seen_utc": fields.get("last_seen_utc", ""),
            "current_status": fields.get("current_status", ""),
        }
        entries[issue_id] = entry

    return entries


def key_for(entry):
    return (
        entry.get("phase", ""),
        entry.get("application", ""),
        entry.get("check", ""),
        entry.get("suspected_subsystem", ""),
    )


existing = parse_existing(issue_file)
key_to_issue_id = {}
prefix_counters = {}

for issue_id, entry in existing.items():
    if all(key_for(entry)):
        key_to_issue_id[key_for(entry)] = issue_id
    prefix, _, suffix = issue_id.rpartition("-")
    if suffix.isdigit():
        prefix_counters[prefix] = max(prefix_counters.get(prefix, 0), int(suffix))


current_records = []
for path in issue_paths:
    if not path.exists():
        continue
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        current_records.append(json.loads(line))


merged = dict(existing)
seen_this_run = set()

for record in current_records:
    record = {key: sanitize(str(value)) for key, value in record.items()}
    issue_key = key_for(record)
    issue_id = key_to_issue_id.get(issue_key)
    if issue_id is None:
        prefix = f"APP-{slug(record['application'])}-{slug(record['suspected_subsystem'])}"
        prefix_counters[prefix] = prefix_counters.get(prefix, 0) + 1
        issue_id = f"{prefix}-{prefix_counters[prefix]:03d}"
        key_to_issue_id[issue_key] = issue_id

    previous = merged.get(issue_id, {})
    merged[issue_id] = {
        "issue_id": issue_id,
        "phase": record["phase"],
        "application": record["application"],
        "check": record["check"],
        "title": record["title"],
        "command": record["command"],
        "expected_behavior": record["expected_behavior"],
        "observed_behavior": record["observed_behavior"],
        "suspected_subsystem": record["suspected_subsystem"],
        "log_path": record["log_path"],
        "first_seen_utc": previous.get("first_seen_utc", timestamp) or timestamp,
        "last_seen_utc": timestamp,
        "current_status": "open",
    }
    seen_this_run.add(issue_id)


for issue_id, entry in list(merged.items()):
    if issue_id in seen_this_run:
        continue
    if entry.get("phase") in executed_phases:
        entry["current_status"] = f"not reproduced in latest {entry.get('phase')} run"
    entry["title"] = sanitize(entry.get("title", ""))
    entry["command"] = sanitize(entry.get("command", ""))
    entry["expected_behavior"] = sanitize(entry.get("expected_behavior", ""))
    entry["observed_behavior"] = sanitize(entry.get("observed_behavior", ""))
    entry["log_path"] = sanitize(entry.get("log_path", ""))


issue_file.parent.mkdir(parents=True, exist_ok=True)

lines = [
    "# Discovered Application-Level Issues",
    "",
    "This file is updated in place by the dependent-matrix runners. Existing issue IDs are preserved when the same phase/application/check/subsystem incompatibility reappears.",
    "",
    "## Latest Run",
    f"- Timestamp (UTC): {timestamp}",
    f"- Implementation: {implementation}",
    f"- Requested mode: {requested_mode}",
    f"- Executed phases: {', '.join(executed_phases) if executed_phases else 'none'}",
]

if current_records:
    current_ids = sorted(seen_this_run)
    lines.append(f"- Result: {len(current_records)} application-level regression(s) reproduced in the latest run: {', '.join(current_ids)}")
else:
    lines.append("- Result: No application-level regressions found in the latest run.")

lines.append(f"- Log roots: {', '.join(log_roots) if log_roots else 'none'}")
lines.append("")

if not merged:
    lines.append("No application-level regressions found.")
else:
    for issue_id in sorted(merged):
        entry = merged[issue_id]
        metadata = {
            "phase": entry.get("phase", ""),
            "application": entry.get("application", ""),
            "check": entry.get("check", ""),
            "suspected_subsystem": entry.get("suspected_subsystem", ""),
        }
        lines.extend(
            [
                f"## {issue_id}",
                f"<!-- dependent-matrix: {json.dumps(metadata, sort_keys=True)} -->",
                f"- Current status: {entry.get('current_status', '')}",
                f"- Title: {entry.get('title', '')}",
                f"- Phase: {entry.get('phase', '')}",
                f"- Application: {entry.get('application', '')}",
                f"- Check: {entry.get('check', '')}",
                f"- First seen UTC: {entry.get('first_seen_utc', '')}",
                f"- Last seen UTC: {entry.get('last_seen_utc', '')}",
                f"- Failing command: {entry.get('command', '')}",
                f"- Expected behavior: {entry.get('expected_behavior', '')}",
                f"- Observed behavior: {entry.get('observed_behavior', '')}",
                f"- Suspected subsystem: {entry.get('suspected_subsystem', '')}",
                f"- Log path: {entry.get('log_path', '')}",
                "",
            ]
        )

issue_file.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
PY
}

mkdir -p "${ROOT_DIR}/safe/.build" "${ROOT_DIR}/safe/tests/regressions"

install_mode_packages

if [ "${JANSSON_IMPLEMENTATION}" = "safe" ]; then
  resolve_safe_packages
  install_safe_jansson
fi

build_status=0
runtime_status=0
executed_phases=()
log_roots=()
issue_files=()

if [ "${RUN_BUILD}" -eq 1 ]; then
  executed_phases+=(build)
  log_roots+=("$(relative_path "${BUILD_LOG_ROOT}")")
  issue_files+=("${BUILD_ISSUES_JSONL}")
  set +e
  run_build_harness
  build_status=$?
  set -e
fi

if [ "${RUN_RUNTIME}" -eq 1 ]; then
  executed_phases+=(runtime)
  log_roots+=("$(relative_path "${RUNTIME_LOG_ROOT}")")
  issue_files+=("${RUNTIME_ISSUES_JSONL}")
  if [ "${JANSSON_IMPLEMENTATION}" = "original" ]; then
    install_original_jansson
  fi
  set +e
  run_runtime_smoke_tests
  runtime_status=$?
  set -e
fi

executed_phases_csv=
if [ "${#executed_phases[@]}" -gt 0 ]; then
  executed_phases_csv="$(printf '%s,' "${executed_phases[@]}")"
  executed_phases_csv="${executed_phases_csv%,}"
fi

log_roots_csv=
if [ "${#log_roots[@]}" -gt 0 ]; then
  log_roots_csv="$(printf '%s,' "${log_roots[@]}")"
  log_roots_csv="${log_roots_csv%,}"
fi

update_discovered_issues_inventory "${executed_phases_csv}" "${log_roots_csv}" "${issue_files[@]}"

if [ "${build_status}" -ne 0 ] || [ "${runtime_status}" -ne 0 ]; then
  exit 1
fi
