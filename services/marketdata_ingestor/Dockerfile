# syntax=docker/dockerfile:1.7

FROM rust:1.94-trixie AS chef
RUN cargo install cargo-chef
WORKDIR /workspace

FROM chef AS planner
ARG BIN=marketdata_ingestor
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG BIN=marketdata_ingestor
COPY --from=planner /workspace/recipe.json recipe.json

ENV DEBIAN_FRONTEND=noninteractive
RUN echo 'Acquire::ForceIPv4 "true";' > /etc/apt/apt.conf.d/99force-ipv4 && \
  apt-get update && apt-get install -y --no-install-recommends \
  build-essential clang llvm cmake pkg-config libssl-dev libsasl2-dev \
  protobuf-compiler \
  && rm -rf /var/lib/apt/lists/*

ENV RUSTFLAGS="-C linker=clang" CC=clang

RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/usr/local/cargo/git \
  cargo chef cook --release --recipe-path recipe.json --package marketdata_ingestor

COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/usr/local/cargo/git \
  cargo build --release -p marketdata_ingestor

FROM debian:trixie-slim AS runtime
ENV DEBIAN_FRONTEND=noninteractive
RUN echo 'Acquire::ForceIPv4 "true";' > /etc/apt/apt.conf.d/99force-ipv4 && \
  apt-get update && apt-get install -y --no-install-recommends \
  ca-certificates openssl libsasl2-2 && \
  rm -rf /var/lib/apt/lists/* && \
  useradd -u 10001 -m runner

WORKDIR /srv
ARG BIN=marketdata_ingestor
COPY --from=builder /workspace/target/release/marketdata_ingestor /srv/app

USER runner
EXPOSE 8080
ENV RUST_LOG=info
ENTRYPOINT ["/srv/app"]

