FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

ENV DEBIAN_FRONTEND=noninteractive

# Core build and shell tools for Rust/Node and likely near-term desktop exploration.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    openssh-client \
    jq \
    ripgrep \
    fd-find \
    unzip \
    xz-utils \
    python3 \
    python3-pip \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    patchelf \
    && rm -rf /var/lib/apt/lists/*

# Node.js LTS and pnpm.
RUN curl -fsSL https://deb.nodesource.com/setup_lts.x | bash - \
    && apt-get update \
    && apt-get install -y --no-install-recommends nodejs \
    && npm install -g pnpm \
    && rm -rf /var/lib/apt/lists/*

# Rust stable toolchain with rustfmt and clippy.
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:${PATH}

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile default --default-toolchain stable \
    && rustup component add rustfmt clippy \
    && rustc --version \
    && cargo --version

# Make tool directories writable for the non-root vscode user.
RUN mkdir -p /usr/local/cargo/registry /usr/local/cargo/git /workspace-target \
    && chown -R vscode:vscode /usr/local/cargo /usr/local/rustup /workspace-target

USER vscode
