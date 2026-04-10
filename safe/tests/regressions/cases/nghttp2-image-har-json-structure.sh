#!/usr/bin/env bash
set -euo pipefail

CASE_NAME="nghttp2-image-har-json-structure"
CHECK_NAME="har-json-structure"

. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/image-nghttp2-common.sh"
