#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_IMAGE="${DOCKER_IMAGE:-ubuntu:24.04}"

docker run --rm -i \
  -v "${ROOT_DIR}:/work" \
  -w /work \
  "${DOCKER_IMAGE}" \
  bash -s -- <<'CONTAINER'
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

note() {
  printf '\n==> %s\n' "$1"
}

fail() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
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

assert_uses_local_jansson() {
  local bin="$1"
  local resolved

  resolved="$(ldd "${bin}" | awk '/libjansson\.so\.4/ { print $3; exit }')"
  [ -n "${resolved}" ] || fail "${bin} does not resolve libjansson.so.4"
  [ "${resolved}" = "${LOCAL_JANSSON}" ] || fail "${bin} resolved libjansson.so.4 to ${resolved}, expected ${LOCAL_JANSSON}"
}

note "Installing toolchain and dependent packages"
apt-get update
apt-get install -y --no-install-recommends \
  autoconf \
  automake \
  build-essential \
  ca-certificates \
  curl \
  emacs-nox \
  iproute2 \
  janus \
  jose \
  jshon \
  libteam-utils \
  mtr-tiny \
  procps \
  python3 \
  redis-server \
  suricata \
  tang-common \
  ulogd2 \
  ulogd2-json \
  libtool \
  wayvnc \
  webdis

note "Building and installing the original Jansson source"
rm -rf /tmp/jansson-src
cp -a /work/original/jansson-2.14 /tmp/jansson-src
(
  cd /tmp/jansson-src
  autoreconf -fi
  ./configure --prefix=/usr/local
  make -j"$(getconf _NPROCESSORS_ONLN)"
  make install
)
ldconfig

LOCAL_JANSSON="$(find /usr/local -name 'libjansson.so.4' -type l | head -n 1)"
[ -n "${LOCAL_JANSSON}" ] || fail "Failed to find the installed /usr/local libjansson.so.4"
LOCAL_LIBDIR="$(dirname "${LOCAL_JANSSON}")"
export LD_LIBRARY_PATH="${LOCAL_LIBDIR}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"

note "Verifying that each exercised binary resolves libjansson from /usr/local"
for bin in \
  /usr/bin/emacs \
  /usr/bin/janus \
  /usr/bin/jose \
  /usr/bin/jshon \
  /usr/bin/mtr \
  /usr/bin/suricata \
  /usr/libexec/tangd \
  /usr/bin/teamd \
  /usr/bin/teamdctl \
  /usr/lib/x86_64-linux-gnu/ulogd/ulogd_output_JSON.so \
  /usr/bin/wayvnc \
  /usr/bin/wayvncctl \
  /usr/bin/webdis; do
  assert_uses_local_jansson "${bin}"
done

test_emacs() {
  note "Testing Emacs JSON support"
  local out

  out="$(emacs --batch --eval '(princ (json-serialize (json-parse-string "{\"x\":1}")))' 2>/dev/null)"
  [ "${out}" = '{"x":1}' ] || fail "Emacs JSON round-trip failed: ${out}"
}

test_janus() (
  note "Testing Janus HTTP JSON API"

  local pid=""
  local log="/tmp/janus.log"
  trap 'kill "${pid}" 2>/dev/null || true; wait "${pid}" 2>/dev/null || true' EXIT

  janus -F /etc/janus >"${log}" 2>&1 &
  pid="$!"

  wait_for_url "http://127.0.0.1:8088/janus/info"
  curl -fsS http://127.0.0.1:8088/janus/info >/tmp/janus-info.json
  curl -fsS \
    -H 'Content-Type: application/json' \
    -d '{"janus":"create","transaction":"txn1"}' \
    http://127.0.0.1:8088/janus >/tmp/janus-create.json

  python3 - /tmp/janus-info.json /tmp/janus-create.json <<'PY'
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
  [ "${out}" = $'2\n3' ] || fail "jshon returned unexpected output: ${out}"
}

test_jose() {
  note "Testing jose JSON/JWK handling"

  jose jwk gen -i '{"alg":"ES256"}' -o /tmp/jose.jwk >/dev/null
  python3 - /tmp/jose.jwk <<'PY'
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

  mtr -r -j -n -c 1 127.0.0.1 >/tmp/mtr.json
  python3 - /tmp/mtr.json <<'PY'
import json
import sys

report = json.load(open(sys.argv[1], "r", encoding="utf-8"))
hubs = report["report"]["hubs"]
assert hubs
assert hubs[0]["host"] == "127.0.0.1"
PY
}

test_suricata() {
  note "Testing Suricata EVE JSON logging"

  rm -rf /tmp/suricata-out
  mkdir -p /tmp/suricata-out

  python3 - <<'PY'
import struct
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

with open("/tmp/suricata-test.pcap", "wb") as handle:
    handle.write(pcap)
PY

  suricata \
    -r /tmp/suricata-test.pcap \
    -l /tmp/suricata-out \
    -c /etc/suricata/suricata.yaml \
    >/tmp/suricata.stdout 2>/tmp/suricata.stderr

  python3 - /tmp/suricata-out/eve.json <<'PY'
import json
import sys

events = [json.loads(line) for line in open(sys.argv[1], "r", encoding="utf-8")]
assert any(event.get("event_type") == "flow" for event in events)
PY
}

