#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MODE="post-fix"
MANIFEST_PATH="${ROOT_DIR}/safe/tests/regressions/manifest.json"
ISSUE_FILE="${ROOT_DIR}/safe/tests/regressions/discovered-issues.md"
CASE_FILTER=

usage() {
  cat <<'EOF' >&2
Usage: safe/scripts/run-regressions.sh [--mode pre-fix|post-fix] [--pre-fix] [--post-fix] [--manifest PATH] [--case SUBSTRING]
EOF
  exit 2
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --mode)
      [ "$#" -ge 2 ] || usage
      MODE="$2"
      shift 2
      ;;
    --pre-fix)
      MODE="pre-fix"
      shift
      ;;
    --post-fix)
      MODE="post-fix"
      shift
      ;;
    --manifest)
      [ "$#" -ge 2 ] || usage
      MANIFEST_PATH="$2"
      shift 2
      ;;
    --case)
      [ "$#" -ge 2 ] || usage
      CASE_FILTER="$2"
      shift 2
      ;;
    --help|-h)
      usage
      ;;
    *)
      usage
      ;;
  esac
done

case "${MODE}" in
  pre-fix|post-fix)
    ;;
  *)
    printf 'ERROR: Unsupported mode %s (expected pre-fix or post-fix)\n' "${MODE}" >&2
    exit 2
    ;;
esac

RUN_STAMP="${REGRESSION_RUN_STAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
RUN_ROOT_DEFAULT="${ROOT_DIR}/safe/.build/regressions/${RUN_STAMP}"
export REGRESSION_RUN_ROOT="${REGRESSION_RUN_ROOT:-${RUN_ROOT_DEFAULT}}"
export REGRESSION_IMAGE_TAG="${REGRESSION_IMAGE_TAG:-libjansson-regressions:safe}"

mkdir -p "${REGRESSION_RUN_ROOT}"

python3 - "${ROOT_DIR}" "${MANIFEST_PATH}" "${ISSUE_FILE}" "${MODE}" "${CASE_FILTER}" <<'PY'
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

root_dir = Path(sys.argv[1]).resolve()
manifest_path = Path(sys.argv[2]).resolve()
issue_file = Path(sys.argv[3]).resolve()
mode = sys.argv[4]
case_filter = sys.argv[5]
run_root = Path(os.environ["REGRESSION_RUN_ROOT"]).resolve()
image_tag = os.environ["REGRESSION_IMAGE_TAG"]


def fail(message: str) -> None:
    print(f"ERROR: {message}", file=sys.stderr)
    raise SystemExit(1)


if not manifest_path.is_file():
    fail(f"missing manifest: {manifest_path}")
if not issue_file.is_file():
    fail(f"missing discovered-issues file: {issue_file}")

cases = json.loads(manifest_path.read_text(encoding="utf-8"))
if not isinstance(cases, list):
    fail("manifest must be a JSON array")

normalized_cases = []
for index, case in enumerate(cases):
    if not isinstance(case, dict):
        fail(f"manifest entry {index} is not an object")
    runner = case.get("runner")
    path = case.get("path")
    status_before_fix = case.get("status_before_fix")
    issue_id = case.get("issue_id")
    if runner not in {"c", "rust", "shell", "image"}:
        fail(f"manifest entry {index} has unsupported runner {runner!r}")
    if not isinstance(path, str) or not path:
        fail(f"manifest entry {index} is missing a non-empty path")
    if status_before_fix not in {"failing", "passing", "not_applicable"}:
        fail(f"manifest entry {index} has unsupported status_before_fix {status_before_fix!r}")
    if issue_id is not None and (not isinstance(issue_id, str) or not issue_id):
        fail(f"manifest entry {index} has invalid issue_id {issue_id!r}")
    normalized_cases.append(case)

issue_ids = sorted(set(re.findall(r"^## (APP-[A-Z0-9-]+)\s*$", issue_file.read_text(encoding="utf-8"), re.M)))
covered_issue_ids = {case["issue_id"] for case in normalized_cases if case.get("issue_id")}
missing_issue_ids = [issue_id for issue_id in issue_ids if issue_id not in covered_issue_ids]
if missing_issue_ids:
    fail(
        "missing regression coverage for discovered issues: "
        + ", ".join(missing_issue_ids)
    )

if not issue_ids:
    has_nghttp2_image_case = any(
        case["runner"] == "image" and "nghttp2" in case["path"] for case in normalized_cases
    )
    if not has_nghttp2_image_case:
        fail("zero-issue inventory still requires at least one checked-in nghttp2 image regression case")

selected_cases = normalized_cases
if case_filter:
    selected_cases = [
        case
        for case in normalized_cases
        if case_filter in case["path"] or case_filter == case.get("issue_id")
    ]
    if not selected_cases:
        fail(f"--case {case_filter!r} did not match any manifest entry")

if any(case["runner"] == "image" for case in selected_cases):
    rebuild_image = os.environ.get("REGRESSION_REBUILD_IMAGE") == "1"
    image_present = False
    if not rebuild_image:
        image_present = subprocess.run(
            ["docker", "image", "inspect", image_tag],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        ).returncode == 0
    if rebuild_image or not image_present:
        subprocess.run(
            [
                str(root_dir / "safe/scripts/build-dependent-image.sh"),
                "--implementation",
                "safe",
                "--tag",
                image_tag,
            ],
            check=True,
            cwd=root_dir,
        )

summary = []
for case in selected_cases:
    case_path = (root_dir / case["path"]).resolve()
    if not case_path.is_file():
        fail(f"manifest case path does not exist: {case['path']}")

    runner = case["runner"]
    if runner not in {"shell", "image"}:
        fail(f"runner {runner!r} is not yet supported by safe/scripts/run-regressions.sh")

    expected_status = case["status_before_fix"] if mode == "pre-fix" else "passing"
    if mode == "pre-fix" and expected_status == "not_applicable":
        print(f"SKIP {case['path']} (status_before_fix=not_applicable)")
        summary.append((case["path"], "skipped"))
        continue

    env = os.environ.copy()
    env["REGRESSION_RUN_ROOT"] = str(run_root)
    env["REGRESSION_IMAGE_TAG"] = image_tag

    result = subprocess.run(
        ["bash", str(case_path)],
        cwd=root_dir,
        env=env,
        check=False,
    )

    if expected_status == "passing":
        ok = result.returncode == 0
    elif expected_status == "failing":
        ok = result.returncode != 0
    else:
        ok = False

    if not ok:
        fail(
            f"case {case['path']} exited {result.returncode}, expected {expected_status} in {mode} mode"
        )

    print(f"PASS {case['path']} ({mode} expected {expected_status})")
    summary.append((case["path"], expected_status))

executed = [item for item in summary if item[1] != "skipped"]
print(
    f"Completed {len(executed)} regression case(s); issue coverage is complete for {len(issue_ids)} discovered issue(s)."
)
PY
