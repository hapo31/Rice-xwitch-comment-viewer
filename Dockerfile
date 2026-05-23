# syntax=docker/dockerfile:1.7

ARG RUST_IMAGE=rust:1.89.0-bookworm
ARG PNPM_VERSION=8.11.0
ARG CARGO_XWIN_VERSION=0.22.0
ARG WINDOWS_TARGET=x86_64-pc-windows-msvc

FROM ${RUST_IMAGE} AS build

ARG PNPM_VERSION
ARG CARGO_XWIN_VERSION
ARG WINDOWS_TARGET

ENV CARGO_TERM_COLOR=always \
    NPM_CONFIG_AUDIT=false \
    NPM_CONFIG_FUND=false

WORKDIR /work

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        clang \
        curl \
        lld \
        llvm \
        nodejs \
        npm \
        nsis \
    && rm -rf /var/lib/apt/lists/*

RUN npm install --global "pnpm@${PNPM_VERSION}" \
    && rustup target add "${WINDOWS_TARGET}" \
    && cargo install --locked --version "${CARGO_XWIN_VERSION}" cargo-xwin

COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

COPY index.html postcss.config.js tailwind.config.js tsconfig.json vite.config.ts ./
COPY src ./src
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/build.rs src-tauri/tauri.conf.json ./src-tauri/
COPY src-tauri/capabilities ./src-tauri/capabilities
COPY src-tauri/icons ./src-tauri/icons
COPY src-tauri/src ./src-tauri/src

RUN pnpm tauri build --bundles nsis --runner cargo-xwin --target "${WINDOWS_TARGET}"

RUN mkdir /out \
    && find "src-tauri/target/${WINDOWS_TARGET}/release/bundle/nsis" \
        -maxdepth 1 \
        -type f \
        -exec cp {} /out/ \;

FROM scratch AS artifacts
COPY --from=build /out/ /
