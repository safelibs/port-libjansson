#!/usr/bin/env bash
set -euo pipefail

CASE_NAME="nghttp2-image-selected-libjansson-resolution"
CHECK_NAME="selected-libjansson-resolution"

. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/image-nghttp2-common.sh"
