# syntax=docker/dockerfile:1.7
FROM rust:1.93-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        file \
        git \
        libfontconfig1-dev \
        libwayland-dev \
        libxkbcommon-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*
RUN rustup component add clippy rustfmt

WORKDIR /src
COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/target \
    CARGO_TARGET_DIR=/target cargo fmt -- --check \
    && CARGO_TARGET_DIR=/target cargo test --locked \
    && CARGO_TARGET_DIR=/target cargo clippy --locked --all-targets -- -D warnings \
    && CARGO_TARGET_DIR=/target cargo build --locked --release \
    && mkdir -p /out \
    && cp /target/release/moonlight-clock /out/moonlight-clock \
    && file /out/moonlight-clock > /out/file.txt \
    && ldd /out/moonlight-clock > /out/ldd.txt \
    && cd /out \
    && sha256sum moonlight-clock > moonlight-clock.sha256

FROM scratch AS export
COPY --from=builder /out /
