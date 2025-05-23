# syntax=docker/dockerfile:1
# check=skip=FromPlatformFlagConstDisallowed;error=true

# Force a platform to ensure we are buidling in an environment 
# that has a native Anza client.
# This base image should be pinned to the same version as in the rust-toolchain file.
FROM --platform=linux/amd64 rust:1.85 AS builder

# These declarations must happen in _this_ layer (after FROM) so that they can be accessed in the current layer.
# 
ARG SOLANA_VERSION=v2.1.18
ARG ANCHOR_VERSION=v0.31.0

COPY rust-toolchain.toml /rust-toolchain.toml

RUN apt-get update && apt-get install -y --no-install-recommends \
  curl \
  build-essential \
  pkg-config \
  libudev-dev \
  && rm -rf /var/lib/apt/lists/*

RUN sh -c "$(curl -sSfL https://release.anza.xyz/${SOLANA_VERSION}/install)"
ENV PATH="/root/.local/share/solana/install/active_release/bin:$PATH"

# Set RUSTFLAGS to target generic CPU to avoid potential SIGILL errors under QEMU emulation
ENV RUSTFLAGS="-C target-cpu=generic"
RUN cargo install --git https://github.com/coral-xyz/anchor --tag ${ANCHOR_VERSION} anchor-cli --locked

ENV RUSTFLAGS=""

WORKDIR /app

COPY . .

RUN anchor build --no-idl --skip-lint
