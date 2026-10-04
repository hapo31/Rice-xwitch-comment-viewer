# syntax=docker/dockerfile:1.7

ARG RUST_IMAGE=rust:1.89.0-bookworm@sha256:948f9b08a66e7fe01b03a98ef1c7568292e07ec2e4fe90d88c07bb14563c84ff
ARG NODE_IMAGE=node:22.22.0-bookworm-slim@sha256:dd9d21971ec4395903fa6143c2b9267d048ae01ca6d3ea96f16cb30df6187d94
ARG DEBIAN_SNAPSHOT=20260921T000000Z
ARG PNPM_VERSION=8.11.0
ARG CARGO_XWIN_VERSION=0.22.0
ARG WINDOWS_TARGET=x86_64-pc-windows-msvc

FROM ${NODE_IMAGE} AS node

FROM ${RUST_IMAGE} AS tools

ARG PNPM_VERSION
ARG CARGO_XWIN_VERSION
ARG WINDOWS_TARGET
ARG DEBIAN_SNAPSHOT

ENV CARGO_TERM_COLOR=always \
    NPM_CONFIG_AUDIT=false \
    NPM_CONFIG_FUND=false \
    XWIN_CACHE_DIR=/opt/xwin-cache \
    TZ=UTC \
    LC_ALL=C.UTF-8

WORKDIR /work

COPY --from=node /usr/local /usr/local

RUN rm -f /etc/apt/sources.list /etc/apt/sources.list.d/debian.sources \
    && printf 'deb [check-valid-until=no] https://snapshot.debian.org/archive/debian/%s/ bookworm main\ndeb [check-valid-until=no] https://snapshot.debian.org/archive/debian-security/%s/ bookworm-security main\n' "${DEBIAN_SNAPSHOT}" "${DEBIAN_SNAPSHOT}" > /etc/apt/sources.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        clang \
        curl \
        lld \
        llvm \
        nsis \
        zip \
    && rm -rf /var/lib/apt/lists/*

RUN npm install --global "pnpm@${PNPM_VERSION}" \
    && rustup target add "${WINDOWS_TARGET}" \
    && cargo install --locked --version "${CARGO_XWIN_VERSION}" cargo-xwin

FROM tools AS build
ARG RUST_IMAGE
ARG NODE_IMAGE
ARG DEBIAN_SNAPSHOT
ARG PNPM_VERSION
ARG CARGO_XWIN_VERSION
ARG WINDOWS_TARGET
ARG RICE_TWITCH_CLIENT_ID
ARG RICE_GIT_COMMIT
ARG SOURCE_DATE_EPOCH
ENV RICE_GIT_COMMIT=${RICE_GIT_COMMIT} SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH}
ENV RICE_BUILD_RUST_IMAGE=${RUST_IMAGE} \
    RICE_BUILD_NODE_IMAGE=${NODE_IMAGE} \
    RICE_BUILD_DEBIAN_SNAPSHOT=${DEBIAN_SNAPSHOT} \
    RICE_BUILD_PNPM_VERSION=${PNPM_VERSION} \
    RICE_BUILD_CARGO_XWIN_VERSION=${CARGO_XWIN_VERSION} \
    RICE_BUILD_WINDOWS_TARGET=${WINDOWS_TARGET}

COPY build/release-inputs.json ./build/release-inputs.json
COPY Dockerfile ./Dockerfile
COPY scripts/verify-release-build-inputs.mjs scripts/record-build-materials.mjs ./scripts/
RUN node scripts/verify-release-build-inputs.mjs --runtime \
    && test "${#RICE_GIT_COMMIT}" = 40 \
    && test "${SOURCE_DATE_EPOCH}" -ge 315532800

COPY scripts/verify-twitch-client-id.mjs ./scripts/verify-twitch-client-id.mjs
RUN node scripts/verify-twitch-client-id.mjs

COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

COPY index.html postcss.config.js tailwind.config.js tsconfig.json vite.config.ts ./
COPY src ./src
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/build.rs src-tauri/tauri.conf.json ./src-tauri/
COPY src-tauri/capabilities ./src-tauri/capabilities
COPY src-tauri/icons ./src-tauri/icons
COPY src-tauri/src ./src-tauri/src

RUN pnpm tauri build --bundles nsis --runner cargo-xwin --target "${WINDOWS_TARGET}"

RUN node scripts/verify-twitch-client-id.mjs "src-tauri/target/${WINDOWS_TARGET}/release/rice.exe"

RUN mkdir /out \
    && find "src-tauri/target/${WINDOWS_TARGET}/release/bundle/nsis" \
        -maxdepth 1 \
        -type f \
        -exec cp {} /out/ \; \
    && app_version="$(node -p 'require("./package.json").version')" \
    && cd "src-tauri/target/${WINDOWS_TARGET}/release" \
    && touch -d "@${SOURCE_DATE_EPOCH}" rice.exe \
    && zip -X -9 "/out/Rice_${app_version}_${WINDOWS_TARGET}_portable.zip" rice.exe

RUN node scripts/record-build-materials.mjs && test -s /out/BUILD-MATERIALS.json

FROM scratch AS artifacts
COPY --from=build /out/ /