test_tang() (
  note "Testing Tang advertisement and recovery endpoints"

  local tmpdir
  local port
  local pid=""
  local exc_kid
  local template
  local good
  local reply

  tmpdir="$(mktemp -d)"
  trap 'kill "${pid}" 2>/dev/null || true; wait "${pid}" 2>/dev/null || true; rm -rf "${tmpdir}"' EXIT

  mkdir -p "${tmpdir}/db"
  /usr/libexec/tangd-keygen "${tmpdir}/db" sig exc >/dev/null

  exc_kid="$(jose jwk thp -i "${tmpdir}/db/exc.jwk")"
  template="$(jose fmt -j "${tmpdir}/db/exc.jwk" -Od x -d y -d d -o-)"
  jose jwk gen -i "${template}" -o "${tmpdir}/exc.jwk" >/dev/null
  jose jwk pub -i "${tmpdir}/exc.jwk" -o "${tmpdir}/exc.pub.jwk" >/dev/null

  port="$(random_port)"
  /usr/libexec/tangd -l -p "${port}" "${tmpdir}/db" >/tmp/tangd.log 2>&1 &
  pid="$!"

  wait_for_url "http://127.0.0.1:${port}/adv"
  curl -fsS "http://127.0.0.1:${port}/adv" >"${tmpdir}/adv.jose"
  jose jws ver -i "${tmpdir}/adv.jose" -k "${tmpdir}/db/sig.jwk" >/dev/null

  good="$(jose jwk exc -i '{"alg":"ECMR","key_ops":["deriveKey"]}' -l "${tmpdir}/exc.jwk" -r "${tmpdir}/db/exc.jwk")"
  reply="$(curl -fsS -X POST \
    -H 'Content-Type: application/jwk+json' \
    --data-binary @"${tmpdir}/exc.pub.jwk" \
    "http://127.0.0.1:${port}/rec/${exc_kid}")"

  [ "${good}" = "${reply}" ] || fail "Tang recovery response did not match the expected exchanged key"
)

test_teamd() (
  note "Testing teamd JSON config parsing and teamdctl JSON dump parsing"

  local teamd_log="/tmp/teamd.log"
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
  trap 'kill "${pid}" 2>/dev/null || true; wait "${pid}" 2>/dev/null || true' EXIT

  cat >/tmp/ulogd-test.conf <<'EOF'
[global]
logfile="stdout"
loglevel=3
plugin="/usr/lib/x86_64-linux-gnu/ulogd/ulogd_inppkt_UNIXSOCK.so"
plugin="/usr/lib/x86_64-linux-gnu/ulogd/ulogd_raw2packet_BASE.so"
plugin="/usr/lib/x86_64-linux-gnu/ulogd/ulogd_output_JSON.so"
stack=us1:UNIXSOCK,base1:BASE,json1:JSON

[us1]
socket_path="/tmp/ulogd-test.sock"

[json1]
sync=1
file="/tmp/ulogd-test.json"
EOF

  rm -f /tmp/ulogd-test.sock /tmp/ulogd-test.json
  ulogd -v -c /tmp/ulogd-test.conf >/tmp/ulogd-test.log 2>&1 &
  pid="$!"

  wait_for_socket /tmp/ulogd-test.sock
  python3 - <<'PY'
import socket
import struct

path = "/tmp/ulogd-test.sock"
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
    if [ -s /tmp/ulogd-test.json ]; then
      break
    fi
    sleep 0.1
  done

  python3 - /tmp/ulogd-test.json <<'PY'
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

  local socket_path="/tmp/wayvnc-test.sock"
  local server_pid=""
  local out

  wayvnc --version >/dev/null
  rm -f "${socket_path}"

  python3 - <<'PY' &
import json
import os
import socket

path = "/tmp/wayvnc-test.sock"

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
  local i

  redis_port="$(random_port)"
  http_port="$(random_port)"

  trap 'kill "${pid}" 2>/dev/null || true; wait "${pid}" 2>/dev/null || true; pkill -x webdis 2>/dev/null || true; redis-cli -p "${redis_port}" shutdown nosave >/dev/null 2>&1 || true' EXIT

  cat >/tmp/webdis.json <<EOF
{
  "redis_host": "127.0.0.1",
  "redis_port": ${redis_port},
  "http_host": "127.0.0.1",
  "http_port": ${http_port},
  "threads": 1,
  "daemonize": false,
  "database": 0,
  "verbosity": 3,
  "logfile": "/tmp/webdis.log"
}
EOF

  redis-server --save "" --appendonly no --daemonize yes --bind 127.0.0.1 --port "${redis_port}"
  webdis /tmp/webdis.json >/tmp/webdis.stdout 2>/tmp/webdis.stderr &
  pid="$!"

  wait_for_url "http://127.0.0.1:${http_port}/PING.json"
  curl -fsS "http://127.0.0.1:${http_port}/SET/testkey/testvalue.json" >/tmp/webdis.set
  curl -fsS "http://127.0.0.1:${http_port}/GET/testkey.json" >/tmp/webdis.get

  python3 - /tmp/webdis.set /tmp/webdis.get <<'PY'
import json
import sys

set_result = json.load(open(sys.argv[1], "r", encoding="utf-8"))
get_result = json.load(open(sys.argv[2], "r", encoding="utf-8"))

assert set_result["SET"][0] is True
assert get_result["GET"] == "testvalue"
PY
)

test_emacs
test_janus
test_jshon
test_jose
test_mtr
test_suricata
test_tang
test_teamd
test_ulogd
test_wayvnc
test_webdis

note "All dependent smoke tests passed against the original Jansson build"
CONTAINER
