ARG BASE_IMAGE=ubuntu:24.04
FROM ${BASE_IMAGE}

ARG JANSSON_IMPLEMENTATION=safe
ARG DEPENDENT_BINARY_PACKAGES
ARG HELPER_BINARY_PACKAGES=nghttp2-server

SHELL ["/bin/bash", "-euxo", "pipefail", "-c"]

COPY safe-dist/ /tmp/safe-dist/

RUN export DEBIAN_FRONTEND=noninteractive \
 && printf '#!/bin/sh\nexit 101\n' >/usr/sbin/policy-rc.d \
 && chmod 0755 /usr/sbin/policy-rc.d \
 && read -r -a dependent_packages <<<"${DEPENDENT_BINARY_PACKAGES}" \
 && read -r -a helper_packages <<<"${HELPER_BINARY_PACKAGES}" \
 && base_packages=( \
      autoconf \
      automake \
      build-essential \
      ca-certificates \
      cargo \
      curl \
      dpkg-dev \
      iproute2 \
      jq \
      libtool \
      procps \
      python3 \
      redis-server \
      rustc \
      ulogd2 \
    ) \
 && declare -A seen=() \
 && install_packages=() \
 && add_packages() { \
      local pkg; \
      for pkg in "$@"; do \
        [ -n "${pkg}" ] || continue; \
        if [ -n "${seen[$pkg]:-}" ]; then \
          continue; \
        fi; \
        seen["$pkg"]=1; \
        install_packages+=("$pkg"); \
      done; \
    } \
 && add_packages "${base_packages[@]}" \
 && add_packages libjansson4 libjansson-dev \
 && add_packages "${dependent_packages[@]}" \
 && add_packages "${helper_packages[@]}" \
 && apt-get update \
 && apt-get install -y --no-install-recommends "${install_packages[@]}" \
 && case "${JANSSON_IMPLEMENTATION}" in \
      original) \
        ;; \
      safe) \
        runtime_deb="$(find /tmp/safe-dist -maxdepth 1 -type f -name 'libjansson4_*.deb' | sort | tail -n 1)" \
        && dev_deb="$(find /tmp/safe-dist -maxdepth 1 -type f -name 'libjansson-dev_*.deb' | sort | tail -n 1)" \
        && [ -n "${runtime_deb}" ] \
        && [ -n "${dev_deb}" ] \
        && dpkg -i "${runtime_deb}" "${dev_deb}" \
        ;; \
      *) \
        printf 'ERROR: Unsupported JANSSON_IMPLEMENTATION=%s (expected original or safe)\n' "${JANSSON_IMPLEMENTATION}" >&2 \
        && exit 1 \
        ;; \
    esac \
 && ldconfig \
 && rm -f /usr/sbin/policy-rc.d \
 && rm -rf /var/lib/apt/lists/* /tmp/safe-dist
