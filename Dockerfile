# Cross-compiles terra-control for the Raspberry Pi 3A+ (64-bit Raspberry Pi OS, aarch64).
#
#   docker build --output dist .
#
# The binary ends up in dist/terra-control. Bookworm's glibc (2.36) matches Raspberry Pi OS
# Bookworm and runs on newer releases as well.

FROM rust:1.97-bookworm AS build

# aarch64 GCC + headers for the C parts (bundled SQLite, rpi_ws281x),
# clang/libclang for the rpi_ws281x bindings (bindgen)
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        gcc-aarch64-linux-gnu libc6-dev-arm64-cross clang libclang-dev \
 && rm -rf /var/lib/apt/lists/*
RUN rustup target add aarch64-unknown-linux-gnu

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar \
    RPI_WS281X_SYSROOT=/usr/aarch64-linux-gnu \
    BINDGEN_EXTRA_CLANG_ARGS_aarch64_unknown_linux_gnu=--sysroot=/usr/aarch64-linux-gnu

WORKDIR /src
COPY Cargo.toml Cargo.lock main.rs ./
COPY modules ./modules
COPY migrations ./migrations

# Cache mounts keep the registry and build artefacts between builds. COPY keeps the file
# times from the host, which cargo may consider older than the cached build, so the own
# sources are touched to force a rebuild of this crate (dependencies stay cached).
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    touch main.rs modules/*.rs migrations/*.sql \
 && cargo build --release --locked --target aarch64-unknown-linux-gnu \
 && cp target/aarch64-unknown-linux-gnu/release/terra-control /terra-control

FROM scratch
COPY --from=build /terra-control /terra-control
