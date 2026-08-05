# syntax=docker/dockerfile:1.7
# Keep this digest and .devcontainer/bootstrap-lock.json in the same review.
FROM node:20.19.4-bookworm-slim@sha256:6db5e436948af8f0244488a1f658c2c8e55a3ae51ca2e1686ed042be8f25f70a AS codex-bootstrap

COPY .devcontainer/bootstrap-lock.json /tmp/bootstrap-lock.json

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl jq openssl \
  && install_npm_tarball() { \
    name="$1"; \
    tarball="$(jq -r ".packages.${name}.tarball" /tmp/bootstrap-lock.json)"; \
    expected_integrity="$(jq -r ".packages.${name}.integrity" /tmp/bootstrap-lock.json)"; \
    curl --fail --location --silent --show-error "${tarball}" --output "/tmp/${name}.tgz"; \
    actual_integrity="sha512-$(openssl dgst -sha512 -binary "/tmp/${name}.tgz" | base64 -w 0)"; \
    test "${actual_integrity}" = "${expected_integrity}"; \
    npm install --global --ignore-scripts --no-audit --no-fund "/tmp/${name}.tgz"; \
  }; \
  install_npm_tarball codex; \
  install_npm_tarball pnpm; \
  rm -rf /var/lib/apt/lists/* /tmp/codex.tgz /tmp/pnpm.tgz

FROM rust:1.89.0-bookworm@sha256:948f9b08a66e7fe01b03a98ef1c7568292e07ec2e4fe90d88c07bb14563c84ff AS rust-bootstrap

RUN rustup component add rustfmt clippy

# The base-image digest is recorded in .devcontainer/bootstrap-lock.json.
FROM mcr.microsoft.com/devcontainers/base:bookworm@sha256:73d85a96694a2cadca1ba3fcb5721f2312a64f1d571dd86f6c77e10a708931dc

COPY --from=codex-bootstrap /usr/local /usr/local
COPY --from=rust-bootstrap /usr/local/cargo /usr/local/cargo
COPY --from=rust-bootstrap /usr/local/rustup /usr/local/rustup
COPY .devcontainer/bootstrap-lock.json /usr/local/share/rice-devcontainer/bootstrap-lock.json

ENV CARGO_HOME=/usr/local/cargo \
  RUSTUP_HOME=/usr/local/rustup \
  PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN for binary in /usr/local/cargo/bin/*; do \
  ln -sf "${binary}" "/usr/local/bin/$(basename "${binary}")"; \
  done

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    file \
    fonts-noto-cjk \
    jq \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libssl-dev \
    libwebkit2gtk-4.1-dev \
    libxdo-dev \
    pkg-config \
    vim \
    wget \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/*
