FROM debian:bookworm-slim

ARG USER_ID=1000
ARG USER_NAME=builder
ENV DEBIAN_FRONTEND=noninteractive

# Install build tools and common libs needed for Rust/Substrate builds
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    ca-certificates \
    git \
    pkg-config \
    libssl-dev \
    clang \
    libclang-dev \
    cmake \
    llvm \
    musl-tools \
    ca-certificates \
    libudev-dev \
    make \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# create an unprivileged user (UID can be overridden at build time)
RUN useradd -m -u ${USER_ID} ${USER_NAME} || useradd -m ${USER_NAME}

ENV HOME=/home/${USER_NAME}
ENV PATH=${HOME}/.cargo/bin:/usr/local/cargo/bin:${PATH}

# Copy entrypoint and make it executable as root (builder can't chmod /usr/local)
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# Switch to the unprivileged builder user and set working dir
USER ${USER_NAME}
WORKDIR /workspace

# Install rustup and a default toolchain (stable). We add common components/targets used by Substrate projects.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
RUN ${HOME}/.cargo/bin/rustup component add rustfmt clippy rust-src || true
RUN ${HOME}/.cargo/bin/rustup target add wasm32-unknown-unknown || true

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
