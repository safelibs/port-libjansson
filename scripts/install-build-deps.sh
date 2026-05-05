#!/usr/bin/env bash
# Install apt packages needed to run safe/scripts/build-deb.sh for the
# safe libjansson port.
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  autoconf \
  automake \
  build-essential \
  cargo \
  ca-certificates \
  dpkg-dev \
  fakeroot \
  file \
  git \
  jq \
  libtool \
  pkg-config \
  python3 \
  rsync \
  rustc \
  xz-utils
